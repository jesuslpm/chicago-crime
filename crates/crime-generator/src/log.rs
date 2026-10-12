//! The run log (decision G1): one file per run, outside the dataset folder, mirrored to the console.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::Abort;
use crate::messages;
use crate::validation::Rejection;

/// Synchronous line logger. Every line is echoed to stderr first and then written to the file, so
/// the console still shows it if the file becomes unwritable.
#[derive(Debug)]
pub struct RunLog {
    path: PathBuf,
    file: File,
    echo: bool,
}

impl RunLog {
    /// Creates (or truncates) the log file. `echo` mirrors every line to stderr.
    pub fn create(path: &Path, echo: bool) -> io::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            file: File::create(path)?,
            echo,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes one line. Each line reaches the file before this returns, so an abort or a crash
    /// never loses what was already reported.
    pub fn line(&mut self, text: &str) -> Result<(), Abort> {
        if self.echo {
            // The console is a best-effort mirror; the file is the record.
            let _ = writeln!(io::stderr().lock(), "{text}");
        }
        self.file
            .write_all(format!("{text}\n").as_bytes())
            .map_err(|_| Abort::LogUnwritable {
                path: self.path.clone(),
            })
    }

    /// Logs a rejected row with its reason and its original content.
    pub fn rejection(&mut self, rejection: &Rejection) -> Result<(), Abort> {
        self.line(&messages::rejected_row(
            &rejection.message(),
            &rejection.raw,
        ))
    }
}
