//! Command line entry point: checks the command line, wires Ctrl+C and hands the run to
//! [`crime_query::run`], which checks the query conditions, opens the dataset and writes the
//! results, or with `--batch` to [`crime_query::run_batch`], which does the same for each line
//! of a file (decision C6; see [`crime_query::exit_code`] and the end of `--help`):
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The query ran, even with no results; with `--batch`, every query of the batch. |
//! | 1    | The query was rejected for the dataset or for its conditions, or it aborted; with `--batch`, also a batch file that cannot be read, is not UTF-8 or has no query, and a line rejected for its conditions or for repeating an output file. |
//! | 2    | The command line, or a line of the batch, is not valid; nothing was queried. |
//! | 130  | The analyst cancelled the query or the batch with Ctrl+C. |

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(crime_query::main(&crime_query::APP))
}
