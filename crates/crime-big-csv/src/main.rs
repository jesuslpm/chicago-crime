//! Command line of the big CSV tool (scenario R13 of the `canopy-db-robustez` PRD).
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | The big CSV was written; stdout says how many records it has. |
//! | 1    | It could not be written; stderr says why, and no file with the output name was left. |
//! | 2    | The command line is not valid; nothing was done. |

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Fabrica el CSV grande del escenario R13: el CSV de referencia con su cabecera una sola vez,
/// seguido de dos copias de todos sus registros con ID y números de caso nuevos.
#[derive(Parser)]
#[command(name = "crime-big-csv", version)]
struct Args {
    /// CSV de referencia (delitos de Chicago).
    #[arg(long, value_name = "CSV")]
    input: PathBuf,
    /// CSV grande que se escribe; no debe existir.
    #[arg(long, value_name = "CSV")]
    output: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match crime_big_csv::build(&args.input, &args.output) {
        Ok(records) => {
            println!(
                "{}",
                crime_big_csv::messages::written(records, &args.output)
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.message());
            ExitCode::from(1)
        }
    }
}
