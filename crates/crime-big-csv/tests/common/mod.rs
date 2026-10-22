//! Fixtures of the big CSV tests: a small reference CSV built in code and runs of the binary.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crime_dataset::HEADER;
use tempfile::TempDir;

/// The first data row of the reference CSV.
const SAMPLE_ROW: [&str; 22] = [
    "14348063",
    "JK438266",
    "10/03/2026 12:00:00 AM",
    "064XX S ELLIS AVE",
    "1090",
    "ARSON",
    "ATTEMPT ARSON",
    "VEHICLE NON-COMMERCIAL",
    "false",
    "false",
    "0314",
    "003",
    "20",
    "42",
    "09",
    "1184164",
    "1862455",
    "2026",
    "10/10/2026 03:42:19 PM",
    "41.777750747",
    "-87.600396201",
    "(41.777750747, -87.600396201)",
];

/// The sample row with its own ID, case number and block.
pub fn row(id: i64, case_number: &str) -> Vec<String> {
    let mut row: Vec<String> = SAMPLE_ROW.iter().map(|field| (*field).to_owned()).collect();
    row[0] = id.to_string();
    row[1] = case_number.to_owned();
    row[3] = format!("{id:05}XX S STATE ST");
    row
}

/// The rows of the default fixture: two rows share a case number, one has an ID far above the
/// others, one has a field with quotes and one has an empty optional field.
pub fn fixture_rows() -> Vec<Vec<String>> {
    let mut rows = vec![
        row(10, "JA100001"),
        row(11, "JA100001"),
        row(9_000_000, "HY000005"),
        row(12, "JB999998"),
        row(13, "JA100002"),
        row(14, "JA100003"),
    ];
    rows[3][7] = "\"QUOTED\" PLACE, WITH A COMMA".to_owned();
    rows[4][13] = String::new();
    rows
}

/// A CSV with the expected header followed by `rows`, every field quoted, lines ended by
/// `terminator`.
pub fn csv(rows: &[Vec<String>], terminator: csv::Terminator) -> Vec<u8> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .terminator(terminator)
        .from_writer(Vec::new());
    writer
        .write_record(HEADER)
        .expect("writing to memory succeeds");
    for row in rows {
        writer
            .write_record(row)
            .expect("writing to memory succeeds");
    }
    writer.into_inner().expect("flushing to memory succeeds")
}

/// A folder with a reference CSV and the path of the big CSV, which does not exist yet.
pub struct Scenario {
    pub dir: TempDir,
    pub input: PathBuf,
    pub output: PathBuf,
}

impl Scenario {
    pub fn new(rows: &[Vec<String>]) -> Self {
        Self::with_bytes(&csv(rows, csv::Terminator::Any(b'\n')))
    }

    pub fn with_bytes(content: &[u8]) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let input = dir.path().join("crimes.csv");
        fs::write(&input, content).expect("the fixture can be written");
        let output = dir.path().join("big.csv");
        Self { dir, input, output }
    }

    pub fn run(&self) -> Output {
        run(&self.input, &self.output)
    }
}

/// Runs the binary with `--input` and `--output`.
pub fn run(input: &Path, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crime-big-csv"))
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .output()
        .expect("the binary starts")
}

/// Every record of `path`, header included, as strings.
pub fn records(path: &Path) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .expect("the CSV opens")
        .records()
        .map(|record| {
            record
                .expect("a valid record")
                .iter()
                .map(str::to_owned)
                .collect()
        })
        .collect()
}
