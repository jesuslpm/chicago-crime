//! Checks that run before any row is read. Each failure aborts with its exact message.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Abort;
use crate::log::RunLog;

/// Creates the run log as a new file; without it rejections could not be recorded. An existing
/// file at `path` is refused and left untouched.
pub fn create_log(path: &Path, echo: bool) -> Result<RunLog, Abort> {
    RunLog::create(path, echo).map_err(|_| Abort::LogUnwritable {
        path: path.to_path_buf(),
    })
}

/// Opens the input CSV, which must be an existing, readable file.
pub fn open_input(path: &Path) -> Result<File, Abort> {
    let unreadable = || Abort::InputUnreadable {
        path: path.to_path_buf(),
    };
    let file = File::open(path).map_err(|_| unreadable())?;
    let is_file = file.metadata().map_err(|_| unreadable())?.is_file();
    if is_file { Ok(file) } else { Err(unreadable()) }
}

/// Proves the temporary folder accepts new files by creating and deleting a probe file.
pub fn check_temp_dir(dir: &Path) -> Result<(), Abort> {
    let unusable = || Abort::TempDirUnusable {
        dir: dir.to_path_buf(),
    };
    let probe = dir.join(probe_file_name());
    // `create_new` never touches an existing file, so only our own probe is ever deleted.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| unusable())?;
    let written = file.write_all(b"probe");
    drop(file);
    let removed = fs::remove_file(&probe);
    if written.is_ok() && removed.is_ok() {
        Ok(())
    } else {
        Err(unusable())
    }
}

/// The output folder, if it exists, must be a folder. A missing folder is not created here.
///
/// What the folder holds (a dataset, leftovers, foreign files) is checked by
/// [`OutputFolder::open`](crate::publish::OutputFolder::open) under the output lock, so that no
/// other run can change it in between.
pub fn check_output(output: &Path) -> Result<(), Abort> {
    if output.exists() && !output.is_dir() {
        return Err(Abort::OutputUnwritable {
            dir: output.to_path_buf(),
        });
    }
    Ok(())
}

/// A name no other run or file is expected to use.
fn probe_file_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!(".crime-generator-probe-{}-{nanos}", std::process::id())
}
