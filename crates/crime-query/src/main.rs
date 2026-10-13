//! Command line entry point: checks the command line, then the query conditions, then opens the
//! dataset (decision C6; see [`crime_query::exit_code`] and the end of `--help`):
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The query ran, even with no results. |
//! | 1    | The query was rejected for the dataset or for its conditions, or it aborted. |
//! | 2    | The command line is not valid; nothing was queried. |
//! | 130  | The analyst cancelled the query with Ctrl+C. |

use std::process::ExitCode;

use crime_query::cli::{self, Mode, Parsed};
use crime_query::{Dataset, ExecError, Execution, Query, exit_code};

fn main() -> ExitCode {
    let options = match cli::parse(std::env::args_os()) {
        Parsed::Run(options) => options,
        Parsed::Help(text) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Parsed::Invalid(message) => {
            eprintln!("{message}");
            return ExitCode::from(exit_code::USAGE);
        }
    };
    let query = match Query::build(&options.conditions) {
        Ok(query) => query.with_limit(options.limit),
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit_code::REJECTED);
        }
    };
    let dataset = match Dataset::open(&options.dataset) {
        Ok(dataset) => dataset,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit_code::REJECTED);
        }
    };
    // Ctrl+C is wired by the output task (T5); until then nothing cancels.
    let cancel = || false;
    let mut execution = match Execution::start(&dataset, &query, &cancel) {
        Ok(execution) => execution,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit_code::REJECTED);
        }
    };
    for warning in execution.warnings() {
        eprintln!("{warning}");
    }
    let summary = match options.mode {
        Mode::Count => match execution.count() {
            Ok(summary) => {
                println!("{}", summary.count);
                summary
            }
            Err(error) => return failed(&error),
        },
        Mode::Records(_) => {
            // Interim behavior until the output task (T5) writes the records in their format:
            // the query runs and reads every record, and nothing is printed on stdout.
            for record in &mut execution {
                if let Err(error) = record {
                    return failed(&error);
                }
            }
            execution.summary()
        }
    };
    eprintln!("{}", summary.stats_line());
    ExitCode::from(exit_code::COMPLETED)
}

/// Reports why the execution ended early: 1 when a read failed, 130 when it was cancelled.
fn failed(error: &ExecError) -> ExitCode {
    eprintln!("{error}");
    ExitCode::from(match error {
        ExecError::Read(_) => exit_code::REJECTED,
        ExecError::Cancelled => exit_code::CANCELLED,
    })
}
