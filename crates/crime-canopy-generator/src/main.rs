//! Command line entry point of the CanopyDB generator: the options, messages and exit codes of
//! the original generator (decision K1 of the parent PRD), with its own name and description.
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The generation completed and the dataset was published, even with rejected rows or the memory warning. |
//! | 1    | The generation aborted on an error; the previous dataset, if any, is intact. |
//! | 2    | The command line is not valid; nothing was done. |
//! | 130  | The operator cancelled the generation with Ctrl+C; the previous dataset, if any, is intact. |

use std::process::ExitCode;

use crime_canopy_generator::{APP, CanopyStorage};
use crime_dataset::Compression;
use crime_generator::cli::{self, Config, Parsed};
use crime_generator::exit_code;

fn main() -> ExitCode {
    let args = match cli::parse_for(&APP, std::env::args_os()) {
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
    let storage = CanopyStorage::new(config.compression != Compression::None, &config.output);
    // The library already reported the outcome on the console and in the log.
    let result = crime_generator::run_with(&config, &APP, storage);
    ExitCode::from(crime_generator::exit_status_of(&result))
}
