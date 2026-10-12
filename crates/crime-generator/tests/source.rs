//! Streaming ingestion of a CSV file: source identity, counts and the run log.

mod common;

use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::Path;

use common::{Row, csv_bytes, csv_with_header, row_with_id, sample_row, with_field};
use crime_dataset::RecordCounts;
use crime_generator::ingest::CsvIngest;
use crime_generator::preflight::create_log;
use sha2::{Digest, Sha256};

/// Serves the bytes its inner reader allows and then fails, like a disk error in the middle of a
/// file.
struct FailsAfter<R>(io::Take<R>);

impl<R: Read> Read for FailsAfter<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.read(buf)? {
            0 if !buf.is_empty() => Err(io::Error::other("simulated read failure")),
            read => Ok(read),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Several CSV buffers worth of rows, with one rejection in the middle.
fn large_fixture() -> Vec<u8> {
    let mut rows: Vec<_> = (1..=2_000).map(row_with_id).collect();
    rows[999] = with_field(rows[999].clone(), "Year", b"unknown");
    csv_with_header(&rows)
}

fn write_fixture(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join("crimes.csv");
    fs::write(&path, bytes).expect("the fixture can be written");
    path
}

/// Reads a CSV whose only row is rejected and returns the run log it produced.
fn log_of_rejected_row(row: Row) -> String {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let path = write_fixture(dir.path(), &csv_with_header(&[row]));
    let log_path = dir.path().join("run.log");
    let mut log = create_log(&log_path, false).expect("log");

    let file = File::open(&path).expect("the fixture can be opened");
    let mut ingest = CsvIngest::new(&path, file).expect("the header is valid");
    let next = ingest.next_valid(&mut log).expect("reading succeeds");

    assert!(next.is_none(), "the only row is rejected");
    fs::read_to_string(&log_path).expect("the log can be read")
}

/// The log holds exactly one entry, with no line break inside it.
fn assert_one_physical_line(content: &str) {
    assert_eq!(
        content.matches('\n').count(),
        1,
        "one entry, one line: {content:?}"
    );
    assert!(
        !content.contains('\r'),
        "no carriage return splits the entry: {content:?}"
    );
}

#[test]
fn source_identity_is_computed_while_streaming() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let bytes = large_fixture();
    let path = write_fixture(dir.path(), &bytes);
    let mut log = create_log(&dir.path().join("run.log"), false).expect("log");

    let file = File::open(&path).expect("the fixture can be opened");
    let mut ingest = CsvIngest::new(&path, file).expect("the header is valid");
    let mut valid = 0;
    while ingest
        .next_valid(&mut log)
        .expect("reading succeeds")
        .is_some()
    {
        valid += 1;
    }
    let counts = ingest.counts();
    let source = ingest.finish().expect("the source can be finished");

    assert_eq!(valid, 1_999);
    assert_eq!(
        counts,
        RecordCounts {
            valid: 1_999,
            rejected: 1
        }
    );
    assert_eq!(source.file_name, "crimes.csv");
    assert_eq!(source.size_bytes, bytes.len() as u64);
    assert_eq!(source.sha256, sha256_hex(&bytes));
}

#[test]
fn source_identity_covers_the_whole_file_even_if_rows_are_left_unread() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let bytes = large_fixture();
    let path = write_fixture(dir.path(), &bytes);

    let file = File::open(&path).expect("the fixture can be opened");
    let ingest = CsvIngest::new(&path, file).expect("the header is valid");
    let source = ingest.finish().expect("the source can be finished");

    assert_eq!(source.size_bytes, bytes.len() as u64);
    assert_eq!(source.sha256, sha256_hex(&bytes));
}

#[test]
fn sha256_of_a_known_text_matches_the_published_vector() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let path = write_fixture(dir.path(), b"abc");

    let file = File::open(&path).expect("the fixture can be opened");
    // "abc" is not a valid header, so read the identity through the hashing reader directly.
    let mut reader = crime_generator::hashing::HashingReader::new(file);
    std::io::copy(&mut reader, &mut std::io::sink()).expect("reading succeeds");
    let fingerprint = reader.finish().expect("finishing succeeds");

    assert_eq!(fingerprint.size_bytes, 3);
    assert_eq!(
        fingerprint.sha256,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn rejections_are_logged_with_their_reason_and_original_row() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let bytes = csv_with_header(&[
        row_with_id(1),
        with_field(sample_row(), "Domestic", b"TRUE"),
        row_with_id(3),
    ]);
    let path = write_fixture(dir.path(), &bytes);
    let log_path = dir.path().join("run.log");
    let mut log = create_log(&log_path, false).expect("log");

    let file = File::open(&path).expect("the fixture can be opened");
    let mut ingest = CsvIngest::new(&path, file).expect("the header is valid");
    let mut lines = Vec::new();
    while let Some(row) = ingest.next_valid(&mut log).expect("reading succeeds") {
        lines.push((row.line, row.record.id));
    }

    assert_eq!(lines, [(2, 1), (4, 3)]);
    let content = fs::read_to_string(&log_path).expect("the log can be read");
    let expected_reason = "Fila 3 omitida, columna \"Domestic\": \"TRUE\" no es un valor Sí/No \
                           válido (true/false).";
    let logged = content
        .lines()
        .find(|line| line.contains(expected_reason))
        .unwrap_or_else(|| panic!("the rejection is logged: {content}"));
    assert!(
        logged.contains("14348063,JK438266,10/03/2026 12:00:00 AM"),
        "the original row is logged: {logged}"
    );
}

#[test]
fn a_rejected_row_with_multiline_fields_is_logged_on_one_physical_line() {
    let multiline = with_field(
        with_field(sample_row(), "Block", b"A\r\nB"),
        "Description",
        b"FIRST LINE\nSECOND LINE",
    );

    let content = log_of_rejected_row(with_field(multiline, "Year", b"later"));

    assert_one_physical_line(&content);
    assert!(
        content.starts_with(
            "Fila 2 omitida, columna \"Year\": \"later\" no es un número entero. \
             Contenido original: "
        ),
        "{content}"
    );
    for escaped in [r#","A\r\nB","#, r#","FIRST LINE\nSECOND LINE","#] {
        assert!(
            content.contains(escaped),
            "{escaped} should be logged: {content}"
        );
    }
}

#[test]
fn a_rejection_quoting_a_multiline_value_is_logged_on_one_physical_line() {
    let row = with_field(sample_row(), "Date", b"10/03/2026\n12:00:00 AM");

    let content = log_of_rejected_row(row);

    assert_one_physical_line(&content);
    let expected_start = "Fila 2 omitida, columna \"Date\": \"10/03/2026\\n12:00:00 AM\" no es una \
                          fecha válida (formato MM/DD/AAAA hh:mm:ss AM/PM). Contenido original: \
                          14348063,JK438266,\"10/03/2026\\n12:00:00 AM\",064XX S ELLIS AVE,";
    assert!(
        content.starts_with(expected_start),
        "reason and raw content keep the escaped value: {content}"
    );
}

#[test]
fn a_read_error_in_the_middle_of_the_file_aborts_instead_of_ending_it() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let bytes = csv_with_header(&[row_with_id(1), row_with_id(2), row_with_id(3)]);
    // The source fails in the middle of the third row, after two complete rows.
    let third_row = csv_bytes(&[row_with_id(3)]).len();
    let readable = (bytes.len() - third_row / 2) as u64;
    let mut log = create_log(&dir.path().join("run.log"), false).expect("log");

    let source = FailsAfter(Cursor::new(bytes).take(readable));
    let mut ingest =
        CsvIngest::new(Path::new("crimes.csv"), source).expect("the header is read in full");
    let mut valid = 0;
    let abort = loop {
        match ingest.next_valid(&mut log) {
            Ok(Some(_)) => valid += 1,
            Ok(None) => panic!("a read error is not the end of the file (after {valid} rows)"),
            Err(abort) => break abort,
        }
    };

    assert_eq!(valid, 2);
    assert_eq!(abort.to_string(), "No se puede leer el archivo crimes.csv.");
}
