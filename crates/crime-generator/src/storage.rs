//! Where a run writes the tables of the dataset.
//!
//! The pipeline in [`crate::generate_with`] reads, validates and sorts the records the same way
//! whatever the storage, and hands the tables to a [`Storage`]: first the primary table in key
//! order, then each index in [`IndexKind::ALL`] order with its keys in order. The storage decides
//! the files of the version folder, the manifest that describes them and the files the final
//! summary lists. [`SstStorage`] writes the LevelDB tables of the original generator (decision
//! D1).

use std::path::{Path, PathBuf};

use crime_dataset::{
    Compression, IndexInfo, IndexKind, Manifest, PRIMARY_FILE_NAME, RecordCounts, SCHEMA_VERSION,
    SourceInfo, TableInfo,
};

use crate::Abort;
use crate::index::IndexTable;
use crate::table::{TableError, TableStats, TableWriter};

/// The parts of a manifest that do not depend on the storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestInfo {
    /// When the dataset was generated, as RFC 3339 local time to the second.
    pub generated_at: String,
    pub source: SourceInfo,
    pub records: RecordCounts,
}

/// Receives the tables of one run, in order.
///
/// The calls always come in this order: [`begin`](Storage::begin), every primary entry with
/// [`add_primary`](Storage::add_primary), [`finish_primary`](Storage::finish_primary); then, for
/// every index in [`IndexKind::ALL`] order, [`begin_index`](Storage::begin_index), its entries
/// with [`add_index`](Storage::add_index) and [`finish_index`](Storage::finish_index); and last
/// [`finish`](Storage::finish). Keys arrive in strictly increasing byte order within a table. A
/// failure returns the [`Abort`] that stops the run; whatever the storage wrote lives inside the
/// new version folder, which the run deletes.
pub trait Storage {
    /// What the storage reports once every table is written.
    type Written: Written;

    /// Starts writing into the new version folder `dir`. `scratch` is a temporary folder of the
    /// run, inside its sort folder, for any temporary file of the storage.
    fn begin(&mut self, dir: &Path, scratch: &Path) -> Result<(), Abort>;
    fn add_primary(&mut self, key: &[u8], value: &[u8]) -> Result<(), Abort>;
    /// Ends the primary table.
    fn finish_primary(&mut self) -> Result<(), Abort>;
    fn begin_index(&mut self, kind: IndexKind) -> Result<(), Abort>;
    fn add_index(&mut self, key: &[u8]) -> Result<(), Abort>;
    /// Ends the current index table.
    fn finish_index(&mut self) -> Result<(), Abort>;
    /// Ends the writing: every table is complete and ready to be read.
    fn finish(self) -> Result<Self::Written, Abort>;
}

/// The tables a [`Storage`] wrote.
pub trait Written {
    /// Entries of the primary table.
    fn primary_entries(&self) -> u64;
    /// Entries of each index, in [`IndexKind::ALL`] order.
    fn index_entries(&self) -> Vec<(IndexKind, u64)>;
    /// The manifest of the dataset as JSON, without a final line break.
    fn manifest_json(&self, info: &ManifestInfo) -> String;
    /// The files of the version folder other than the manifest, in the order of the final
    /// summary, each with its name as the summary shows it and its size in bytes.
    fn files(&self) -> Vec<(String, u64)>;
}

/// The LevelDB tables of the original generator: `primary.sst` and one `index-<name>.sst` per
/// index, compressed as `compression` says (decision G2).
pub struct SstStorage {
    compression: Compression,
    /// The output folder, named by the messages of a write failure.
    output: PathBuf,
    dir: PathBuf,
    current: Option<(PathBuf, TableWriter)>,
    current_kind: Option<IndexKind>,
    primary: Option<TableStats>,
    indexes: Vec<IndexTable>,
}

impl SstStorage {
    pub fn new(compression: Compression, output: &Path) -> Self {
        Self {
            compression,
            output: output.to_path_buf(),
            dir: PathBuf::new(),
            current: None,
            current_kind: None,
            primary: None,
            indexes: Vec::new(),
        }
    }

    fn abort(&self, path: &Path, error: TableError) -> Abort {
        match error {
            TableError::OutOfOrder { .. } => Abort::UnorderedKeys {
                file: path.to_path_buf(),
            },
            TableError::Io(error) => Abort::output_io(&self.output, &error),
        }
    }

    fn create(&mut self, file_name: &str) -> Result<(), Abort> {
        let path = self.dir.join(file_name);
        let table = TableWriter::create(&path, self.compression)
            .map_err(|error| self.abort(&path, error))?;
        self.current = Some((path, table));
        Ok(())
    }

    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<(), Abort> {
        let (path, table) = self.current.as_mut().expect("a table is open");
        match table.add(key, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                let path = path.clone();
                Err(self.abort(&path, error))
            }
        }
    }

    fn finish_current(&mut self) -> Result<TableStats, Abort> {
        let (path, table) = self.current.take().expect("a table is open");
        table.finish().map_err(|error| self.abort(&path, error))
    }
}

impl Storage for SstStorage {
    type Written = SstWritten;

    /// Creates `primary.sst` right away, before the first entry arrives.
    fn begin(&mut self, dir: &Path, _scratch: &Path) -> Result<(), Abort> {
        self.dir = dir.to_path_buf();
        self.create(PRIMARY_FILE_NAME)
    }

    fn add_primary(&mut self, key: &[u8], value: &[u8]) -> Result<(), Abort> {
        self.add(key, value)
    }

    fn finish_primary(&mut self) -> Result<(), Abort> {
        self.primary = Some(self.finish_current()?);
        Ok(())
    }

    fn begin_index(&mut self, kind: IndexKind) -> Result<(), Abort> {
        self.create(kind.file_name())?;
        self.current_kind = Some(kind);
        Ok(())
    }

    fn add_index(&mut self, key: &[u8]) -> Result<(), Abort> {
        self.add(key, &[])
    }

    fn finish_index(&mut self) -> Result<(), Abort> {
        let stats = self.finish_current()?;
        let kind = self.current_kind.take().expect("an index is open");
        self.indexes.push(IndexTable { kind, stats });
        Ok(())
    }

    fn finish(self) -> Result<SstWritten, Abort> {
        Ok(SstWritten {
            compression: self.compression,
            primary: self.primary.expect("the primary table is finished"),
            indexes: self.indexes,
        })
    }
}

/// The tables [`SstStorage`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SstWritten {
    pub compression: Compression,
    pub primary: TableStats,
    /// One per index, in [`IndexKind::ALL`] order.
    pub indexes: Vec<IndexTable>,
}

impl Written for SstWritten {
    fn primary_entries(&self) -> u64 {
        self.primary.entries
    }

    fn index_entries(&self) -> Vec<(IndexKind, u64)> {
        self.indexes
            .iter()
            .map(|table| (table.kind, table.stats.entries))
            .collect()
    }

    fn manifest_json(&self, info: &ManifestInfo) -> String {
        Manifest {
            schema_version: SCHEMA_VERSION,
            generated_at: info.generated_at.clone(),
            source: info.source.clone(),
            records: info.records,
            compression: self.compression,
            primary: TableInfo {
                file: PRIMARY_FILE_NAME.to_owned(),
                entries: self.primary.entries,
                size_bytes: self.primary.size_bytes,
            },
            indexes: self
                .indexes
                .iter()
                .map(|table| IndexInfo {
                    name: table.kind.name().to_owned(),
                    file: table.kind.file_name().to_owned(),
                    entries: table.stats.entries,
                    size_bytes: table.stats.size_bytes,
                })
                .collect(),
        }
        .to_json()
    }

    fn files(&self) -> Vec<(String, u64)> {
        let mut files = vec![(PRIMARY_FILE_NAME.to_owned(), self.primary.size_bytes)];
        files.extend(
            self.indexes
                .iter()
                .map(|table| (table.kind.file_name().to_owned(), table.stats.size_bytes)),
        );
        files
    }
}
