//! Command line of the verifier:
//!
//! ```text
//! crime-acceptance --csv <file> --dataset <output folder> [--reference]
//! ```
//!
//! It prints one PASS or FAIL line per check and a final result line, and exits with 0 when
//! every check passed, 1 when any failed and 2 when the command line is not valid. Progress goes
//! to the standard error.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crime_acceptance::{Options, verify};

const USAGE: &str = "usage: crime-acceptance --csv <file> --dataset <output folder> [--reference]

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
    let request = match parse(std::env::args_os().skip(1)) {
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
