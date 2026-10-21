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

use std::io::{self, BufWriter};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crime_query::cli::{self, BatchOptions, Options, Parsed};
use crime_query::{exit_code, messages};

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args_os()) {
        Parsed::Run(options) => Command::Query(options),
        Parsed::Batch(options) => Command::Batch(options),
        Parsed::Help(text) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Parsed::Invalid(message) => {
            eprintln!("{message}");
            return ExitCode::from(exit_code::USAGE);
        }
    };
    // Ctrl+C only raises this flag; the query polls it and ends with exit code 130, dropping a
    // half-written output file. Nothing else is published through the flag, so no ordering
    // with other memory is needed.
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler_flag = Arc::clone(&cancelled);
    if ctrlc::set_handler(move || handler_flag.store(true, Ordering::Relaxed)).is_err() {
        eprintln!("{}", messages::cancel_unavailable());
    }
    let cancel = || cancelled.load(Ordering::Relaxed);
    // Buffered: stdout is line-buffered by default, one write per record line otherwise.
    let mut stdout = BufWriter::new(io::stdout().lock());
    let code = match &command {
        Command::Query(options) => {
            crime_query::run(options, &cancel, &mut stdout, &mut io::stderr())
        }
        Command::Batch(options) => {
            crime_query::run_batch(options, &cancel, &mut stdout, &mut io::stderr())
        }
    };
    ExitCode::from(code)
}

/// What a valid command line asks for.
enum Command {
    Query(Options),
    Batch(BatchOptions),
}
