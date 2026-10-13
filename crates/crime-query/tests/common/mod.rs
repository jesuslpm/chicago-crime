//! Fixtures shared by the integration tests: small datasets published by the generator library
//! from CSV rows built in code, and helpers to run the built binary and damage tables.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crime_dataset::{CivilDateTime, Compression, HEADER};
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::Config;
use crime_generator::index::DEFAULT_INDEX_SEGMENT_SIZE;
use crime_generator::memory::DEFAULT_MEMORY_LIMIT_MIB;
use crime_generator::preflight::create_log;
use crime_generator::sort::DEFAULT_SEGMENT_SIZE;
use tempfile::TempDir;

/// The first data row of the reference CSV.
pub const SAMPLE_ROW: [&str; 22] = [
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

/// One CSV row as text fields, in header order.
pub type Row = Vec<String>;

/// The sample row with some columns replaced, by name.
pub fn row(changes: &[(&str, &str)]) -> Row {
    let mut fields: Row = SAMPLE_ROW.iter().map(|&field| field.to_owned()).collect();
    for &(name, value) in changes {
        let column = HEADER
            .iter()
            .position(|&column| column == name)
            .unwrap_or_else(|| panic!("{name} is a column"));
        fields[column] = value.to_owned();
    }
    fields
}

/// Seven rows out of ID order with text in mixed case, shared and lowercase case numbers, empty
/// districts and community areas, and dates one second apart around the edges of the ranges
/// the tests ask for. Sorted by date and then ID they are 5, 10, 20, 15, 30, 40, 50.
pub fn fixture_rows() -> Vec<Row> {
    vec![
        row(&[
            ("ID", "30"),
            ("Case Number", "ja300"),
            ("Date", "01/02/2020 11:59:59 PM"),
            ("Primary Type", "theft"),
            ("District", ""),
            ("Community Area", "42"),
        ]),
        row(&[
            ("ID", "10"),
            ("Case Number", "JA100"),
            ("Date", "01/01/2020 12:00:00 AM"),
            ("Primary Type", "THEFT"),
            ("District", "008"),
            ("Community Area", "42"),
        ]),
        row(&[
            ("ID", "50"),
            ("Case Number", "JA500"),
            ("Date", "01/03/2020 12:00:01 AM"),
            ("Primary Type", "Battery"),
            ("District", ""),
            ("Community Area", "7"),
        ]),
        row(&[
            ("ID", "5"),
            ("Case Number", "JA005"),
            ("Date", "12/31/2019 11:59:59 PM"),
            ("Primary Type", "ARSON"),
            ("District", "001"),
            ("Community Area", "1"),
        ]),
        row(&[
            ("ID", "40"),
            ("Case Number", "JA400"),
            ("Date", "01/03/2020 12:00:00 AM"),
            ("Primary Type", "BATTERY"),
            ("District", "008"),
            ("Community Area", ""),
        ]),
        row(&[
            ("ID", "20"),
            ("Case Number", "JA100"),
            ("Date", "01/01/2020 12:00:01 AM"),
            ("Primary Type", "Theft"),
            ("District", "8"),
            ("Community Area", ""),
        ]),
        row(&[
            ("ID", "15"),
            ("Case Number", "JA150"),
            ("Date", "01/02/2020 12:00:00 PM"),
            ("Primary Type", "ASSAULT"),
            ("District", "008"),
            ("Community Area", "42"),
        ]),
    ]
}

/// `count` thefts with IDs `1..=count`, one hour apart from `01/01/2020 12:00:00 AM` on: enough
/// entries for every table to span several blocks.
pub fn many_rows(count: i64) -> Vec<Row> {
    (1..=count)
        .map(|id| {
            let date = hourly_date(id).to_string();
            row(&[
                ("ID", &id.to_string()),
                ("Case Number", &format!("JB{id}")),
                ("Date", &date),
                ("Primary Type", "THEFT"),
                ("Block", &format!("{id:05}XX S STATE ST")),
            ])
        })
        .collect()
}

/// The date of the row `id` of [`many_rows`].
pub fn hourly_date(id: i64) -> CivilDateTime {
    let start = datetime("01/01/2020 12:00:00 AM").seconds();
    CivilDateTime::from_seconds(start + id * 3_600).expect("inside the supported years")
}

/// Parses a datetime the test knows to be valid.
pub fn datetime(text: &str) -> CivilDateTime {
    text.parse()
        .unwrap_or_else(|error| panic!("{text:?} should parse: {error:?}"))
}

/// The CSV text of `rows` under the expected header, written like the source: every non-empty
/// field quoted (a `"` inside doubled), every empty field left bare, `\n` after each line.
pub fn csv_text(rows: &[Row]) -> String {
    let line = |fields: &[String]| {
        let quoted: Vec<String> = fields
            .iter()
            .map(|field| {
                if field.is_empty() {
                    String::new()
                } else {
                    format!("\"{}\"", field.replace('"', "\"\""))
                }
            })
            .collect();
        quoted.join(",") + "\n"
    };
    let header: Row = HEADER.iter().map(|&name| name.to_owned()).collect();
    let mut text = line(&header);
    for row in rows {
        text.push_str(&line(row));
    }
    text
}

/// A temporary folder where the generator publishes datasets into `<dir>/out`.
pub struct Fixture {
    pub dir: TempDir,
    runs: usize,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        fs::create_dir(dir.path().join("tmp")).expect("the temporary folder can be created");
        Self { dir, runs: 0 }
    }

    /// A fixture with `rows` already published.
    pub fn with_rows(rows: &[Row]) -> Self {
        let mut fixture = Self::new();
        fixture.publish(rows);
        fixture
    }

    /// The output folder of the generator: the folder the analyst gives the query app.
    pub fn output(&self) -> PathBuf {
        self.dir.path().join("out")
    }

    /// The version folder that `CURRENT` names, read without the dataset crate.
    pub fn version_dir(&self) -> PathBuf {
        let current = fs::read_to_string(self.output().join("CURRENT")).expect("CURRENT exists");
        self.output().join(current.trim_end())
    }

    /// A fixture with `rows` already published with this block compression.
    pub fn with_rows_compressed(rows: &[Row], compression: Compression) -> Self {
        let mut fixture = Self::new();
        fixture.publish_compressed(rows, compression);
        fixture
    }

    /// Publishes `rows` into the output folder, replacing the dataset already there.
    pub fn publish(&mut self, rows: &[Row]) {
        self.publish_compressed(rows, Compression::Snappy);
    }

    /// [`Fixture::publish`] with this block compression.
    pub fn publish_compressed(&mut self, rows: &[Row], compression: Compression) {
        self.runs += 1;
        let input = self.dir.path().join(format!("crimes-{}.csv", self.runs));
        fs::write(&input, csv_text(rows)).expect("the input can be written");
        let config = Config {
            input,
            output: self.output(),
            temp_dir: self.dir.path().join("tmp"),
            log: self.dir.path().join(format!("run-{}.log", self.runs)),
            overwrite: true,
            compression,
            segment_size: DEFAULT_SEGMENT_SIZE,
            index_segment_size: DEFAULT_INDEX_SEGMENT_SIZE,
            memory_limit_mib: DEFAULT_MEMORY_LIMIT_MIB,
        };
        let mut log = create_log(&config.log, false).expect("the log can be created");
        let result = crime_generator::generate(&config, &mut log, &CancelFlag::new());
        drop(log);
        let content = fs::read_to_string(&config.log).unwrap_or_default();
        if let Err(abort) = result {
            panic!("the generator aborted: {abort}\n{content}");
        }
    }

    /// The log of the last generation.
    pub fn last_log(&self) -> String {
        fs::read_to_string(self.dir.path().join(format!("run-{}.log", self.runs)))
            .expect("the log can be read")
    }
}

/// Every table file of a version folder.
pub const TABLE_FILES: [&str; 6] = [
    "primary.sst",
    "index-date.sst",
    "index-primary_type.sst",
    "index-district.sst",
    "index-community_area.sst",
    "index-case_number.sst",
];

/// Writes a `CURRENT` file naming `name` in `output`.
pub fn write_current(output: &Path, name: &str) {
    fs::write(output.join("CURRENT"), format!("{name}\n")).expect("CURRENT can be written");
}

/// Runs the built binary on `dataset` with `args` after `--dataset`. Only for the integration
/// tests: the crate's unit tests include this file too, and Cargo builds no binary for them.
pub fn query_on(dataset: &Path, args: &[&str]) -> Output {
    let Some(binary) = option_env!("CARGO_BIN_EXE_crime-query") else {
        panic!("only the integration tests have the binary");
    };
    let mut command = Command::new(binary);
    command.arg("--dataset").arg(dataset).args(args);
    command.output().expect("the query app can be started")
}

/// The standard output of a run, which is always UTF-8.
pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

/// The lines of the standard error of a run.
pub fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Overwrites bytes in the middle of a table file, past the part opening reads, so the damage
/// is found while the query reads records.
pub fn damage_middle(path: &Path) {
    let size = fs::metadata(path).expect("the table exists").len();
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the table can be opened for writing");
    file.seek(SeekFrom::Start(size / 3)).expect("seekable");
    file.write_all(&vec![0xa5; (size / 3) as usize])
        .expect("the table can be damaged");
}
