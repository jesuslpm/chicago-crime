//! Errors that stop the whole run (decision D5: only whole-file problems abort).

use std::fmt;
use std::path::PathBuf;

use crate::messages;

/// Why the run stops. `Display` renders the exact user-facing message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abort {
    /// The input CSV is missing, is not a file, or failed while being read.
    InputUnreadable { path: PathBuf },
    /// The temporary folder is missing or does not accept new files.
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
        };
        f.write_str(&message)
    }
}

impl std::error::Error for Abort {}
