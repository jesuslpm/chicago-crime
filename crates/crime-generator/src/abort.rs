//! Errors that stop the whole run (decision D5: only whole-file problems abort).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::messages;

/// Why the run stops. `Display` renders the exact user-facing message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abort {
    /// The input CSV is missing, is not a file, or failed while being read.
    InputUnreadable { path: PathBuf },
    /// The temporary folder is missing, does not accept new files, or failed during the sort.
    TempDirUnusable { dir: PathBuf },
    /// The run log cannot be created or written.
    LogUnwritable { path: PathBuf },
    /// The first record is not the expected header. `column` is 1-based; a missing column is
    /// reported as an empty `found` and an extra column as an empty `expected`.
    HeaderMismatch {
        column: usize,
        expected: String,
        found: String,
    },
    /// The output folder already holds a dataset and overwriting was not allowed.
    DatasetExists,
    /// The output folder cannot be created or written.
    OutputUnwritable { dir: PathBuf },
    /// A folder ran out of space while the run was writing to it.
    InsufficientSpace { dir: PathBuf },
    /// A table received keys out of order: a defect of the generator, never of the data.
    UnorderedKeys { file: PathBuf },
    /// A primary value could not be decoded to build its index entries: a defect of the
    /// generator, because every value was encoded by the same run.
    UndecodableRecord { id: i64 },
    /// An index does not hold one entry per valid record: a defect of the generator.
    IndexCountMismatch {
        index: &'static str,
        entries: u64,
        valid: u64,
    },
}

impl Abort {
    /// A failure of the temporary folder during the sort; running out of space has its own
    /// message.
    pub fn temp_io(dir: &Path, error: &io::Error) -> Self {
        if error.kind() == io::ErrorKind::StorageFull {
            Abort::InsufficientSpace {
                dir: dir.to_path_buf(),
            }
        } else {
            Abort::TempDirUnusable {
                dir: dir.to_path_buf(),
            }
        }
    }

    /// A failure while writing the output folder; running out of space has its own message.
    pub fn output_io(dir: &Path, error: &io::Error) -> Self {
        if error.kind() == io::ErrorKind::StorageFull {
            Abort::InsufficientSpace {
                dir: dir.to_path_buf(),
            }
        } else {
            Abort::OutputUnwritable {
                dir: dir.to_path_buf(),
            }
        }
    }
}

impl fmt::Display for Abort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Abort::InputUnreadable { path } => messages::input_unreadable(path),
            Abort::TempDirUnusable { dir } => messages::temp_dir_unusable(dir),
            Abort::LogUnwritable { path } => messages::log_unwritable(path),
            Abort::HeaderMismatch {
                column,
                expected,
                found,
            } => messages::header_mismatch(*column, expected, found),
            Abort::DatasetExists => messages::dataset_exists(),
            Abort::OutputUnwritable { dir } => messages::output_unwritable(dir),
            Abort::InsufficientSpace { dir } => messages::insufficient_space(dir),
            Abort::UnorderedKeys { file } => messages::unordered_keys(file),
            Abort::UndecodableRecord { id } => messages::undecodable_record(*id),
            Abort::IndexCountMismatch {
                index,
                entries,
                valid,
            } => messages::index_count_mismatch(index, *entries, *valid),
        };
        f.write_str(&message)
    }
}

impl std::error::Error for Abort {}
