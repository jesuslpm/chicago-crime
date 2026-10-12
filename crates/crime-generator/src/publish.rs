//! Provisional publication of the generated tables.
//!
//! Tables are written into a fresh staging folder inside the output folder and renamed into
//! place only once they are complete, so a failed run never leaves a partial table and never
//! touches the existing dataset. The staging folder is deleted on success and on drop.
//!
//! The atomic switch of the whole dataset, the manifest and the cleanup of leftovers from
//! interrupted runs come later (T5); they build on this staging folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Name prefix of the staging folder inside the output folder.
pub const STAGING_DIR_PREFIX: &str = ".crime-generator-staging-";

pub struct Staging {
    output: PathBuf,
    dir: TempDir,
}

impl Staging {
    /// Creates the output folder if it is missing and a new staging folder inside it.
    pub fn create(output: &Path) -> io::Result<Self> {
        fs::create_dir_all(output)?;
        let dir = tempfile::Builder::new()
            .prefix(STAGING_DIR_PREFIX)
            .tempdir_in(output)?;
        Ok(Self {
            output: output.to_path_buf(),
            dir,
        })
    }

    /// Where a dataset file is written before it is published.
    pub fn path(&self, file_name: &str) -> PathBuf {
        self.dir.path().join(file_name)
    }

    /// Moves the finished files into the output folder, replacing files with the same name, and
    /// deletes the staging folder.
    pub fn publish(self, file_names: &[&str]) -> io::Result<()> {
        for file_name in file_names {
            fs::rename(self.path(file_name), self.output.join(file_name))?;
        }
        self.dir.close()
    }
}
