//! One run of a valid command line, from the conditions to the exit code, with stdout and stderr
//! given by the caller so the tests can drive it (decisions C5, C6 and C8).
//!
//! The order is fixed: the conditions are checked, the dataset is opened, the output file is
//! prepared, the query starts and its warnings go to stderr; then the results go to stdout or to
//! the output file, and the stats line goes to stderr last.
//!
//! The output file is written to a temporary file in the target's folder, created before the
//! query runs, and renamed over the target only once every result is written, flushed and
//! synced. A rejection, a read or write error or a cancellation drops the temporary file, which
//! deletes it: the target keeps its old content, or does not exist when it did not before.
//!
//! A process killed without unwinding (console closed, `taskkill /F`, power loss) cannot delete
//! it, so its name tells what it is: `.<target name>.XXXXXX.crime-query.tmp` next to the target
//! (see [`TEMPORARY_SUFFIX`]). Such a leftover can be deleted by hand; the run never deletes one
//! itself, since it may belong to another run writing to the same target.

use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use tempfile::NamedTempFile;

use crate::cli::{Format, Mode, Options};
use crate::output::RecordWriter;
use crate::{Dataset, ExecError, Execution, Query, exit_code, messages};

/// The end of the name of every temporary output file, after `.<target name>.` and six random
/// characters.
const TEMPORARY_SUFFIX: &str = ".crime-query.tmp";

/// Runs `options` and returns the exit code. `cancel` is polled while the query runs; once it
/// answers `true` the run ends with [`exit_code::CANCELLED`] (Ctrl+C in the binary).
pub fn run(
    options: &Options,
    cancel: &dyn Fn() -> bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let query = match Query::build(&options.conditions) {
        Ok(query) => query.with_limit(options.limit),
        Err(error) => return report(stderr, error, exit_code::REJECTED),
    };
    let dataset = match Dataset::open(&options.dataset) {
        Ok(dataset) => dataset,
        Err(error) => return report(stderr, error, exit_code::REJECTED),
    };
    let temporary = match &options.output {
        Some(path) => match temporary_next_to(path) {
            Ok(file) => Some((path.as_path(), file)),
            Err(_) => {
                return report(
                    stderr,
                    messages::output_unwritable(path),
                    exit_code::REJECTED,
                );
            }
        },
        None => None,
    };
    let mut execution = match Execution::start(&dataset, &query, cancel) {
        Ok(execution) => execution,
        Err(error) => return failed(stderr, &error),
    };
    for warning in execution.warnings() {
        say(stderr, warning);
    }
    // A Ctrl+C during the existence checks or the warnings ends the run before any output.
    if cancel() {
        return failed(stderr, &ExecError::Cancelled);
    }
    let format = match options.mode {
        Mode::Count => return count(execution, stdout, stderr),
        Mode::Records(format) => format,
    };
    let job = Job {
        format,
        query: &query,
        dataset: &dataset,
    };
    let written = match temporary {
        Some((path, file)) => job.to_file(&mut execution, path, file, cancel, stderr),
        None => job.to_stdout(&mut execution, stdout, stderr),
    };
    match written {
        Ok(()) => {
            say(stderr, execution.summary().stats_line());
            exit_code::COMPLETED
        }
        Err(code) => code,
    }
}

/// What went wrong while the records were written.
enum Failure {
    /// The execution ended early: a read error or a cancellation.
    Execution(ExecError),
    /// The destination refused a write.
    Write(io::Error),
}

/// How and what the records are written.
struct Job<'a> {
    format: Format,
    query: &'a Query,
    dataset: &'a Dataset,
}

impl Job<'_> {
    /// Writes every record of `execution` to `out` and flushes it.
    fn write<W: Write>(&self, execution: &mut Execution, out: W) -> Result<W, Failure> {
        let interpreted = self.query.interpreted();
        let mut writer = RecordWriter::begin(self.format, out, &interpreted, self.dataset)
            .map_err(Failure::Write)?;
        for record in execution {
            let record = record.map_err(Failure::Execution)?;
            writer.write(&record).map_err(Failure::Write)?;
        }
        writer.finish().map_err(Failure::Write)
    }

    /// Writes the records to stdout. `Err` holds the exit code of a run that ends here,
    /// without the stats line.
    fn to_stdout(
        &self,
        execution: &mut Execution,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<(), u8> {
        match self.write(execution, stdout) {
            Ok(_) => Ok(()),
            Err(Failure::Execution(error)) => Err(failed(stderr, &error)),
            Err(Failure::Write(error)) => Err(stdout_failed(stderr, &error)),
        }
    }

    /// Writes the records to `file`, then replaces `path` with it. Every early return drops
    /// `file`, which deletes it. `Err` holds the exit code of a run that ends here.
    fn to_file(
        &self,
        execution: &mut Execution,
        path: &Path,
        mut file: NamedTempFile,
        cancel: &dyn Fn() -> bool,
        stderr: &mut dyn Write,
    ) -> Result<(), u8> {
        let unwritable = |stderr: &mut dyn Write| {
            Err(report(
                stderr,
                messages::output_unwritable(path),
                exit_code::REJECTED,
            ))
        };
        match self.write(execution, BufWriter::new(file.as_file_mut())) {
            Ok(_) => {}
            Err(Failure::Execution(error)) => return Err(failed(stderr, &error)),
            Err(Failure::Write(_)) => return unwritable(stderr),
        }
        // A Ctrl+C that arrives once the records are written still cancels, up to the rename.
        if cancel() {
            return Err(failed(stderr, &ExecError::Cancelled));
        }
        if file.as_file().sync_all().is_err() || file.persist(path).is_err() {
            return unwritable(stderr);
        }
        Ok(())
    }
}

/// Count only: the bare number on stdout, then the stats line.
fn count(execution: Execution, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    let summary = match execution.count() {
        Ok(summary) => summary,
        Err(error) => return failed(stderr, &error),
    };
    if let Err(error) = writeln!(stdout, "{}", summary.count).and_then(|()| stdout.flush()) {
        return stdout_failed(stderr, &error);
    }
    say(stderr, summary.stats_line());
    exit_code::COMPLETED
}

/// The temporary file the output is written to, in the folder of `path` so that the final
/// rename never crosses file systems, named `.<target name>.XXXXXX.crime-query.tmp` so that a
/// leftover is recognizable. A `path` that is a folder is refused here, before the query runs,
/// rather than when the rename fails.
fn temporary_next_to(path: &Path) -> io::Result<NamedTempFile> {
    if path.is_dir() {
        return Err(io::Error::from(io::ErrorKind::IsADirectory));
    }
    let folder = match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    let mut prefix = OsString::from(".");
    prefix.push(path.file_name().unwrap_or(OsStr::new("output")));
    prefix.push(".");
    tempfile::Builder::new()
        .prefix(&prefix)
        .suffix(TEMPORARY_SUFFIX)
        .tempfile_in(folder)
}

/// Ends the run after a failed write on stdout.
///
/// A closed pipe means the consumer (`head`, a pager the analyst quit) wants no more results:
/// the run stops quietly and counts as completed (exit code 0), without the stats line. Any
/// other error is reported and the run fails (exit code 1).
fn stdout_failed(stderr: &mut dyn Write, error: &io::Error) -> u8 {
    if error.kind() == io::ErrorKind::BrokenPipe {
        exit_code::COMPLETED
    } else {
        report(stderr, messages::stdout_unwritable(), exit_code::REJECTED)
    }
}

/// Reports why the execution ended early: 1 when a read failed, 130 when it was cancelled.
fn failed(stderr: &mut dyn Write, error: &ExecError) -> u8 {
    let code = match error {
        ExecError::Read(_) => exit_code::REJECTED,
        ExecError::Cancelled => exit_code::CANCELLED,
    };
    report(stderr, error, code)
}

fn report(stderr: &mut dyn Write, message: impl Display, code: u8) -> u8 {
    say(stderr, message);
    code
}

/// One line on stderr. A failure to write it cannot be reported anywhere, so it is ignored.
fn say(stderr: &mut dyn Write, message: impl Display) {
    let _ = writeln!(stderr, "{message}");
}
