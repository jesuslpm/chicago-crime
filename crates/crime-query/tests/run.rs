//! Running queries with the built binary: the count on stdout, the warnings and the stats line on
//! stderr (decision C5), exit codes after warnings, empty results and read errors (decision C6).

mod common;

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Output};

use common::{Fixture, Row, fixture_rows, many_rows, row};
use crime_query::exit_code;

fn query_on(dataset: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-query"));
    command.arg("--dataset").arg(dataset).args(args);
    command.output().expect("the query app can be started")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn assert_completed(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(i32::from(exit_code::COMPLETED)),
        "{:?}",
        stderr_lines(output)
    );
}

/// Asserts that `text` is a table with no rows: the header row and the separator row only.
fn assert_empty_table(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].starts_with("ID  "), "{text}");
    assert!(lines[0].trim_end().ends_with("Location"), "{text}");
    assert!(lines[1].chars().all(|c| c == '-' || c == ' '), "{text}");
}

/// Asserts the stats line `<results> en <t> s. Índices usados: <indexes>. Filtros sin índice:
/// <filters>.`, whatever the time.
fn assert_stats(line: &str, results: &str, indexes: &str, filters: &str) {
    let (head, tail) = line
        .split_once(" s. ")
        .unwrap_or_else(|| panic!("not a stats line: {line}"));
    let (count, seconds) = head
        .rsplit_once(" en ")
        .unwrap_or_else(|| panic!("not a stats line: {line}"));
    assert_eq!(count, results, "{line}");
    let (whole, millis) = seconds
        .split_once(',')
        .unwrap_or_else(|| panic!("seconds with a decimal comma: {line}"));
    assert!(!whole.is_empty() && whole.chars().all(|c| c.is_ascii_digit() || c == '.'));
    assert!(
        millis.len() == 3 && millis.chars().all(|c| c.is_ascii_digit()),
        "{line}"
    );
    assert_eq!(
        tail,
        format!("Índices usados: {indexes}. Filtros sin índice: {filters}.")
    );
}

#[test]
fn count_prints_the_bare_number_and_the_stats_line() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(
        &fixture.output(),
        &["--where", "Primary Type=theft", "--count"],
    );
    assert_completed(&output);
    assert_eq!(stdout(&output), "3\n");
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_stats(&lines[0], "3 resultados", "Primary Type", "ninguno");
}

#[test]
fn the_stats_line_names_every_index_used_and_every_filter() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(
        &fixture.output(),
        &[
            "--where",
            "arrest=false",
            "--where",
            "Primary Type=THEFT",
            "--range",
            "Date=2020-01-01..2020-01-02",
            "--where",
            "Beat=0314",
            "--count",
        ],
    );
    assert_completed(&output);
    assert_eq!(stdout(&output), "3\n");
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_stats(
        &lines[0],
        "3 resultados",
        "Date, Primary Type",
        "Arrest, Beat",
    );
}

#[test]
fn missing_ids_are_warned_and_the_query_completes() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(
        &fixture.output(),
        &[
            "--where", "ID=999", "--where", "ID=10", "--where", "ID=123", "--count",
        ],
    );
    assert_completed(&output);
    assert_eq!(stdout(&output), "1\n");
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], "No existe ningún delito con ID 123.");
    assert_eq!(lines[1], "No existe ningún delito con ID 999.");
    assert_stats(&lines[2], "1 resultado", "ID", "ninguno");
}

#[test]
fn a_missing_case_number_is_warned_and_the_query_completes() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(&fixture.output(), &["--where", "Case Number=JA000000"]);
    assert_completed(&output);
    assert_empty_table(&stdout(&output));
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[0],
        "No existe ningún delito con número de caso JA000000."
    );
    assert_stats(&lines[1], "0 resultados", "Case Number", "ninguno");

    let shared = query_on(
        &fixture.output(),
        &["--where", "Case Number=ja100", "--count"],
    );
    assert_completed(&shared);
    assert_eq!(stdout(&shared), "2\n");
}

#[test]
fn an_empty_result_exits_with_0() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(&fixture.output(), &["--where", "Primary Type=homicide"]);
    assert_completed(&output);
    assert_empty_table(&stdout(&output));
    let lines = stderr_lines(&output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_stats(&lines[0], "0 resultados", "Primary Type", "ninguno");
}

#[test]
fn the_limit_caps_the_count() {
    let fixture = Fixture::with_rows(&many_rows(50));
    let output = query_on(
        &fixture.output(),
        &["--where", "Primary Type=theft", "--limit", "7", "--count"],
    );
    assert_completed(&output);
    assert_eq!(stdout(&output), "7\n");
    assert_stats(
        &stderr_lines(&output)[0],
        "7 resultados",
        "Primary Type",
        "ninguno",
    );
}

#[test]
fn a_filter_ignores_case_like_the_indexes() {
    let rows: Vec<Row> = [
        ("1", "STREET"),
        ("2", "street"),
        ("3", "Street"),
        ("4", "ALLEY"),
    ]
    .iter()
    .map(|&(id, place)| row(&[("ID", id), ("Location Description", place)]))
    .collect();
    let fixture = Fixture::with_rows(&rows);
    for spelling in ["street", "STREET"] {
        let filter = format!("Location Description={spelling}");
        let output = query_on(
            &fixture.output(),
            &[
                "--where",
                "Primary Type=arson",
                "--where",
                &filter,
                "--count",
            ],
        );
        assert_completed(&output);
        assert_eq!(stdout(&output), "3\n", "{spelling}");
        assert_stats(
            &stderr_lines(&output)[0],
            "3 resultados",
            "Primary Type",
            "Location Description",
        );
    }
}

/// Overwrites bytes in the middle of a table file, past the part opening reads, so the damage
/// is found while the query reads records.
fn damage_middle(path: &Path) {
    let size = fs::metadata(path).expect("the table exists").len();
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the table can be opened for writing");
    file.seek(SeekFrom::Start(size / 3)).expect("seekable");
    file.write_all(&vec![0xa5; (size / 3) as usize])
        .expect("the table can be damaged");
}

#[test]
fn a_read_error_while_counting_exits_with_1_without_a_number() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    damage_middle(&fixture.version_dir().join("primary.sst"));

    let output = query_on(
        &fixture.output(),
        &[
            "--where",
            "Primary Type=theft",
            "--where",
            "Arrest=false",
            "--count",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(i32::from(exit_code::REJECTED)),
        "{:?}",
        stderr_lines(&output)
    );
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert_eq!(
        stderr_lines(&output),
        [format!(
            "No se pudo leer el conjunto de datos en {}.",
            fixture.output().display()
        )]
    );
}
