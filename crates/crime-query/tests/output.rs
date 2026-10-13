//! The formats of the results with the built binary (decisions C4 and C5, AC-C8, AC-C11, AC-C16):
//! table, CSV and JSON hold the same records, CSV keeps the source header and text, JSON carries
//! the interpreted query and the dataset version, and warnings and stats stay on stderr.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use common::{Fixture, Row, csv_text, fixture_rows, many_rows, row};
use crime_dataset::HEADER;
use crime_query::exit_code;
use serde_json::Value;

fn query_on(dataset: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-query"));
    command.arg("--dataset").arg(dataset).args(args);
    command.output().expect("the query app can be started")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Runs the query in `format` and checks that it completed.
fn records(dataset: &Path, args: &[&str], format: &str) -> (String, Vec<String>) {
    let mut all = args.to_vec();
    all.extend(["--format", format]);
    let output = query_on(dataset, &all);
    assert_eq!(
        output.status.code(),
        Some(i32::from(exit_code::COMPLETED)),
        "{args:?} {format}: {:?}",
        stderr_lines(&output)
    );
    (stdout(&output), stderr_lines(&output))
}

/// The source header line, every name quoted.
fn source_header() -> String {
    csv_text(&[]).trim_end().to_owned()
}

/// CSV text parsed back: the header and the rows sorted by ID.
fn parse_csv(text: &str) -> (Vec<String>, Vec<Row>) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(text.as_bytes());
    let header = reader
        .headers()
        .expect("a header")
        .iter()
        .map(str::to_owned)
        .collect();
    let mut rows: Vec<Row> = reader
        .records()
        .map(|record| {
            let record = record.expect("a valid CSV record");
            record.iter().map(str::to_owned).collect()
        })
        .collect();
    rows.sort_by_key(id);
    (header, rows)
}

/// The table parsed back by the column positions of the separator row; only for rows whose
/// values fit their columns. Rows sorted by ID.
fn parse_table(text: &str) -> (Vec<String>, Vec<Row>) {
    let mut lines = text.lines();
    let header = lines.next().expect("a header row");
    let separator = lines.next().expect("a separator row");
    let mut columns = Vec::new();
    let mut start = None;
    for (position, character) in separator.char_indices().chain([(separator.len(), ' ')]) {
        match (character, start) {
            ('-', None) => start = Some(position),
            (' ', Some(begin)) => {
                columns.push((begin, position));
                start = None;
            }
            _ => {}
        }
    }
    assert_eq!(columns.len(), 22, "one dash run per column: {separator}");
    let cut = |line: &str| -> Row {
        columns
            .iter()
            .enumerate()
            .map(|(index, &(begin, end))| {
                let end = if index == 21 { line.len() } else { end };
                line.get(begin..end.min(line.len()))
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            })
            .collect()
    };
    let header = cut(header);
    let mut rows: Vec<Row> = lines.map(cut).collect();
    rows.sort_by_key(id);
    (header, rows)
}

fn parse_json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("valid JSON ({error}):\n{text}"))
}

fn id(row: &Row) -> i64 {
    row[0].parse().expect("an integer ID")
}

/// The JSON value a source field should become: numbers, booleans, `null` for empty.
fn typed(name: &str, text: &str) -> Value {
    if text.is_empty() {
        return Value::Null;
    }
    match name {
        "ID" | "Ward" | "Community Area" | "X Coordinate" | "Y Coordinate" | "Year" => {
            Value::from(text.parse::<i64>().expect("an integer"))
        }
        "Latitude" | "Longitude" => Value::from(text.parse::<f64>().expect("a decimal")),
        "Arrest" | "Domestic" => Value::from(text == "true"),
        _ => Value::from(text),
    }
}

/// The JSON results as rows of their values, sorted by ID; checks every record has the 22 keys
/// with values typed as `typed` would type the row they become.
fn json_rows(document: &Value) -> Vec<Row> {
    let results = document["results"].as_array().expect("a results array");
    let mut rows: Vec<Row> = results
        .iter()
        .map(|record| {
            let object = record.as_object().expect("a record object");
            assert_eq!(object.len(), 22, "{record}");
            HEADER
                .iter()
                .map(|&name| {
                    let value = &object[name];
                    let text = match value {
                        Value::Null => String::new(),
                        Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    let expected = typed(name, &text);
                    if name == "Latitude" || name == "Longitude" {
                        assert_eq!(value.as_f64(), expected.as_f64(), "{name} in {record}");
                    } else {
                        assert_eq!(value, &expected, "{name} in {record}");
                    }
                    text
                })
                .collect()
        })
        .collect();
    rows.sort_by_key(id);
    rows
}

/// The fixture rows with these IDs, in ID order.
fn expected_rows(rows: &[Row], ids: &[i64]) -> Vec<Row> {
    let mut expected: Vec<Row> = rows
        .iter()
        .filter(|row| ids.contains(&id(row)))
        .cloned()
        .collect();
    expected.sort_by_key(id);
    expected
}

fn assert_no_warnings_or_stats(text: &str) {
    for marker in ["No existe", "resultado", "Índices usados"] {
        assert!(!text.contains(marker), "{marker:?} in:\n{text}");
    }
}

#[test]
fn table_csv_and_json_hold_the_same_records() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let cases: [(&[&str], &[i64]); 4] = [
        (&["--where", "ID=10"], &[10]),
        (&["--where", "Primary Type=theft"], &[10, 20, 30]),
        (
            &[
                "--where",
                "Primary Type=theft",
                "--range",
                "Date=2020-01-01..2020-01-01",
                "--where",
                "Arrest=false",
            ],
            &[10, 20],
        ),
        (&["--where", "Primary Type=homicide"], &[]),
    ];
    for (args, ids) in cases {
        let expected = expected_rows(&rows, ids);

        let (table, _) = records(&fixture.output(), args, "table");
        let (header, table_rows) = parse_table(&table);
        assert_eq!(header, HEADER, "{args:?}");
        assert_eq!(table_rows, expected, "{args:?} table:\n{table}");

        let (csv, _) = records(&fixture.output(), args, "csv");
        assert_eq!(
            csv.lines().next(),
            Some(source_header().as_str()),
            "{args:?}"
        );
        let (header, csv_rows) = parse_csv(&csv);
        assert_eq!(header, HEADER, "{args:?}");
        assert_eq!(csv_rows, expected, "{args:?} csv:\n{csv}");

        let (json, _) = records(&fixture.output(), args, "json");
        assert_eq!(
            json_rows(&parse_json(&json)),
            expected,
            "{args:?} json:\n{json}"
        );

        for text in [&table, &csv, &json] {
            assert_no_warnings_or_stats(text);
        }
    }
}

#[test]
fn csv_lines_are_the_source_lines() {
    let rows = fixture_rows();
    let fixture = Fixture::with_rows(&rows);
    let (csv, _) = records(&fixture.output(), &["--where", "Case Number=JA100"], "csv");
    let expected = csv_text(&expected_rows(&rows, &[10, 20]));
    let mut lines: Vec<&str> = csv.split_inclusive('\n').collect();
    let mut expected_lines: Vec<&str> = expected.split_inclusive('\n').collect();
    assert_eq!(lines.remove(0), expected_lines.remove(0), "the header line");
    lines.sort_unstable();
    expected_lines.sort_unstable();
    assert_eq!(lines, expected_lines, "byte for byte, '\\n' ended:\n{csv}");
}

/// A record with every optional field empty, mixed case text with a comma and a time that does
/// not exist in Chicago (the spring-forward gap), and one with coordinates and mixed case codes.
fn detail_rows() -> Vec<Row> {
    vec![
        row(&[
            ("ID", "1"),
            ("Case Number", "jA001"),
            ("Date", "03/08/2020 02:30:00 AM"),
            ("Block", "064xx S Ellis Ave"),
            ("Primary Type", "Theft"),
            ("Description", "Attempt Theft, Petty"),
            ("Location Description", ""),
            ("District", ""),
            ("Ward", ""),
            ("Community Area", ""),
            ("X Coordinate", ""),
            ("Y Coordinate", ""),
            ("Latitude", ""),
            ("Longitude", ""),
            ("Location", ""),
        ]),
        row(&[
            ("ID", "2"),
            ("Case Number", "JA002"),
            ("Date", "11/01/2020 01:30:00 AM"),
            ("Primary Type", "THEFT"),
            ("Location Description", "Street"),
            ("FBI Code", "08b"),
            ("Arrest", "true"),
            ("Latitude", "41.5"),
            ("Longitude", "-87.25"),
            ("Location", "(41.5, -87.25)"),
        ]),
    ]
}

#[test]
fn every_format_keeps_the_source_text_and_rebuilds_location() {
    let rows = detail_rows();
    let fixture = Fixture::with_rows(&rows);
    let args = ["--where", "ID=1", "--where", "ID=2"];
    let expected = expected_rows(&rows, &[1, 2]);

    let (csv, _) = records(&fixture.output(), &args, "csv");
    assert_eq!(parse_csv(&csv).1, expected, "{csv}");
    assert!(csv.contains(",\"Attempt Theft, Petty\","), "{csv}");
    assert!(csv.contains("\"03/08/2020 02:30:00 AM\""), "{csv}");
    assert!(csv.contains("\"(41.5, -87.25)\""), "{csv}");

    let (json, _) = records(&fixture.output(), &args, "json");
    let document = parse_json(&json);
    assert_eq!(json_rows(&document), expected, "{json}");
    let first = document["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|record| record["ID"] == 1)
        .expect("ID 1");
    assert_eq!(first["Date"], "03/08/2020 02:30:00 AM");
    assert_eq!(first["Location"], Value::Null);
    assert_eq!(first["Latitude"], Value::Null);
    assert_eq!(first["District"], Value::Null);
    assert_eq!(first["Primary Type"], "Theft");

    let (table, _) = records(&fixture.output(), &args, "table");
    let (_, table_rows) = parse_table(&table);
    assert_eq!(table_rows, expected, "{table}");
}

#[test]
fn json_records_are_keyed_in_csv_order_one_per_line() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let (json, _) = records(
        &fixture.output(),
        &["--where", "Primary Type=theft"],
        "json",
    );
    let lines: Vec<&str> = json.lines().collect();
    assert_eq!(lines.len(), 5, "preamble, three records, closing:\n{json}");
    for line in &lines[1..4] {
        let positions: Vec<usize> = HEADER
            .iter()
            .map(|name| {
                line.find(&format!("\"{name}\":"))
                    .unwrap_or_else(|| panic!("{name} in {line}"))
            })
            .collect();
        assert!(positions.is_sorted(), "keys in CSV order: {line}");
    }
    for (args, expected) in [
        (&["--where", "ID=10"][..], 1),
        (&["--where", "ID=999"][..], 0),
    ] {
        let (json, _) = records(&fixture.output(), args, "json");
        let document = parse_json(&json);
        assert_eq!(document["results"].as_array().map(Vec::len), Some(expected));
    }
}

#[test]
fn json_carries_the_interpreted_query_and_the_dataset_version() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let (json, _) = records(
        &fixture.output(),
        &[
            "--where",
            "Primary Type=THEFT",
            "--range",
            "Date=2020-01-01..2020-01-02",
        ],
        "json",
    );
    let document = parse_json(&json);
    let conditions = document["query"]["conditions"]
        .as_array()
        .expect("a list of conditions");
    assert!(
        conditions.contains(&serde_json::json!({
            "field": "Primary Type", "indexed": true, "values": ["theft"]
        })),
        "{conditions:?}"
    );
    assert!(
        conditions.contains(&serde_json::json!({
            "field": "Date",
            "indexed": true,
            "intervals": [{"from": "2020-01-01T00:00:00", "to": "2020-01-02T23:59:59"}]
        })),
        "{conditions:?}"
    );
    assert_eq!(document["query"]["limit"], Value::Null);

    let version_dir = fixture.version_dir();
    let version = version_dir.file_name().expect("a version folder name");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(version_dir.join("manifest.json")).expect("the manifest"),
    )
    .expect("a JSON manifest");
    assert_eq!(
        document["dataset"],
        serde_json::json!({
            "version": version.to_str().expect("UTF-8"),
            "generated_at": manifest["generated_at"],
            "schema_version": 2,
        })
    );
    assert_eq!(document["results"].as_array().map(Vec::len), Some(3));
}

#[test]
fn a_limit_gives_exactly_n_records_of_the_unlimited_result() {
    let fixture = Fixture::with_rows(&many_rows(50));
    let all: BTreeSet<i64> = (1..=50).collect();
    let args = ["--where", "Primary Type=theft", "--limit", "7"];
    let (csv, _) = records(&fixture.output(), &args, "csv");
    let (table, _) = records(&fixture.output(), &args, "table");
    let (json, _) = records(&fixture.output(), &args, "json");
    let document = parse_json(&json);
    assert_eq!(document["query"]["limit"], 7);
    for (format, rows) in [
        ("csv", parse_csv(&csv).1),
        ("table", parse_table(&table).1),
        ("json", json_rows(&document)),
    ] {
        let ids: BTreeSet<i64> = rows.iter().map(id).collect();
        assert_eq!(rows.len(), 7, "{format}");
        assert_eq!(ids.len(), 7, "{format}: distinct");
        assert!(ids.is_subset(&all), "{format}: {ids:?}");
    }
}

#[test]
fn warnings_come_first_and_the_stats_line_last_on_stderr() {
    let fixture = Fixture::with_rows(&fixture_rows());
    for format in ["table", "csv", "json"] {
        let (out, err) = records(
            &fixture.output(),
            &["--where", "ID=999", "--where", "ID=10"],
            format,
        );
        assert_no_warnings_or_stats(&out);
        assert_eq!(err.len(), 2, "{format}: {err:?}");
        assert_eq!(err[0], "No existe ningún delito con ID 999.");
        assert!(err[1].starts_with("1 resultado en "), "{format}: {err:?}");
    }
}

#[test]
fn a_long_value_shifts_only_its_row_and_is_never_cut() {
    let block = "0123456789XX S A VERY LONG STREET NAME THAT DOES NOT FIT ITS COLUMN";
    let rows = vec![row(&[("ID", "1"), ("Block", block)]), row(&[("ID", "2")])];
    let fixture = Fixture::with_rows(&rows);
    let (table, _) = records(
        &fixture.output(),
        &["--where", "ID=1", "--where", "ID=2"],
        "table",
    );
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 4, "{table}");
    let long = lines
        .iter()
        .find(|line| line.starts_with("1 "))
        .expect("row 1");
    let short = lines
        .iter()
        .find(|line| line.starts_with("2 "))
        .expect("row 2");
    assert!(long.contains(&format!("{block}  1090")), "{long}");
    assert!(long.len() > short.len(), "{long}\n{short}");
    assert_eq!(lines[0].find("IUCR"), short.find("1090"), "{table}");
}
