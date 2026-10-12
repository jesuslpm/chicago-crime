//! Streaming ingestion of a CSV file: source identity, counts and the run log.

mod common;

use std::fs::{self, File};
use std::path::Path;

use common::{csv_with_header, row_with_id, sample_row, with_field};
use crime_dataset::RecordCounts;
use crime_generator::ingest::CsvIngest;
use crime_generator::preflight::create_log;
use sha2::{Digest, Sha256};

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
