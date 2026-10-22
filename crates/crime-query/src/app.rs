//! A query app: its name and help, and how it opens the dataset. The original query app reads
//! LevelDB tables ([`APP`]); the CanopyDB query app shares everything else with it (decision K1
//! of the canopy-db PRD) and brings its own [`App`].

use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::cli::{self, Parsed};
use crate::{Dataset, OpenError, Source, exit_code, messages};

/// Opens the dataset of the output folder `folder` (exactly as the analyst gave it). `cancel` is
/// the Ctrl+C check, for an opening that waits; `stderr` takes the warnings of the opening.
pub type Opener = fn(
    folder: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
) -> Result<Box<dyn Source>, OpenError>;

/// A query app.
pub struct App {
    /// The program name, in the help and its usage lines.
    pub name: &'static str,
    /// The first line of the help.
    pub about: &'static str,
    /// A paragraph added at the end of the help, after the exit codes; empty for none.
    pub help_note: &'static str,
    /// How it opens the dataset.
    pub open: Opener,
}

/// The original query app, over the LevelDB tables of the generator.
pub const APP: App = App {
    name: "crime-query",
    about: "Consulta el conjunto de datos de delitos de Chicago publicado por el generador.",
    help_note: "",
    open: open_tables,
};

fn open_tables(
    folder: &Path,
    _cancel: &dyn Fn() -> bool,
    _stderr: &mut dyn Write,
) -> Result<Box<dyn Source>, OpenError> {
    Ok(Box::new(Dataset::open(folder)?))
}

/// The whole program of `app`: checks the command line, wires Ctrl+C and runs the query or the
/// batch, returning the exit code (decision C6).
pub fn main(app: &App) -> u8 {
    let command = match cli::parse_for(app, std::env::args_os()) {
        Parsed::Run(options) => Command::Query(options),
        Parsed::Batch(options) => Command::Batch(options),
        Parsed::Help(text) => {
            print!("{text}");
            return exit_code::COMPLETED;
        }
        Parsed::Invalid(message) => {
            eprintln!("{message}");
            return exit_code::USAGE;
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
    match &command {
        Command::Query(options) => {
            crate::run_with(app, options, &cancel, &mut stdout, &mut io::stderr())
        }
        Command::Batch(options) => {
            crate::run_batch_with(app, options, &cancel, &mut stdout, &mut io::stderr())
        }
    }
}

/// What a valid command line asks for.
enum Command {
    Query(cli::Options),
    Batch(cli::BatchOptions),
}
