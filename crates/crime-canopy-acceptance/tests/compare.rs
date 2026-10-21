//! The comparison mode of the CanopyDB verifier (canopy-db-generador PRD, KG8, AC-M3 and AC-KG9):
//! two output folders of either engine, compared table by table and entry by entry.

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use canopydb::{EnvOptions, Environment};
use foundationdb_tuple::pack;
use support::{Entry, Fixture, outcome_lines, run};

const TABLES: [&str; 6] = [
    "primary",
    "date",
    "primary_type",
    "district",
    "community_area",
    "case_number",
];

fn compare(a: &Path, b: &Path) -> (Option<i32>, String) {
    run(&[OsStr::new("--compare"), a.as_os_str(), b.as_os_str()])
}

/// The report lines of six equal tables of the fixture.
fn all_equal() -> Vec<String> {
    TABLES
        .iter()
        .map(|table| {
            format!("PASS  COMPARE-{table}  5 entries on each side, equal keys and values")
        })
        .collect()
}

/// The report lines when only `table` differs, as `summary` says.
fn one_differs(table: &str, summary: &str) -> Vec<String> {
    TABLES
        .iter()
        .map(|name| {
            if *name == table {
                format!("FAIL  COMPARE-{name}  {summary}")
            } else {
                format!("PASS  COMPARE-{name}  5 entries on each side, equal keys and values")
            }
        })
        .collect()
}

#[test]
fn a_canopy_dataset_has_the_same_entries_as_the_sstables_it_was_copied_from() {
    let fixture = Fixture::new();

    let (code, stdout) = compare(&fixture.canopy, &fixture.sstable);

    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(outcome_lines(&stdout), all_equal(), "{stdout}");
    assert!(stdout.ends_with("RESULT: PASS (6 checks)\n"), "{stdout}");

    let (code, stdout) = compare(&fixture.sstable, &fixture.canopy);
    assert_eq!(code, Some(0), "{stdout}");
}

#[test]
fn canopy_datasets_with_and_without_compression_are_equal() {
    let fixture = Fixture::new();
    let uncompressed = fixture.canopy_copy("uncompressed", false, |_, _| {});

    let (code, stdout) = compare(&fixture.canopy, &uncompressed);

    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(outcome_lines(&stdout), all_equal(), "{stdout}");
}

#[test]
fn an_extra_entry_fails_its_table_with_the_entry_counts_and_none_on_the_shorter_side() {
    let fixture = Fixture::new();
    let longer = fixture.canopy_copy("longer", true, |table, entries: &mut Vec<Entry>| {
        if table == "date" {
            entries.push((pack(&(i64::MAX, 1_i64)), Vec::new()));
        }
    });

    let (code, stdout) = compare(&fixture.canopy, &longer);

    assert_eq!(code, Some(1), "{stdout}");
    assert_eq!(
        outcome_lines(&stdout),
        one_differs(
            "date",
            "A 5 entries, B 6 entries; first difference at entry 6: A none, B \
             (Int(9223372036854775807), 1)"
        ),
        "{stdout}"
    );
    assert!(
        stdout.ends_with("RESULT: FAIL (1 of 6 checks failed)\n"),
        "{stdout}"
    );
}

#[test]
fn a_different_key_fails_its_table_with_both_keys_decoded() {
    let fixture = Fixture::new();
    let changed = fixture.canopy_copy("changed-key", true, |table, entries: &mut Vec<Entry>| {
        if table == "district" {
            let last = entries.last_mut().expect("the index has entries");
            last.0 = pack(&("zzz", 1_i64));
        }
    });

    let (code, stdout) = compare(&fixture.sstable, &changed);

    assert_eq!(code, Some(1), "{stdout}");
    assert_eq!(
        outcome_lines(&stdout),
        one_differs(
            "district",
            "A 5 entries, B 5 entries; first difference at entry 5: A (String(\"003\"), \
             14348063), B (String(\"zzz\"), 1)"
        ),
        "{stdout}"
    );
}

#[test]
fn a_different_value_fails_its_table_with_the_key_and_values_differ() {
    let fixture = Fixture::new();
    let changed = fixture.canopy_copy("changed-value", true, |table, entries: &mut Vec<Entry>| {
        if table == "primary" {
            let (_, value) = entries
                .iter_mut()
                .find(|(key, _)| *key == pack(&(900_i64,)))
                .expect("ID 900 is stored");
            value.push(0);
        }
    });

    let (code, stdout) = compare(&fixture.canopy, &changed);

    assert_eq!(code, Some(1), "{stdout}");
    assert_eq!(
        outcome_lines(&stdout),
        one_differs(
            "primary",
            "A 5 entries, B 5 entries; first difference at entry 2: (900,) values differ"
        ),
        "{stdout}"
    );
}

#[test]
fn a_folder_without_a_dataset_gives_a_single_compare_failure() {
    let fixture = Fixture::new();
    let empty = fixture.dir.path().join("empty");
    fs::create_dir(&empty).expect("the folder can be created");

    let (code, stdout) = compare(&fixture.canopy, &empty);

    assert_eq!(code, Some(1), "{stdout}");
    let lines = outcome_lines(&stdout);
    assert_eq!(lines.len(), 1, "{stdout}");
    assert!(lines[0].starts_with("FAIL  COMPARE  B "), "{stdout}");
    assert!(lines[0].contains("CURRENT"), "{stdout}");
    assert!(
        stdout.ends_with("RESULT: FAIL (1 of 1 checks failed)\n"),
        "{stdout}"
    );
}

#[test]
fn a_base_held_open_by_another_process_gives_a_single_compare_failure() {
    let fixture = Fixture::new();
    let base = fixture.canopy_version().join("canopydb");
    let mut options = EnvOptions::new(&base);
    options.page_cache_size = 8 * 1024 * 1024;
    let _held = Environment::with_options(options).expect("the base opens");

    let (code, stdout) = compare(&fixture.sstable, &fixture.canopy);

    assert_eq!(code, Some(1), "{stdout}");
    let lines = outcome_lines(&stdout);
    assert_eq!(lines.len(), 1, "{stdout}");
    assert!(lines[0].starts_with("FAIL  COMPARE  B "), "{stdout}");
    assert!(lines[0].contains("cannot be opened"), "{stdout}");
}

#[test]
fn an_invalid_command_line_exits_with_2() {
    let fixture = Fixture::new();
    let a = fixture.canopy.as_os_str();
    let b = fixture.sstable.as_os_str();
    let csv = fixture.csv.as_os_str();
    let invalid: [&[&OsStr]; 6] = [
        &[],
        &[OsStr::new("--compare"), a],
        &[OsStr::new("--compare"), a, b, OsStr::new("--reference")],
        &[OsStr::new("--csv"), csv, OsStr::new("--compare"), a, b],
        &[OsStr::new("--csv"), csv],
        &[OsStr::new("select"), OsStr::new("--csv"), csv],
    ];
    for args in invalid {
        let (code, stdout) = run(args);
        assert_eq!(code, Some(2), "{args:?}: {stdout}");
        assert!(stdout.is_empty(), "{args:?}: {stdout}");
    }
}
