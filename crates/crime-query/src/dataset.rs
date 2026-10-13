//! Opening a published dataset as a snapshot, and every read the query engine makes from it.
//!
//! Opening follows the dataset layout (see `crime_dataset::resolve_current`): resolve `CURRENT`
//! to the active version folder, read its manifest, check the schema version, and open a handle
//! to every table file before any table data is read. From then on the dataset is a snapshot: a
//! generator that publishes a new version and deletes this one cannot change what an open
//! dataset reads (AC-C12), because the version folder is never modified once published and
//! open handles keep the files readable after they are deleted (Rust opens files on Windows
//! with delete sharing). A table that vanished between resolving `CURRENT` and opening it means
//! such a publication happened in between, so opening starts over, a few times at most.
//!
//! The query app only reads: nothing here creates, changes or deletes anything in the folder.

use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crime_dataset::{
    CivilDateTime, CrimeRecord, CurrentError, IndexKind, MANIFEST_FILE_NAME, Manifest,
    PRIMARY_FILE_NAME, SCHEMA_VERSION, decode_value, primary_key, resolve_current,
};
use serde::Deserialize;
use sstable::{Options, Status, Table};

use crate::index::{IdStream, IndexValue};
use crate::messages;
use crate::tracked::{ReadHealth, TrackedFile, read_exact_at};

/// How many times opening resolves `CURRENT` when the version it named vanished meanwhile.
const OPEN_ATTEMPTS: usize = 3;

/// Bytes at the end of every table file: two block handles, padding and the magic number.
const TABLE_FOOTER_BYTES: u64 = 48;

/// The last 8 bytes of every table file in the LevelDB table format.
const TABLE_MAGIC: [u8; 8] = [0x57, 0xfb, 0x80, 0x8b, 0x24, 0x75, 0x47, 0xdb];

/// Why a dataset could not be opened. [`fmt::Display`] gives the message for the analyst.
#[derive(Debug)]
pub enum OpenError {
    /// No `CURRENT`, a `CURRENT` that names no existing version, a missing or unparseable
    /// manifest, or a missing table.
    NotFound { folder: PathBuf },
    /// The manifest declares a schema version other than [`SCHEMA_VERSION`].
    IncompatibleSchema { found: u32 },
    /// A file exists but cannot be read, or a table file is not a table.
    Unreadable { folder: PathBuf, cause: String },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NotFound { folder } => f.write_str(&messages::dataset_not_found(folder)),
            OpenError::IncompatibleSchema { found } => {
                f.write_str(&messages::incompatible_schema(*found))
            }
            OpenError::Unreadable { folder, .. } => {
                f.write_str(&messages::dataset_unreadable(folder))
            }
        }
    }
}

impl std::error::Error for OpenError {}

/// A read of the open dataset failed: the query must abort rather than return a partial result.
/// [`fmt::Display`] gives the message for the analyst; [`ReadError::cause`] says what failed.
#[derive(Debug, Clone)]
pub struct ReadError {
    folder: PathBuf,
    cause: String,
}

impl ReadError {
    pub(crate) fn new(folder: PathBuf, cause: String) -> Self {
        Self { folder, cause }
    }

    /// What failed, in English, for diagnostics.
    pub fn cause(&self) -> &str {
        &self.cause
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&messages::dataset_unreadable(&self.folder))
    }
}

impl std::error::Error for ReadError {}

/// One published version of the dataset, opened as a snapshot.
pub struct Dataset {
    /// The output folder exactly as the analyst gave it.
    folder: PathBuf,
    version: String,
    manifest: Manifest,
    primary: Table,
    /// One per index, in [`IndexKind::ALL`] order.
    indexes: Vec<Table>,
    health: Arc<ReadHealth>,
}

/// Why one attempt to open failed.
enum Attempt {
    /// The version named by `CURRENT` disappeared before every file was open: try again.
    Vanished,
    Failed(OpenError),
}

impl Dataset {
    /// Opens the version of the dataset that `CURRENT` names in `folder`.
    pub fn open(folder: &Path) -> Result<Dataset, OpenError> {
        for _ in 0..OPEN_ATTEMPTS {
            match open_version(folder) {
                Ok(dataset) => return Ok(dataset),
                Err(Attempt::Vanished) => {}
                Err(Attempt::Failed(error)) => return Err(error),
            }
        }
        Err(not_found(folder))
    }

    /// The output folder exactly as the analyst gave it.
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Name of the open version folder.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The record with this ID, every field exactly as the CSV has it.
    pub fn record(&self, id: i64) -> Result<Option<CrimeRecord>, ReadError> {
        self.primary_value(id)?
            .map(|bytes| {
                decode_value(&bytes)
                    .map_err(|error| self.error(format!("record {id} cannot be decoded: {error}")))
            })
            .transpose()
    }

    /// Whether a record with this ID exists.
    pub fn id_exists(&self, id: i64) -> Result<bool, ReadError> {
        Ok(self.primary_value(id)?.is_some())
    }

    /// Every ID whose `kind` index holds exactly `value`, in ascending ID order. For the
    /// equality indexes: primary type, district, community area and case number.
    pub fn ids_with_value(&self, kind: IndexKind, value: &IndexValue) -> IdStream {
        IdStream::for_value(
            self.index(kind),
            value,
            Arc::clone(&self.health),
            self.folder.clone(),
        )
    }

    /// Every ID whose date is within `[from, to]`, both ends included, in date and then ID
    /// order (not ID order).
    pub fn ids_in_date_range(&self, from: CivilDateTime, to: CivilDateTime) -> IdStream {
        IdStream::for_dates(
            self.index(IndexKind::Date),
            from.seconds(),
            to.seconds(),
            Arc::clone(&self.health),
            self.folder.clone(),
        )
    }

    /// Whether the record `id` has `value` in the `kind` index: one point lookup of the key
    /// `(value, id)`, so a stream from another index can be checked against this one without
    /// holding either set in memory.
    pub fn has_entry(
        &self,
        kind: IndexKind,
        value: &IndexValue,
        id: i64,
    ) -> Result<bool, ReadError> {
        let found = self.index(kind).get(&value.key(id));
        self.check()?;
        let found = found.map_err(|status| self.table_error(kind.file_name(), &status))?;
        Ok(found.is_some())
    }

    /// Whether any record has `value` in the `kind` index.
    pub fn has_value(&self, kind: IndexKind, value: &IndexValue) -> Result<bool, ReadError> {
        let first = self.ids_with_value(kind, value).next().transpose()?;
        Ok(first.is_some())
    }

    fn index(&self, kind: IndexKind) -> &Table {
        &self.indexes[usize::from(kind.ordinal())]
    }

    fn primary_value(&self, id: i64) -> Result<Option<Vec<u8>>, ReadError> {
        let found = self.primary.get(&primary_key(id));
        self.check()?;
        found.map_err(|status| self.table_error(PRIMARY_FILE_NAME, &status))
    }

    /// Fails when any read of the dataset has failed so far.
    fn check(&self) -> Result<(), ReadError> {
        match self.health.failure() {
            Some(cause) => Err(self.error(cause)),
            None => Ok(()),
        }
    }

    fn table_error(&self, file: &str, status: &Status) -> ReadError {
        self.error(format!("{file}: {}", status_text(status)))
    }

    fn error(&self, cause: String) -> ReadError {
        ReadError::new(self.folder.clone(), cause)
    }
}

/// One attempt: resolve `CURRENT`, read and check the manifest, open every table.
fn open_version(folder: &Path) -> Result<Dataset, Attempt> {
    let failed = |error| Attempt::Failed(error);
    let version_dir = match resolve_current(folder) {
        Ok(dir) => dir,
        Err(CurrentError::MissingVersion(_)) => return Err(Attempt::Vanished),
        Err(CurrentError::Unreadable(error)) => {
            return Err(failed(unreadable(folder, format!("CURRENT: {error}"))));
        }
        Err(CurrentError::NoDataset | CurrentError::InvalidName(_)) => {
            return Err(failed(not_found(folder)));
        }
    };
    let manifest = read_manifest(folder, &version_dir)?;

    let health = ReadHealth::new();
    let primary_file = open_table_file(folder, &version_dir, PRIMARY_FILE_NAME)?;
    let mut index_files = Vec::with_capacity(IndexKind::ALL.len());
    for kind in IndexKind::ALL {
        index_files.push(open_table_file(folder, &version_dir, kind.file_name())?);
    }
    // Every handle is open: the snapshot no longer depends on the version folder existing.

    // One bounded block cache shared by every table.
    let options = Options::default();
    let primary = open_table(folder, &options, primary_file, &health)?;
    let indexes = index_files
        .into_iter()
        .map(|file| open_table(folder, &options, file, &health))
        .collect::<Result<_, _>>()?;
    let version = version_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Dataset {
        folder: folder.to_path_buf(),
        version,
        manifest,
        primary,
        indexes,
        health,
    })
}

/// The only field read before the schema version is known to be supported, so that a manifest
/// of another schema, whatever else it holds, gets the schema message.
#[derive(Deserialize)]
struct SchemaProbe {
    schema_version: u32,
}

fn read_manifest(folder: &Path, version_dir: &Path) -> Result<Manifest, Attempt> {
    let bytes = match std::fs::read(version_dir.join(MANIFEST_FILE_NAME)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(Attempt::Vanished),
        Err(error) => {
            return Err(Attempt::Failed(unreadable(
                folder,
                format!("{MANIFEST_FILE_NAME}: {error}"),
            )));
        }
    };
    let invalid = || Attempt::Failed(not_found(folder));
    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    let probe: SchemaProbe = serde_json::from_str(text).map_err(|_| invalid())?;
    if probe.schema_version != SCHEMA_VERSION {
        return Err(Attempt::Failed(OpenError::IncompatibleSchema {
            found: probe.schema_version,
        }));
    }
    Manifest::from_json(text).map_err(|_| invalid())
}

/// An opened table file and its name.
struct TableFile {
    file: File,
    name: &'static str,
}

fn open_table_file(
    folder: &Path,
    version_dir: &Path,
    name: &'static str,
) -> Result<TableFile, Attempt> {
    match File::open(version_dir.join(name)) {
        Ok(file) => Ok(TableFile { file, name }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(Attempt::Vanished),
        Err(error) => Err(Attempt::Failed(unreadable(
            folder,
            format!("{name}: {error}"),
        ))),
    }
}

/// Reads the table index of an opened file.
///
/// The footer is checked first: the `sstable` reader panics on a file shorter than its footer
/// or without the magic number.
fn open_table(
    folder: &Path,
    options: &Options,
    table: TableFile,
    health: &Arc<ReadHealth>,
) -> Result<Table, Attempt> {
    let TableFile { file, name } = table;
    let fail = |cause: String| Attempt::Failed(unreadable(folder, format!("{name}: {cause}")));
    let size = file
        .metadata()
        .map_err(|error| fail(error.to_string()))?
        .len();
    if size < TABLE_FOOTER_BYTES {
        return Err(fail(format!("{size} bytes is too short for a table")));
    }
    let mut magic = [0; 8];
    read_exact_at(&file, size - magic.len() as u64, &mut magic)
        .map_err(|error| fail(error.to_string()))?;
    if magic != TABLE_MAGIC {
        return Err(fail("not a table file".to_owned()));
    }
    let size = usize::try_from(size).map_err(|_| fail(format!("{size} bytes is too large")))?;
    let tracked = TrackedFile::new(file, name, Arc::clone(health));
    Table::new(options.clone(), Box::new(tracked), size)
        .map_err(|status| fail(status_text(&status)))
}

fn not_found(folder: &Path) -> OpenError {
    OpenError::NotFound {
        folder: folder.to_path_buf(),
    }
}

fn unreadable(folder: &Path, cause: String) -> OpenError {
    OpenError::Unreadable {
        folder: folder.to_path_buf(),
        cause,
    }
}

/// A table failure as text. `Status` is described by hand: its `Display` never returns in
/// `sstable` 0.11.
fn status_text(status: &Status) -> String {
    format!("{:?} {}", status.code, status.err)
}
