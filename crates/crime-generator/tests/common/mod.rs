//! Fixtures shared by the integration tests: rows are built in code as raw bytes so a test can
//! inject anything, including invalid UTF-8.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::io::Cursor;
use std::path::Path;

use crime_dataset::{CivilDateTime, CrimeRecord, HEADER};
use crime_generator::ingest::{CsvIngest, RowOutcome};

/// One CSV row as raw field bytes.
pub type Row = Vec<Vec<u8>>;

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

/// Parses a datetime the test knows to be valid.
pub fn datetime(text: &str) -> CivilDateTime {
    text.parse()
        .unwrap_or_else(|error| panic!("{text:?} should parse: {error:?}"))
}

/// [`SAMPLE_ROW`] fully typed.
pub fn sample_record() -> CrimeRecord {
    CrimeRecord {
        id: 14_348_063,
        case_number: "JK438266".to_owned(),
        date: datetime("10/03/2026 12:00:00 AM"),
        block: "064XX S ELLIS AVE".to_owned(),
        iucr: "1090".to_owned(),
        primary_type: "ARSON".to_owned(),
        description: "ATTEMPT ARSON".to_owned(),
        location_description: Some("VEHICLE NON-COMMERCIAL".to_owned()),
        arrest: false,
        domestic: false,
        beat: "0314".to_owned(),
        district: Some("003".to_owned()),
        ward: Some(20),
        community_area: Some(42),
        fbi_code: "09".to_owned(),
        x_coordinate: Some(1_184_164),
        y_coordinate: Some(1_862_455),
        year: 2026,
        updated_on: datetime("10/10/2026 03:42:19 PM"),
        latitude: Some(41.777750747),
        longitude: Some(-87.600396201),
    }
}

/// Position of a column in the header, by name.
pub fn column(name: &str) -> usize {
    HEADER
        .iter()
        .position(|&column| column == name)
        .unwrap_or_else(|| panic!("{name:?} is not a column"))
}

pub fn header_row() -> Row {
    HEADER.iter().map(|name| name.as_bytes().to_vec()).collect()
}

pub fn sample_row() -> Row {
    SAMPLE_ROW
        .iter()
        .map(|field| field.as_bytes().to_vec())
        .collect()
}

/// The sample row with another ID.
pub fn row_with_id(id: i64) -> Row {
    with_field(sample_row(), "ID", id.to_string().as_bytes())
}

/// Replaces one field of a row, by column name.
pub fn with_field(mut row: Row, name: &str, value: &[u8]) -> Row {
    row[column(name)] = value.to_vec();
    row
}

/// Serializes rows as CSV with every field quoted and LF line endings, like the source.
pub fn csv_bytes(rows: &[Row]) -> Vec<u8> {
    csv_bytes_with(rows, csv::Terminator::Any(b'\n'))
}

pub fn csv_bytes_with(rows: &[Row], terminator: csv::Terminator) -> Vec<u8> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .terminator(terminator)
        .flexible(true)
        .from_writer(Vec::new());
    for row in rows {
        writer
            .write_record(row)
            .expect("writing to memory succeeds");
    }
    writer.into_inner().expect("flushing to memory succeeds")
}

/// A CSV with the expected header followed by `rows`.
pub fn csv_with_header(rows: &[Row]) -> Vec<u8> {
    let mut all = vec![header_row()];
    all.extend_from_slice(rows);
    csv_bytes(&all)
}

/// Reads every row of an in-memory CSV whose header is valid.
pub fn read_all(bytes: Vec<u8>) -> Vec<RowOutcome> {
    let mut ingest = CsvIngest::new(Path::new("fixture.csv"), Cursor::new(bytes))
        .expect("the fixture header is valid");
    let mut outcomes = Vec::new();
    while let Some(outcome) = ingest.next_row().expect("reading memory succeeds") {
        outcomes.push(outcome);
    }
    outcomes
}

/// Asserts that an outcome is a valid row starting at `line` with the given ID.
pub fn assert_valid(outcome: &RowOutcome, line: u64, id: i64) {
    let row = outcome
        .as_ref()
        .unwrap_or_else(|rejection| panic!("line {line} should be valid: {rejection:?}"));
    assert_eq!((row.line, row.record.id), (line, id));
}
