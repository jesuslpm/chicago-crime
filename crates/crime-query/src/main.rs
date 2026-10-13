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

use crime_query::cli::{self, Parsed};
use crime_query::{Dataset, Query, exit_code};

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
    // Interim behavior until the execution task (T4) runs the query and writes its results
    // (T5): a valid query on a valid dataset ends successfully and prints nothing.
    drop((query, dataset));
    ExitCode::from(exit_code::COMPLETED)
}
