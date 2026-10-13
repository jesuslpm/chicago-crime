//! Command line entry point: checks the command line, wires Ctrl+C and hands the run to
//! [`crime_query::run`], which checks the query conditions, opens the dataset and writes the
//! results (decision C6; see [`crime_query::exit_code`] and the end of `--help`):
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The query ran, even with no results. |
//! | 1    | The query was rejected for the dataset or for its conditions, or it aborted. |
//! | 2    | The command line is not valid; nothing was queried. |
//! | 130  | The analyst cancelled the query with Ctrl+C. |

use std::io::{self, BufWriter};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crime_query::cli::{self, Parsed};
use crime_query::{exit_code, messages};

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
    let code = crime_query::run(&options, &cancel, &mut stdout, &mut io::stderr());
    ExitCode::from(code)
}
