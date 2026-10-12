//! Command line entry point: parses the options and hands over to the library.
//!
//! Exit codes (decision G3; see [`crime_generator::exit_code`], and the end of `--help`):
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The generation completed and the dataset was published, even with rejected rows or the memory warning. |
//! | 1    | The generation aborted on an error; the previous dataset, if any, is intact. |
//! | 2    | The command line is not valid; nothing was done. |
//! | 130  | The operator cancelled the generation with Ctrl+C; the previous dataset, if any, is intact. |

use std::process::ExitCode;

use crime_generator::cli::{self, Config, Parsed};
use crime_generator::exit_code;

fn main() -> ExitCode {
    let args = match cli::parse(std::env::args_os()) {
        Parsed::Run(args) => args,
        Parsed::Help(text) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Parsed::Invalid(message) => {
            eprintln!("{message}");
            return ExitCode::from(exit_code::USAGE);
        }
    };
    let config = Config::from_args(args, &cli::local_timestamp());
    // The library already reported the outcome on the console and in the log.
    let result = crime_generator::run(&config);
    ExitCode::from(crime_generator::exit_status(&result))
}
