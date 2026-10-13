//! Command line of the verifier:
//!
//! ```text
//! crime-acceptance --csv <file> --dataset <output folder> [--reference]
//! crime-acceptance select --csv <file> [--where FIELD=VALUE]... [--range Date=FROM..TO]...
//!                         [--ids-out <file>] [--rows-out <file>]
//! ```
//!
//! The verification prints one PASS, FAIL or SKIP line per check and a final result line, and
//! exits with 0 when no check failed (a SKIP counts as neither PASS nor FAIL), 1 when any failed
//! and 2 when the command line is not valid. Progress goes to the standard error.
//!
//! `select` filters the CSV alone (see [`crime_acceptance::select`]): it prints the number of
//! matching rows, and exits with 0, 1 when a file cannot be read or written, or 2 when the
//! command line is not valid.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crime_acceptance::{Options, select, verify};

const USAGE: &str = "usage: crime-acceptance --csv <file> --dataset <output folder> [--reference]
       crime-acceptance select --csv <file> [conditions and outputs]

  --csv <file>        the CSV the dataset was generated from
  --dataset <folder>  the generator's output folder (the one that holds CURRENT)
  --reference         also require the exact figures the PRDs measured on the reference CSV";

/// What the command line asks for.
struct Request {
    csv: PathBuf,
    dataset: PathBuf,
    options: Options,
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Request, String> {
    let mut csv = None;
    let mut dataset = None;
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--csv") => csv = Some(args.next().ok_or("--csv needs a file")?),
            Some("--dataset") => dataset = Some(args.next().ok_or("--dataset needs a folder")?),
            Some("--reference") => options.reference = true,
            _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
        }
    }
    match (csv, dataset) {
        (Some(csv), Some(dataset)) => Ok(Request {
            csv: csv.into(),
            dataset: dataset.into(),
            options,
        }),
        _ => Err("both --csv and --dataset are required".to_owned()),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.peek().and_then(|arg| arg.to_str()) == Some("select") {
        args.next();
        return run_select(args);
    }
    let request = match parse(args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let started = Instant::now();
    let report = verify(
        &request.csv,
        &request.dataset,
        request.options,
        &mut |step| eprintln!("[{:>6.1} s] {step}", started.elapsed().as_secs_f64()),
    );
    print!("{}", report.render());
    eprintln!(
        "[{:>6.1} s] verification finished",
        started.elapsed().as_secs_f64()
    );
    if report.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_select(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let request = match select::parse(args) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}\n\n{}", select::USAGE);
            return ExitCode::from(2);
        }
    };
    match select::run(&request) {
        Ok(summary) => {
            println!("{}", summary.matching);
            eprintln!(
                "{} rows read, {} matching, {} skipped (not 22 fields, not UTF-8 or an ID that \
                 is not an integer)",
                summary.rows, summary.matching, summary.skipped
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}
