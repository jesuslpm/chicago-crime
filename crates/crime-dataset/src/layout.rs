//! How a published dataset is laid out inside its output folder.
//!
//! ```text
//! <output folder>/
//!     CURRENT                              name of the active version folder and a line feed
//!     dataset-<YYYYMMDD-HHMMSS>-<pid>/     one version folder (a few more may exist briefly)
//!         primary.sst
//!         index-<name>.sst                 one per index
//!         manifest.json
//! ```
//!
//! Windows cannot atomically replace a folder that has files in it, so the dataset is published
//! like LevelDB's `CURRENT` file: every generation writes a complete new version folder that
//! nothing references yet, and then replaces `CURRENT` with a one-line file that names it. A
//! reader that resolves `CURRENT` (see [`resolve_current`]) always lands on a complete version;
//! folders that `CURRENT` does not name are unfinished or replaced versions and must be ignored.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// File in the output folder that names the active version folder.
pub const CURRENT_FILE_NAME: &str = "CURRENT";

/// Every version folder name starts with this prefix and continues with ASCII letters, digits
/// and hyphens only (see [`is_version_dir_name`]).
pub const VERSION_DIR_PREFIX: &str = "dataset-";

/// Whether `name` is a valid version folder name: [`VERSION_DIR_PREFIX`] followed by at least one
/// ASCII letter, digit or hyphen, and nothing else. Such a name can never be a path: it has no
/// separators, no drive and no `.` or `..` component.
pub fn is_version_dir_name(name: &str) -> bool {
    name.strip_prefix(VERSION_DIR_PREFIX).is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

/// Why `CURRENT` does not lead to a version folder.
#[derive(Debug)]
pub enum CurrentError {
    /// There is no `CURRENT` file: the folder holds no published dataset.
    NoDataset,
    /// `CURRENT` exists but cannot be read.
    Unreadable(io::Error),
    /// `CURRENT` does not hold one valid version folder name (see [`is_version_dir_name`]).
    InvalidName(String),
    /// `CURRENT` names a version folder that does not exist or is not a folder.
    MissingVersion(PathBuf),
}

impl fmt::Display for CurrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurrentError::NoDataset => write!(f, "there is no {CURRENT_FILE_NAME} file"),
            CurrentError::Unreadable(error) => {
                write!(f, "{CURRENT_FILE_NAME} cannot be read: {error}")
            }
            CurrentError::InvalidName(content) => write!(
                f,
                "{CURRENT_FILE_NAME} does not name a version folder: {content:?}"
            ),
            CurrentError::MissingVersion(path) => {
                write!(f, "the version folder {} does not exist", path.display())
            }
        }
    }
}

impl std::error::Error for CurrentError {}

/// The active version folder of the dataset published in `output_dir`.
///
/// `CURRENT` must hold exactly one valid version folder name, optionally followed by one line
/// feed (`\n` or `\r\n`), and that folder must exist.
pub fn resolve_current(output_dir: &Path) -> Result<PathBuf, CurrentError> {
    let bytes = match fs::read(output_dir.join(CURRENT_FILE_NAME)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CurrentError::NoDataset);
        }
        Err(error) => return Err(CurrentError::Unreadable(error)),
    };
    let content = String::from_utf8_lossy(&bytes);
    let name = content.strip_suffix('\n').map_or(content.as_ref(), |line| {
        line.strip_suffix('\r').unwrap_or(line)
    });
    if !is_version_dir_name(name) {
        return Err(CurrentError::InvalidName(content.into_owned()));
    }
    let version = output_dir.join(name);
    if version.is_dir() {
        Ok(version)
    } else {
        Err(CurrentError::MissingVersion(version))
    }
}
