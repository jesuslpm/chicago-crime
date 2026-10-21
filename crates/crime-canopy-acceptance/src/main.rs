//! Command line of the CanopyDB verifier:
//!
//! ```text
//! crime-canopy-acceptance --csv <file> --dataset <output folder> [--reference]
//! crime-canopy-acceptance --compare <folder A> <folder B>
//! ```
//!
//! Both modes print one PASS, FAIL or SKIP line per check and a final result line, and exit with
//! 0 when no check failed (a SKIP counts as neither PASS nor FAIL), 1 when any failed and 2 when
//! the command line is not valid. `--compare` combines with no other option. Progress goes to the
//! standard error.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crime_acceptance::{Options, Report};
use crime_canopy_acceptance::{compare, verify};

const USAGE: &str =
    "usage: crime-canopy-acceptance --csv <file> --dataset <output folder> [--reference]
       crime-canopy-acceptance --compare <folder A> <folder B>

  --csv <file>        the CSV the CanopyDB dataset was generated from
  --dataset <folder>  the CanopyDB generator's output folder (the one that holds CURRENT)
  --reference         also require the exact figures the PRDs measured on the reference CSV
  --compare <A> <B>   compare the entries of two output folders, each of either engine";

/// What the command line asks for.
enum Request {
    Verify {
        csv: PathBuf,
        dataset: PathBuf,
        options: Options,
    },
    Compare {
        a: PathBuf,
        b: PathBuf,
    },
}

fn parse(args: Vec<OsString>) -> Result<Request, String> {
    if args.iter().any(|arg| arg == "--compare") {
        return match <[OsString; 3]>::try_from(args) {
            Ok([flag, a, b]) if flag == "--compare" => Ok(Request::Compare {
                a: a.into(),
                b: b.into(),
            }),
            _ => Err("--compare takes two folders and no other option".to_owned()),
        };
    }
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
        (Some(csv), Some(dataset)) => Ok(Request::Verify {
            csv: csv.into(),
            dataset: dataset.into(),
            options,
        }),
        _ => Err("both --csv and --dataset are required".to_owned()),
    }
}

fn main() -> ExitCode {
    let request = match parse(std::env::args_os().skip(1).collect()) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("{error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let started = Instant::now();
    let mut progress =
        |step: &str| eprintln!("[{:>6.1} s] {step}", started.elapsed().as_secs_f64());
    let report: Report = match request {
        Request::Verify {
            csv,
            dataset,
            options,
        } => verify(&csv, &dataset, options, &mut progress),
        Request::Compare { a, b } => compare(&a, &b, &mut progress),
    };
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
