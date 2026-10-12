//! The run log (decision G1): one file per run, outside the dataset folder, mirrored to the console.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::Abort;
use crate::messages;
use crate::validation::Rejection;

/// Synchronous line logger. Every line is echoed to stderr first and then written to the file, so
/// the console still shows it if the file becomes unwritable.
///
/// Each entry is exactly one physical line, on the console and in the file: carriage returns and
/// line feeds inside it are written as the two-character escapes `\r` and `\n`. This covers both
/// the raw content of a rejected row with multi-line fields and a reason that quotes a value with
/// a line break. Backslashes are kept as they are: the log is meant to be read, not parsed back.
#[derive(Debug)]
pub struct RunLog {
    path: PathBuf,
    file: File,
    echo: bool,
}

impl RunLog {
    /// Creates the log file, which must not exist yet: a run never overwrites an earlier log, the
    /// input CSV or any other file it is pointed at. `echo` mirrors every line to stderr.
    pub fn create(path: &Path, echo: bool) -> io::Result<Self> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            echo,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes one entry as one physical line (see [`RunLog`]). Each line reaches the file before
    /// this returns, so an abort or a crash never loses what was already reported.
    pub fn line(&mut self, text: &str) -> Result<(), Abort> {
        let entry = escape_line_breaks(text);
        if self.echo {
            // The console is a best-effort mirror; the file is the record.
            let _ = writeln!(io::stderr().lock(), "{entry}");
        }
        self.file
            .write_all(format!("{entry}\n").as_bytes())
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

/// `text` with every carriage return and line feed written as `\r` or `\n`.
fn escape_line_breaks(text: &str) -> String {
    text.replace('\r', r"\r").replace('\n', r"\n")
}
