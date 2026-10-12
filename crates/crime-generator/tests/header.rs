//! Header validation happens before any row is read.

mod common;

use std::io::Cursor;
use std::path::Path;

use common::{Row, assert_valid, csv_bytes, header_row, sample_row};
use crime_generator::Abort;
use crime_generator::ingest::CsvIngest;

/// The abort message produced by a CSV that starts with `header` and has one valid row.
fn header_abort(header: Row) -> String {
    let bytes = csv_bytes(&[header, sample_row()]);
    match CsvIngest::new(Path::new("fixture.csv"), Cursor::new(bytes)) {
        Ok(_) => panic!("the header should be rejected"),
        Err(abort) => abort.to_string(),
    }
}

#[test]
fn renamed_column_aborts_with_the_first_difference() {
    let mut header = header_row();
    header[4] = b"IUCR Code".to_vec();

    assert_eq!(
        header_abort(header),
        "La cabecera no coincide con el esquema: columna 5 esperaba \"IUCR\" y encontró \
         \"IUCR Code\"."
    );
}

#[test]
fn renamed_column_is_reported_as_a_typed_abort() {
    let mut header = header_row();
    header[4] = b"Crime Code".to_vec();
    let bytes = csv_bytes(&[header]);

    let abort = CsvIngest::new(Path::new("fixture.csv"), Cursor::new(bytes))
        .err()
        .expect("the header should be rejected");

    assert_eq!(
        abort,
        Abort::HeaderMismatch {
            column: 5,
            expected: "IUCR".to_owned(),
            found: "Crime Code".to_owned(),
        }
    );
}

#[test]
fn missing_column_is_reported_as_empty() {
    let mut header = header_row();
    header.pop();

    assert_eq!(
        header_abort(header),
        "La cabecera no coincide con el esquema: columna 22 esperaba \"Location\" y encontró \"\"."
    );
}

#[test]
fn extra_column_is_reported_with_an_empty_expectation() {
    let mut extra = header_row();
    extra.push(b"Notes".to_vec());
    let mut empty_extra = header_row();
    empty_extra.push(Vec::new());

    assert_eq!(
        header_abort(extra),
        "La cabecera no coincide con el esquema: columna 23 esperaba \"\" y encontró \"Notes\"."
    );
    assert_eq!(
        header_abort(empty_extra),
        "La cabecera no coincide con el esquema: columna 23 esperaba \"\" y encontró \"\"."
    );
}

#[test]
fn empty_file_has_no_header() {
    let abort = CsvIngest::new(Path::new("fixture.csv"), Cursor::new(Vec::new()))
        .err()
        .expect("an empty file has no header");

    assert_eq!(
        abort.to_string(),
        "La cabecera no coincide con el esquema: columna 1 esperaba \"ID\" y encontró \"\"."
    );
}

#[test]
fn utf8_bom_before_the_header_is_accepted() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(csv_bytes(&[header_row(), sample_row()]));

    let mut ingest = CsvIngest::new(Path::new("fixture.csv"), Cursor::new(bytes))
        .expect("a BOM before the header is not part of the first column name");

    let first = ingest.next_row().expect("reading memory succeeds");
    assert_valid(&first.expect("there is one row"), 2, 14_348_063);
}

#[test]
fn bom_is_only_stripped_from_the_start_of_the_file() {
    let mut header = header_row();
    header[1] = b"\xEF\xBB\xBFCase Number".to_vec();

    assert_eq!(
        header_abort(header),
        "La cabecera no coincide con el esquema: columna 2 esperaba \"Case Number\" y encontró \
         \"\u{feff}Case Number\"."
    );
}
