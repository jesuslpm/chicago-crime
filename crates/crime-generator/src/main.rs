//! Command line entry point: parses the options and hands over to the library.

use std::process::ExitCode;

use crime_generator::cli::{self, Config, Parsed};

/// Exit code of a command line usage error, as clap and most CLI tools use.
const USAGE_ERROR: u8 = 2;

fn main() -> ExitCode {
    let args = match cli::parse(std::env::args_os()) {
        Parsed::Run(args) => args,
        Parsed::Help(text) => {
            print!("{text}");
            return ExitCode::SUCCESS;
        }
        Parsed::Invalid(message) => {
            eprintln!("{message}");
            return ExitCode::from(USAGE_ERROR);
        }
    };
    let config = Config::from_args(args, &cli::local_timestamp());
    match crime_generator::run(&config) {
        Ok(_) => ExitCode::SUCCESS,
        // The library already reported the cause on the console and in the log.
        Err(_) => ExitCode::FAILURE,
    }
}
