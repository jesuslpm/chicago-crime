//! The big CSV of scenario R13 (KR3, AC-KR5): the reference CSV, then two copies of its records
//! with new IDs and case numbers, every other field unchanged.

mod common;

use std::collections::{HashMap, HashSet};
use std::fs;

use common::{Scenario, fixture_rows, records, row};
use crime_dataset::HEADER;

fn is_standard(case_number: &str) -> bool {
    let bytes = case_number.as_bytes();
    bytes.len() == 8
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..].iter().all(u8::is_ascii_digit)
}

/// Runs the default fixture and returns the scenario, the original rows and the rows of the output
/// without its header.
fn built() -> (Scenario, Vec<Vec<String>>, Vec<Vec<String>>) {
    let rows = fixture_rows();
    let scenario = Scenario::new(&rows);
    let output = scenario.run();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut all = records(&scenario.output);
    let header = all.remove(0);
    assert_eq!(header, HEADER);
    (scenario, rows, all)
}

#[test]
fn the_header_appears_once_and_the_records_triple() {
    let (scenario, rows, all) = built();
    assert_eq!(all.len(), rows.len() * 3);
    let content = fs::read_to_string(&scenario.output).unwrap();
    let header_line = content.lines().next().unwrap();
    assert_eq!(
        content.lines().filter(|line| *line == header_line).count(),
        1
    );
    assert!(content.ends_with('\n'));
}

#[test]
fn the_original_comes_first_byte_for_byte() {
    let (scenario, rows, all) = built();
    let input = fs::read(&scenario.input).unwrap();
    let output = fs::read(&scenario.output).unwrap();
    assert!(output.starts_with(&input));
    assert_eq!(all[..rows.len()], rows[..]);
}

#[test]
fn every_field_of_the_copies_is_quoted_and_lines_end_like_the_input() {
    let (scenario, rows, _) = built();
    let content = fs::read_to_string(&scenario.output).unwrap();
    let lines: Vec<&str> = content.split_terminator('\n').collect();
    assert_eq!(lines.len(), 1 + rows.len() * 3);
    for line in &lines[1 + rows.len()..] {
        assert!(!line.ends_with('\r'), "{line:?}");
        assert!(
            line.starts_with('"') && line.ends_with('"'),
            "every field is quoted: {line:?}"
        );
        assert!(line.contains("\",\""), "{line:?}");
    }
}

#[test]
fn crlf_input_keeps_crlf_in_the_copies() {
    let rows = fixture_rows();
    let scenario = Scenario::with_bytes(&common::csv(&rows, csv::Terminator::CRLF));
    let output = scenario.run();
    assert_eq!(output.status.code(), Some(0));
    let content = fs::read_to_string(&scenario.output).unwrap();
    let lines: Vec<&str> = content.split_terminator('\n').collect();
    assert_eq!(lines.len(), 1 + rows.len() * 3);
    assert!(lines.iter().all(|line| line.ends_with('\r')));
    assert_eq!(records(&scenario.output).len(), 1 + rows.len() * 3);
}

#[test]
fn copy_ids_are_above_every_original_id_and_unique() {
    let (_, rows, all) = built();
    let max_original = rows
        .iter()
        .map(|row| row[0].parse::<i64>().unwrap())
        .max()
        .unwrap();
    let mut seen = HashSet::new();
    for copy in &all[rows.len()..] {
        let id: i64 = copy[0].parse().expect("a numeric ID");
        assert!(id > max_original, "{id} > {max_original}");
        assert!(seen.insert(id), "ID {id} repeated");
    }
}

#[test]
fn copy_case_numbers_keep_their_letters_and_format_and_are_new_and_unique() {
    let (_, rows, all) = built();
    let originals: HashSet<&str> = rows.iter().map(|row| row[1].as_str()).collect();
    let mut seen = HashSet::new();
    for (index, copy) in all[rows.len()..].iter().enumerate() {
        let source = &rows[index % rows.len()];
        let case_number = copy[1].as_str();
        assert!(is_standard(case_number), "{case_number}");
        assert_eq!(case_number[..2], source[1][..2]);
        assert!(!originals.contains(case_number), "{case_number} is new");
        assert!(seen.insert(case_number), "{case_number} repeated");
    }
}

#[test]
fn a_case_number_shared_in_the_original_appears_only_as_often_as_there() {
    let (_, rows, all) = built();
    let count = |records: &[Vec<String>]| {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for record in records {
            *counts.entry(record[1].clone()).or_default() += 1;
        }
        counts
    };
    let original = count(&rows);
    assert_eq!(original["JA100001"], 2);
    let output = count(&all);
    for (case_number, times) in &output {
        let expected = original.get(case_number).copied().unwrap_or(1);
        assert_eq!(*times, expected, "{case_number}");
    }
}

#[test]
fn every_other_field_of_the_copies_is_unchanged() {
    let (_, rows, all) = built();
    for (index, copy) in all[rows.len()..].iter().enumerate() {
        let source = &rows[index % rows.len()];
        assert_eq!(copy.len(), source.len());
        assert_eq!(copy[2..], source[2..], "copy {index}");
    }
}

#[test]
fn non_standard_case_numbers_get_a_new_unique_value_with_the_same_first_two_characters() {
    let rows = vec![
        row(1, "JA000001"),
        row(2, "JA000001-B"),
        row(3, "X1"),
        row(4, "ja123456"),
        row(5, "HY12"),
    ];
    let scenario = Scenario::new(&rows);
    let output = scenario.run();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut all = records(&scenario.output);
    all.remove(0);
    assert_eq!(all.len(), rows.len() * 3);
    let originals: HashSet<&str> = rows.iter().map(|row| row[1].as_str()).collect();
    let mut seen = HashSet::new();
    for (index, copy) in all[rows.len()..].iter().enumerate() {
        let source = &rows[index % rows.len()][1];
        let case_number = copy[1].as_str();
        let prefix: String = source.chars().take(2).collect();
        assert!(case_number.starts_with(&prefix), "{case_number} / {source}");
        assert!(!originals.contains(case_number), "{case_number} is new");
        assert!(
            seen.insert(case_number.to_owned()),
            "{case_number} repeated"
        );
    }
}

#[test]
fn it_says_how_many_records_it_wrote() {
    let rows: Vec<Vec<String>> = (1..=400).map(|id| row(id, &format!("JC{id:06}"))).collect();
    let scenario = Scenario::new(&rows);
    let output = scenario.run();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "Se han escrito 1.200 registros en {}.\n",
            scenario.output.display()
        )
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn an_existing_output_is_refused_and_left_untouched() {
    let scenario = Scenario::new(&fixture_rows());
    fs::write(&scenario.output, b"previous content").unwrap();
    let output = scenario.run();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(fs::read(&scenario.output).unwrap(), b"previous content");
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "El archivo {} ya existe; no se sobrescribe.\n",
            scenario.output.display()
        )
    );
}

#[test]
fn a_missing_input_ends_with_code_1_and_writes_nothing() {
    let scenario = Scenario::new(&fixture_rows());
    let missing = scenario.dir.path().join("missing.csv");
    let output = common::run(&missing, &scenario.output);
    assert_eq!(output.status.code(), Some(1));
    assert!(!scenario.output.exists());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("No se puede leer el archivo {}.\n", missing.display())
    );
    assert_eq!(
        fs::read_dir(scenario.dir.path()).unwrap().count(),
        1,
        "no temporary file is left"
    );
}

#[test]
fn an_input_with_another_header_is_refused() {
    let mut content = common::csv(&fixture_rows(), csv::Terminator::Any(b'\n'));
    content[1] = b'X';
    let scenario = Scenario::with_bytes(&content);
    let output = scenario.run();
    assert_eq!(output.status.code(), Some(1));
    assert!(!scenario.output.exists());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no tiene la cabecera esperada"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_options_are_a_usage_error() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_crime-big-csv"))
        .output()
        .expect("the binary starts");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn an_input_without_a_final_line_break_still_gives_one_record_per_line() {
    let rows = fixture_rows();
    let mut content = common::csv(&rows, csv::Terminator::Any(b'\n'));
    assert_eq!(content.pop(), Some(b'\n'));
    let scenario = Scenario::with_bytes(&content);
    let output = scenario.run();
    assert_eq!(output.status.code(), Some(0));
    let mut all = records(&scenario.output);
    all.remove(0);
    assert_eq!(all.len(), rows.len() * 3);
    assert_eq!(all[..rows.len()], rows[..]);
}
