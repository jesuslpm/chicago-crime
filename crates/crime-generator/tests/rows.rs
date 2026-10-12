//! Row validation: every row becomes a typed record or a rejection with the exact message.

mod common;

use common::{
    Row, assert_valid, csv_bytes_with, csv_with_header, header_row, read_all, row_with_id,
    sample_record, sample_row, with_field,
};
use crime_generator::ingest::RowOutcome;

/// Reads the header (line 1), a valid row with ID 1 (line 2), `row` (line 3) and a valid row
/// with ID 2 (line 4).
fn outcomes_around(row: Row) -> Vec<RowOutcome> {
    read_all(csv_with_header(&[row_with_id(1), row, row_with_id(2)]))
}

/// Asserts that `row` is rejected with `expected` while its neighbours are still accepted.
fn assert_rejected(row: Row, expected: &str) {
    let outcomes = outcomes_around(row);

    assert_eq!(outcomes.len(), 3, "{outcomes:?}");
    assert_valid(&outcomes[0], 2, 1);
    let rejection = outcomes[1]
        .as_ref()
        .expect_err("the row under test should be rejected");
    assert_eq!(rejection.line, 3);
    assert_eq!(rejection.message(), expected);
    assert_valid(&outcomes[2], 4, 2);
}

/// Asserts the rejection of the sample row with one field replaced.
fn assert_field_rejected(column: &str, value: &[u8], expected: &str) {
    assert_rejected(with_field(sample_row(), column, value), expected);
}

#[test]
fn sample_row_parses_to_the_expected_record() {
    let outcomes = read_all(csv_with_header(&[sample_row()]));

    assert_eq!(outcomes.len(), 1);
    let row = outcomes[0].as_ref().expect("the sample row is valid");
    assert_eq!(row.line, 2);
    assert_eq!(row.record, sample_record());
}

#[test]
fn empty_optional_fields_become_none() {
    let optional = [
        "Location Description",
        "District",
        "Ward",
        "Community Area",
        "X Coordinate",
        "Y Coordinate",
        "Latitude",
        "Longitude",
        "Location",
    ];
    let row = optional
        .iter()
        .fold(sample_row(), |row, column| with_field(row, column, b""));

    let outcomes = read_all(csv_with_header(&[row]));

    let record = &outcomes[0]
        .as_ref()
        .expect("optional fields may be empty")
        .record;
    assert_eq!(record.location_description, None);
    assert_eq!(record.district, None);
    assert_eq!(record.ward, None);
    assert_eq!(record.community_area, None);
    assert_eq!(record.x_coordinate, None);
    assert_eq!(record.y_coordinate, None);
    assert_eq!(record.latitude, None);
    assert_eq!(record.longitude, None);
}

#[test]
fn row_with_another_column_count_is_rejected() {
    let mut short = sample_row();
    short.pop();
    let mut long = sample_row();
    long.push(b"extra".to_vec());

    assert_rejected(short, "Fila 3 omitida: se esperaban 22 columnas y hay 21.");
    assert_rejected(long, "Fila 3 omitida: se esperaban 22 columnas y hay 23.");
}

#[test]
fn id_that_is_not_an_integer_is_rejected() {
    for value in ["abc", "", "14348063.0", " 14348063", "99999999999999999999"] {
        assert_field_rejected(
            "ID",
            value.as_bytes(),
            &format!("Fila 3 omitida: el ID \"{value}\" no es un número entero."),
        );
    }
}

#[test]
fn empty_mandatory_field_is_rejected() {
    let mandatory = [
        "Case Number",
        "Date",
        "Block",
        "IUCR",
        "Primary Type",
        "Description",
        "Arrest",
        "Domestic",
        "Beat",
        "FBI Code",
        "Year",
        "Updated On",
    ];
    for column in mandatory {
        assert_field_rejected(
            column,
            b"",
            &format!("Fila 3 omitida: la columna \"{column}\" es obligatoria y está vacía."),
        );
    }
}

#[test]
fn date_that_does_not_parse_is_rejected() {
    let cases = [
        ("Date", "2026-10-03 00:00:00"),
        ("Date", "02/30/2026 10:00:00 AM"),
        ("Date", "10/03/2026 00:00:00 AM"),
        ("Updated On", "10/10/2026 15:42:19"),
    ];
    for (column, value) in cases {
        assert_field_rejected(
            column,
            value.as_bytes(),
            &format!(
                "Fila 3 omitida, columna \"{column}\": \"{value}\" no es una fecha válida \
                 (formato MM/DD/AAAA hh:mm:ss AM/PM)."
            ),
        );
    }
}

#[test]
fn flag_that_is_not_exactly_true_or_false_is_rejected() {
    let cases = [
        ("Arrest", "TRUE"),
        ("Arrest", "yes"),
        ("Domestic", "1"),
        ("Domestic", "False"),
    ];
    for (column, value) in cases {
        assert_field_rejected(
            column,
            value.as_bytes(),
            &format!(
                "Fila 3 omitida, columna \"{column}\": \"{value}\" no es un valor Sí/No válido \
                 (true/false)."
            ),
        );
    }
}

#[test]
fn number_that_is_not_an_integer_is_rejected() {
    let cases = [
        ("Ward", "20.5"),
        ("Community Area", "forty-two"),
        ("X Coordinate", "1,184,164"),
        ("Y Coordinate", "1862455e0"),
        ("Year", "2026.0"),
    ];
    for (column, value) in cases {
        assert_field_rejected(
            column,
            value.as_bytes(),
            &format!("Fila 3 omitida, columna \"{column}\": \"{value}\" no es un número entero."),
        );
    }
}

#[test]
fn decimal_that_is_not_a_finite_number_is_rejected() {
    let cases = [
        ("Latitude", "NaN"),
        ("Longitude", "inf"),
        ("Longitude", "-infinity"),
        ("Latitude", "41,77"),
    ];
    for (column, value) in cases {
        assert_field_rejected(
            column,
            value.as_bytes(),
            &format!(
                "Fila 3 omitida, columna \"{column}\": \"{value}\" no es un número decimal \
                 válido."
            ),
        );
    }
}

#[test]
fn field_that_is_not_utf8_is_rejected() {
    for column in ["ID", "Block", "Location Description", "Latitude"] {
        assert_field_rejected(
            column,
            b"\xff\xfe broken",
            &format!("Fila 3 omitida, columna \"{column}\": el texto no es UTF-8 válido."),
        );
    }
}

#[test]
fn location_column_is_ignored() {
    for value in [&b"not a location"[..], b"\xff", b""] {
        let outcomes = outcomes_around(with_field(sample_row(), "Location", value));

        assert_valid(&outcomes[1], 3, 14_348_063);
    }
}

#[test]
fn only_the_first_problem_of_a_row_is_reported() {
    let bad_date_and_flag = with_field(
        with_field(sample_row(), "Date", b"yesterday"),
        "Arrest",
        b"maybe",
    );
    assert_rejected(
        bad_date_and_flag,
        "Fila 3 omitida, columna \"Date\": \"yesterday\" no es una fecha válida \
         (formato MM/DD/AAAA hh:mm:ss AM/PM).",
    );

    let mut short_with_bad_id = with_field(sample_row(), "ID", b"abc");
    short_with_bad_id.pop();
    assert_rejected(
        short_with_bad_id,
        "Fila 3 omitida: se esperaban 22 columnas y hay 21.",
    );

    let bad_id_and_empty_case =
        with_field(with_field(sample_row(), "Case Number", b""), "ID", b"x1");
    assert_rejected(
        bad_id_and_empty_case,
        "Fila 3 omitida: el ID \"x1\" no es un número entero.",
    );
}

#[test]
fn rejection_keeps_the_column_and_the_original_row() {
    let row = with_field(sample_row(), "Arrest", b"TRUE");

    let outcomes = outcomes_around(row);

    let rejection = outcomes[1].as_ref().expect_err("Arrest is invalid");
    assert_eq!(rejection.column(), Some("Arrest"));
    assert_eq!(
        rejection.raw,
        "14348063,JK438266,10/03/2026 12:00:00 AM,064XX S ELLIS AVE,1090,ARSON,ATTEMPT ARSON,\
         VEHICLE NON-COMMERCIAL,TRUE,false,0314,003,20,42,09,1184164,1862455,2026,\
         10/10/2026 03:42:19 PM,41.777750747,-87.600396201,\"(41.777750747, -87.600396201)\""
    );
}

#[test]
fn row_numbers_follow_the_starting_line_after_multiline_fields() {
    let multiline = with_field(row_with_id(1), "Description", b"FIRST LINE\nSECOND LINE");
    let bytes = csv_with_header(&[
        multiline,
        with_field(row_with_id(2), "Year", b"later"),
        row_with_id(3),
    ]);

    let outcomes = read_all(bytes);

    assert_valid(&outcomes[0], 2, 1);
    let rejection = outcomes[1].as_ref().expect_err("Year is invalid");
    assert_eq!(rejection.line, 4);
    assert!(rejection.message().starts_with("Fila 4 omitida"));
    assert_valid(&outcomes[2], 5, 3);
}

#[test]
fn row_numbers_are_correct_with_crlf_line_endings() {
    let bytes = csv_bytes_with(
        &[
            header_row(),
            row_with_id(1),
            with_field(row_with_id(2), "Description", b"A\r\nB"),
            row_with_id(3),
        ],
        csv::Terminator::CRLF,
    );

    let outcomes = read_all(bytes);

    assert_valid(&outcomes[0], 2, 1);
    assert_valid(&outcomes[1], 3, 2);
    assert_valid(&outcomes[2], 5, 3);
}

#[test]
fn blank_lines_are_skipped_but_still_counted() {
    for (terminator, ending) in [
        (csv::Terminator::Any(b'\n'), &b"\n"[..]),
        (csv::Terminator::CRLF, b"\r\n"),
    ] {
        let mut bytes = csv_bytes_with(&[header_row(), row_with_id(1)], terminator);
        bytes.extend(ending.repeat(2));
        bytes.extend(csv_bytes_with(&[row_with_id(2)], terminator));

        let outcomes = read_all(bytes);

        assert_eq!(outcomes.len(), 2, "{outcomes:?}");
        assert_valid(&outcomes[0], 2, 1);
        assert_valid(&outcomes[1], 5, 2);
    }
}

#[test]
fn last_row_without_a_line_ending_keeps_its_number() {
    let mut bytes = csv_with_header(&[row_with_id(1), row_with_id(2)]);
    assert_eq!(bytes.pop(), Some(b'\n'));

    let outcomes = read_all(bytes);

    assert_valid(&outcomes[1], 3, 2);
}
