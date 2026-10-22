//! Generator of the Chicago crime dataset in a CanopyDB base (child PRD `canopy-db-generador`).
//!
//! It shares the whole pipeline of the original generator (decision K1 of the parent PRD and
//! KG11 of the child PRD): options, validation, sorting, publication, log, messages, exit codes
//! and summary. Only the storage of the tables differs: [`CanopyStorage`] writes the primary
//! table and the five indexes as trees of one CanopyDB database inside the `canopydb/` folder of
//! the version folder (decision KG2), with the same keys and values as the LevelDB tables
//! (decision K2), and describes them with the CanopyDB manifest (decision KG3).

use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use canopydb::{Database, DbOptions, EnvOptions, Environment, TreeOptions};
use crime_dataset::{IndexKind, RecordCounts, SourceInfo};
use crime_generator::storage::{ManifestInfo, Storage, Written};
use crime_generator::{Abort, App};
use serde::Serialize;

/// The application (decision KG1): the options of the original generator with its own name, the
/// description of what it writes, the help of `--no-compression` (decision KG4) and its own sort
/// folders (decision KG6).
pub const APP: App = App {
    name: "crime-canopy-generator",
    about: "Genera el conjunto de datos de delitos de Chicago (base CanopyDB y manifiesto) a \
            partir del CSV.",
    no_compression_help: "Escribe la base sin compresión. Por defecto, CanopyDB comprime con LZ4 \
                          los valores grandes.",
    sort_dir_prefix: "crime-canopy-generator-sort-",
};

// The names of the CanopyDB dataset contract and the engine detail line, shared with the
// CanopyDB query app.
pub use crime_dataset::canopy::{
    BASE_DIR_NAME, CANOPYDB_VERSION, DATABASE_NAME, PRIMARY_TABLE_NAME, SCHEMA_VERSION,
    engine_detail, panic_detail,
};

/// Page cache of the base: CanopyDB's default of 1 GiB is twice the memory limit of the run.
pub const PAGE_CACHE_BYTES: usize = 64 << 20;

/// Uncommitted changes a write transaction keeps in memory, and committed changes that start a
/// checkpoint.
const TRANSACTION_MEMORY_BYTES: usize = 32 << 20;

/// Committed changes above which new write transactions wait for the checkpoint.
const STALL_MEMORY_BYTES: usize = 128 << 20;

/// Entries written per transaction.
const ENTRIES_PER_COMMIT: usize = 100_000;

/// The options a CanopyDB base of this application is opened with: the page cache of
/// [`PAGE_CACHE_BYTES`] and no write-ahead log, so the base holds no log to replay and a reader
/// adds no file to it (decision K6 of the parent PRD).
pub fn database_options() -> DbOptions {
    let mut options = DbOptions::default();
    options.use_wal = false;
    options.write_txn_memory_limit = TRANSACTION_MEMORY_BYTES;
    options.checkpoint_target_size = TRANSACTION_MEMORY_BYTES;
    options.throttle_memory_limit = STALL_MEMORY_BYTES;
    options.stall_memory_limit = STALL_MEMORY_BYTES;
    options
}

/// The environment options of a base in `path`; CanopyDB's temporary files go to `scratch`.
pub fn environment_options(path: &Path, scratch: &Path) -> EnvOptions {
    let mut options = EnvOptions::new(path);
    options.page_cache_size = PAGE_CACHE_BYTES;
    options.wal_write_batch_tempfile_dir = scratch.to_path_buf();
    options
}

/// Writes the tables as trees of one CanopyDB database (see the crate documentation).
pub struct CanopyStorage {
    compression: bool,
    /// The output folder, named by the messages of a write failure.
    output: PathBuf,
    base_dir: PathBuf,
    open: Option<(Environment, Database)>,
    table: &'static str,
    pending: Vec<(Vec<u8>, Vec<u8>)>,
    entries: u64,
    tables: Vec<(&'static str, u64)>,
}

impl CanopyStorage {
    /// `compression` keeps CanopyDB's compression of large values (decision KG4).
    pub fn new(compression: bool, output: &Path) -> Self {
        Self {
            compression,
            output: output.to_path_buf(),
            base_dir: PathBuf::new(),
            open: None,
            table: PRIMARY_TABLE_NAME,
            pending: Vec::with_capacity(ENTRIES_PER_COMMIT),
            entries: 0,
            tables: Vec::new(),
        }
    }

    /// The abort for a failure of the engine (decision KG7): the message of the original
    /// generator for the output folder, the only folder the engine writes to (it writes no
    /// temporary files without its write-ahead log), with the error as the detail line.
    fn engine(&self, error: canopydb::Error) -> Abort {
        let cause = match &error {
            canopydb::Error::Io(io_error) | canopydb::Error::FatalIo(io_error) => {
                Abort::output_io(&self.output, io_error)
            }
            _ => Abort::OutputUnwritable {
                dir: self.output.clone(),
            },
        };
        Abort::WithDetail {
            detail: engine_detail(&error.to_string()),
            cause: Box::new(cause),
        }
    }

    /// Runs `operation` of the engine, turning its error or its panic into the abort of the run.
    fn guarded<T>(
        &self,
        operation: impl FnOnce() -> Result<T, canopydb::Error>,
    ) -> Result<T, Abort> {
        match catch_unwind(AssertUnwindSafe(operation)) {
            Ok(result) => result.map_err(|error| self.engine(error)),
            Err(payload) => Err(Abort::WithDetail {
                detail: panic_detail(payload.as_ref()),
                cause: Box::new(Abort::OutputUnwritable {
                    dir: self.output.clone(),
                }),
            }),
        }
    }

    fn tree_options(&self) -> TreeOptions {
        let mut options = TreeOptions::default();
        if !self.compression {
            options.compress_overflow_values = None;
        }
        options
    }

    fn database(&self) -> &Database {
        &self.open.as_ref().expect("the base is open").1
    }

    /// Starts the table `name`: its tree exists from now on, even if it gets no entry.
    fn start_table(&mut self, name: &'static str) -> Result<(), Abort> {
        self.table = name;
        self.entries = 0;
        self.commit()
    }

    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<(), Abort> {
        self.pending.push((key.to_vec(), value.to_vec()));
        self.entries += 1;
        if self.pending.len() == ENTRIES_PER_COMMIT {
            self.commit()?;
        }
        Ok(())
    }

    /// Writes the pending entries of the current table in one transaction.
    fn commit(&mut self) -> Result<(), Abort> {
        let options = self.tree_options();
        let result = self.guarded(|| {
            fault::inject()?;
            let transaction = self.database().begin_write()?;
            {
                let mut tree =
                    transaction.get_or_create_tree_with(self.table.as_bytes(), options)?;
                for (key, value) in &self.pending {
                    tree.insert(key, value)?;
                }
            }
            transaction.commit().map(|_| ())
        });
        self.pending.clear();
        result
    }

    fn finish_table(&mut self) -> Result<(), Abort> {
        self.commit()?;
        self.tables.push((self.table, self.entries));
        Ok(())
    }
}

impl Storage for CanopyStorage {
    type Written = CanopyWritten;

    /// Creates the base in `canopydb/` and its primary tree.
    fn begin(&mut self, dir: &Path, scratch: &Path) -> Result<(), Abort> {
        self.base_dir = dir.join(BASE_DIR_NAME);
        // CanopyDB does not create the folder of its base.
        fs::create_dir(&self.base_dir).map_err(|error| Abort::output_io(&self.output, &error))?;
        let environment = self
            .guarded(|| Environment::with_options(environment_options(&self.base_dir, scratch)))?;
        let database = self.guarded(|| {
            environment.get_or_create_database_with(DATABASE_NAME, database_options())
        })?;
        self.open = Some((environment, database));
        self.start_table(PRIMARY_TABLE_NAME)
    }

    fn add_primary(&mut self, key: &[u8], value: &[u8]) -> Result<(), Abort> {
        self.add(key, value)
    }

    fn finish_primary(&mut self) -> Result<(), Abort> {
        self.finish_table()
    }

    fn begin_index(&mut self, kind: IndexKind) -> Result<(), Abort> {
        self.start_table(kind.name())
    }

    fn add_index(&mut self, key: &[u8]) -> Result<(), Abort> {
        self.add(key, &[])
    }

    fn finish_index(&mut self) -> Result<(), Abort> {
        self.finish_table()
    }

    /// Moves everything to the data file and closes the base, so that it is ready to be read
    /// (flow step 5 of the child PRD), then lists its files.
    fn finish(mut self) -> Result<CanopyWritten, Abort> {
        self.guarded(|| self.database().checkpoint())?;
        // Closing waits for the background threads of the engine.
        let open = self.open.take();
        self.guarded(|| {
            drop(open);
            Ok(())
        })?;
        let version_dir = self
            .base_dir
            .parent()
            .expect("the base is inside the version folder");
        let files = list_files(version_dir, &self.base_dir)
            .map_err(|error| Abort::output_io(&self.output, &error))?;
        let mut tables = self.tables.into_iter();
        let primary_entries = tables.next().map_or(0, |(_, entries)| entries);
        Ok(CanopyWritten {
            compression: self.compression,
            primary_entries,
            indexes: IndexKind::ALL
                .into_iter()
                .zip(tables.map(|(_, entries)| entries))
                .collect(),
            files,
        })
    }
}

/// The message of a panic, as `panic!` received it.
/// Failures of the engine injected by the tests (feature `fail-points`); without the feature
/// nothing is ever injected.
pub mod fault {
    /// A failure the next write of the base meets.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Fault {
        /// The disk of the base is full.
        DiskFull,
        /// Any other failure of a write.
        WriteFailure,
        /// A failure of the engine itself, not of the disk.
        Internal,
        /// The engine panics.
        Panic,
        /// An I/O error whose text spans several lines.
        MultiLine,
    }

    #[cfg(feature = "fail-points")]
    std::thread_local! {
        static ARMED: std::cell::Cell<Option<Fault>> = const { std::cell::Cell::new(None) };
    }

    /// Arms `fault` on this thread for the next write of the base, until the guard drops.
    #[cfg(feature = "fail-points")]
    pub fn arm(fault: Fault) -> Armed {
        ARMED.with(|armed| armed.set(Some(fault)));
        Armed { _private: () }
    }

    /// Disarms the fault when dropped.
    #[cfg(feature = "fail-points")]
    pub struct Armed {
        _private: (),
    }

    #[cfg(feature = "fail-points")]
    impl Drop for Armed {
        fn drop(&mut self) {
            ARMED.with(|armed| armed.set(None));
        }
    }

    /// Meets the armed fault, once.
    #[cfg(feature = "fail-points")]
    pub(crate) fn inject() -> Result<(), canopydb::Error> {
        use std::io::{Error, ErrorKind};
        match ARMED.with(|armed| armed.take()) {
            None => Ok(()),
            Some(Fault::DiskFull) => Err(canopydb::Error::Io(Error::from(ErrorKind::StorageFull))),
            Some(Fault::WriteFailure) => {
                Err(canopydb::Error::Io(Error::other("injected write failure")))
            }
            Some(Fault::Internal) => Err(canopydb::Error::DatabaseHalted),
            Some(Fault::Panic) => panic!("injected engine panic"),
            Some(Fault::MultiLine) => {
                Err(canopydb::Error::Io(Error::other("first line\nsecond line")))
            }
        }
    }

    #[cfg(not(feature = "fail-points"))]
    pub(crate) fn inject() -> Result<(), canopydb::Error> {
        Ok(())
    }
}

/// Every file under `dir`, with its path relative to `root` separated with `/` and its size, in
/// byte order of the path (see the manifest of the child PRD).
fn list_files(root: &Path, dir: &Path) -> io::Result<Vec<(String, u64)>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("the file is under the root")
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                files.push((relative, metadata.len()));
            }
        }
    }
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(files)
}

/// The tables [`CanopyStorage`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanopyWritten {
    pub compression: bool,
    pub primary_entries: u64,
    /// Entries of each index, in [`IndexKind::ALL`] order.
    pub indexes: Vec<(IndexKind, u64)>,
    /// Every file of the base with its path relative to the version folder and its size when
    /// the base was closed, in byte order of the path.
    pub files: Vec<(String, u64)>,
}

impl Written for CanopyWritten {
    fn primary_entries(&self) -> u64 {
        self.primary_entries
    }

    fn index_entries(&self) -> Vec<(IndexKind, u64)> {
        self.indexes.clone()
    }

    fn manifest_json(&self, info: &ManifestInfo) -> String {
        let mut tables = vec![TableEntries {
            name: PRIMARY_TABLE_NAME,
            entries: self.primary_entries,
        }];
        tables.extend(self.indexes.iter().map(|&(kind, entries)| TableEntries {
            name: kind.name(),
            entries,
        }));
        let manifest = CanopyManifest {
            schema_version: SCHEMA_VERSION,
            generated_at: &info.generated_at,
            engine: Engine {
                name: crime_dataset::canopy::ENGINE_NAME,
                version: CANOPYDB_VERSION,
                fork_commit: None,
            },
            source: &info.source,
            records: info.records,
            compression: if self.compression { "lz4" } else { "none" },
            tables,
            files: self
                .files
                .iter()
                .map(|(path, size_bytes)| FileEntry {
                    path,
                    size_bytes: *size_bytes,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&manifest).expect("a manifest always serializes to JSON")
    }

    fn files(&self) -> Vec<(String, u64)> {
        self.files.clone()
    }
}

/// The CanopyDB manifest (decision KG3), with its fields in this order.
#[derive(Serialize)]
struct CanopyManifest<'a> {
    schema_version: &'static str,
    generated_at: &'a str,
    engine: Engine,
    source: &'a SourceInfo,
    records: RecordCounts,
    compression: &'static str,
    tables: Vec<TableEntries>,
    files: Vec<FileEntry<'a>>,
}

#[derive(Serialize)]
struct Engine {
    name: &'static str,
    version: &'static str,
    /// The commit of the fork when the application depends on one (flow E of the parent PRD).
    fork_commit: Option<&'static str>,
}

#[derive(Serialize)]
struct TableEntries {
    name: &'static str,
    entries: u64,
}

#[derive(Serialize)]
struct FileEntry<'a> {
    path: &'a str,
    size_bytes: u64,
}
