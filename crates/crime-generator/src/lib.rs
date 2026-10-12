//! Generator of the Chicago crime dataset.
//!
//! The binary only parses the command line; every stage lives here so it can be tested:
//!
//! - [`cli`]: options and the resolved [`Config`];
//! - [`preflight`]: checks that run before any row is read;
//! - [`ingest`] and [`validation`]: streaming the CSV into typed records or rejections;
//! - [`hashing`]: the source fingerprint computed during the same read (decision G4);
//! - [`log`]: the per-run log mirrored to the console (decision G1);
//! - [`messages`]: every user-facing text.

mod abort;
pub mod cli;
pub mod hashing;
pub mod ingest;
pub mod log;
pub mod messages;
pub mod preflight;
pub mod validation;

pub use abort::Abort;
pub use cli::Config;

use crime_dataset::{RecordCounts, SourceInfo};

use crate::ingest::CsvIngest;
use crate::log::RunLog;

/// What a completed run read from the CSV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub counts: RecordCounts,
    pub source: SourceInfo,
}

/// Runs the generator: preflight, header, every row through validation, and the reading summary.
///
/// The log is created first so that every later abort is recorded in it as well as on the
/// console. When the log itself cannot be created, the message only reaches the console.
pub fn run(config: &Config) -> Result<RunSummary, Abort> {
    let mut log =
        preflight::create_log(&config.log, true).inspect_err(|abort| eprintln!("{abort}"))?;
    let result = read_source(config, &mut log);
    if let Err(abort) = &result {
        // The console already shows the message even if the log has become unwritable.
        let _ = log.line(&abort.to_string());
    }
    result
}

fn read_source(config: &Config, log: &mut RunLog) -> Result<RunSummary, Abort> {
    let input = preflight::open_input(&config.input)?;
    preflight::check_temp_dir(&config.temp_dir)?;

    let mut ingest = CsvIngest::new(&config.input, input)?;
    while ingest.next_valid(log)?.is_some() {
        // Later stages consume the valid rows; for now they are only counted.
    }
    let counts = ingest.counts();
    let source = ingest.finish()?;

    log.line(&messages::reading_completed(counts))?;
    log.line(&messages::source_fingerprint(&source))?;
    Ok(RunSummary { counts, source })
}
