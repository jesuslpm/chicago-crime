//! The CSV mode of the CanopyDB verifier against small CanopyDB datasets, intact and broken on
//! purpose: the content checks are the original verifier's, the layout and manifest checks are
//! the CanopyDB ones (canopy-db-generador PRD, "Informe del verificador").

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crime_acceptance::{Options, Outcome, Report};
use crime_canopy_acceptance::verify;
use foundationdb_tuple::{Element, pack, unpack};
use support::{CRITERIA, Fixture, csv_bytes, fixture_rows, outcome_lines, row, run, tree_names};

fn verify_output(fixture: &Fixture, output: &Path, options: Options) -> Report {
    verify(&fixture.csv, output, options, &mut |_| {})
}

fn failed(report: &Report) -> Vec<&str> {
    report
        .checks
        .iter()
        .filter(|check| check.failed())
        .map(|check| check.criterion.as_str())
        .collect()
}

fn check<'a>(report: &'a Report, criterion: &str) -> &'a crime_acceptance::Check {
    report
        .check(criterion)
        .unwrap_or_else(|| panic!("{criterion} ran:\n{}", report.render()))
}

/// Replaces `from` with `to` in the manifest of the CanopyDB dataset.
fn edit_manifest(fixture: &Fixture, from: &str, to: &str) {
    let path = fixture.canopy_version().join("manifest.json");
    let text = fs::read_to_string(&path).expect("the manifest can be read");
    let edited = text.replacen(from, to, 1);
    assert_ne!(edited, text, "{from:?} is in the manifest:\n{text}");
    fs::write(&path, edited).expect("the manifest can be replaced");
}

#[test]
fn a_canopy_dataset_built_from_its_csv_passes_every_check() {
    let fixture = Fixture::new();

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    let rendered = report.render();
    assert!(report.passed(), "{rendered}");
    let criteria: Vec<&str> = report
        .checks
        .iter()
        .map(|check| check.criterion.as_str())
        .collect();
    assert_eq!(criteria, CRITERIA);
    assert!(
        check(&report, "AC-G1")
            .summary
            .starts_with("primary: 5 entries in strictly increasing ID order"),
        "{rendered}"
    );
    assert!(
        check(&report, "AC-G3").summary.contains("5 CSV rows"),
        "{rendered}"
    );
    assert!(
        check(&report, "AC-G4-SHARED-CASES")
            .summary
            .contains("2 shared case numbers"),
        "{rendered}"
    );
    assert!(
        check(&report, "LAYOUT")
            .summary
            .contains("exactly manifest.json and canopydb/"),
        "{rendered}"
    );

    let (code, stdout) = run(&[
        OsStr::new("--csv"),
        fixture.csv.as_os_str(),
        OsStr::new("--dataset"),
        fixture.canopy.as_os_str(),
    ]);
    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(outcome_lines(&stdout).len(), CRITERIA.len(), "{stdout}");
    assert!(stdout.ends_with("RESULT: PASS (19 checks)\n"), "{stdout}");
}

#[test]
fn a_changed_value_in_the_primary_tree_fails_ac_g3_naming_the_row_and_column() {
    let fixture = Fixture::new();
    let changed = fixture.canopy_copy("changed", true, |table, entries| {
        if table != "primary" {
            return;
        }
        let (_, value) = entries
            .iter_mut()
            .find(|(key, _)| unpack::<(i64,)>(key).is_ok_and(|(id,)| id == 900))
            .expect("ID 900 is stored");
        let mut fields: Vec<Element<'_>> = unpack(value).expect("the value is a tuple");
        fields[3] = Element::String("999XX S NOWHERE".into());
        *value = pack(&fields.as_slice());
    });

    let report = verify_output(&fixture, &changed, Options::default());

    assert_eq!(
        failed(&report),
        ["AC-G3", "AC-G3-SAMPLE"],
        "{}",
        report.render()
    );
    let details = check(&report, "AC-G3").details.join("\n");
    assert!(
        details.contains("ID 900") && details.contains("Block"),
        "{details}"
    );
}

#[test]
fn the_reference_option_requires_the_figures_measured_on_the_reference_csv() {
    let fixture = Fixture::new();

    let report = verify_output(&fixture, &fixture.canopy, Options { reference: true });

    let failures = failed(&report);
    for criterion in [
        "AC-G1",
        "AC-G3-DST",
        "AC-G4-THEFT",
        "AC-G4-COMMUNITY-NIL",
        "AC-G4-SHARED-CASES",
    ] {
        assert!(failures.contains(&criterion), "{}", report.render());
    }
    assert!(check(&report, "AC-G3").passed(), "{}", report.render());

    let (code, stdout) = run(&[
        OsStr::new("--csv"),
        fixture.csv.as_os_str(),
        OsStr::new("--dataset"),
        fixture.canopy.as_os_str(),
        OsStr::new("--reference"),
    ]);
    assert_eq!(code, Some(1), "{stdout}");
    assert!(stdout.contains("FAIL  AC-G4-THEFT  "), "{stdout}");
}

#[test]
fn without_the_reference_missing_sample_categories_are_skipped_and_the_verifier_passes() {
    let fixture = Fixture::with_rows(&[
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Case Number", "JA100"),
            ("Primary Type", "THEFT"),
        ]),
        row(&[("ID", "5000"), ("Case Number", "JA100")]),
    ]);

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    let rendered = report.render();
    assert!(report.passed(), "{rendered}");
    assert_eq!(
        check(&report, "AC-G3-DST").outcome,
        Outcome::Skip,
        "{rendered}"
    );
    let (code, stdout) = run(&[
        OsStr::new("--csv"),
        fixture.csv.as_os_str(),
        OsStr::new("--dataset"),
        fixture.canopy.as_os_str(),
    ]);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("SKIP  AC-G3-DST  "), "{stdout}");
    assert!(
        stdout.contains("RESULT: PASS (19 checks, 1 skipped)"),
        "{stdout}"
    );
}

#[test]
fn another_csv_with_the_same_rows_fails_only_the_source_identity() {
    let fixture = Fixture::new();
    let crlf = fixture.dir.path().join("crimes-crlf.csv");
    fs::write(&crlf, csv_bytes(&fixture_rows(), "\r\n")).expect("the copy can be written");

    let report = verify(&crlf, &fixture.canopy, Options::default(), &mut |_| {});

    assert_eq!(failed(&report), ["MANIFEST-SOURCE"], "{}", report.render());
}

#[test]
fn an_unknown_compression_fails_the_manifest_schema() {
    let fixture = Fixture::new();
    edit_manifest(
        &fixture,
        "\"compression\": \"lz4\"",
        "\"compression\": \"snappy\"",
    );

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-SCHEMA"], "{}", report.render());
    assert_eq!(
        check(&report, "MANIFEST-SCHEMA").details,
        ["the compression must be one of [\"lz4\", \"none\"]"],
        "{}",
        report.render()
    );
}

#[test]
fn another_engine_name_fails_the_manifest_schema() {
    let fixture = Fixture::new();
    edit_manifest(&fixture, "\"name\": \"canopydb\"", "\"name\": \"rocksdb\"");

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-SCHEMA"], "{}", report.render());
    assert_eq!(
        check(&report, "MANIFEST-SCHEMA").details,
        ["the engine must be \"canopydb\""],
        "{}",
        report.render()
    );
}

#[test]
fn fields_out_of_order_fail_the_manifest_schema() {
    let fixture = Fixture::new();
    // `compression` moves from after `records` to after `generated_at`.
    edit_manifest(&fixture, "  \"compression\": \"lz4\",\n", "");
    edit_manifest(
        &fixture,
        "  \"engine\": {",
        "  \"compression\": \"lz4\",\n  \"engine\": {",
    );

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-SCHEMA"], "{}", report.render());
    let details = check(&report, "MANIFEST-SCHEMA").details.join("\n");
    assert!(
        details.starts_with("the fields of the manifest are [\"schema_version\", \"generated_at\", \"compression\", \"engine\""),
        "{details}"
    );
}

#[test]
fn tables_out_of_order_fail_the_manifest_schema_but_not_the_entry_counts() {
    let fixture = Fixture::new();
    edit_manifest(&fixture, "\"name\": \"date\"", "\"name\": \"placeholder\"");
    edit_manifest(&fixture, "\"name\": \"district\"", "\"name\": \"date\"");
    edit_manifest(
        &fixture,
        "\"name\": \"placeholder\"",
        "\"name\": \"district\"",
    );

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-SCHEMA"], "{}", report.render());
    assert_eq!(
        check(&report, "MANIFEST-SCHEMA").details,
        [
            "the tables are [\"primary\", \"district\", \"primary_type\", \"date\", \
             \"community_area\", \"case_number\"], expected [\"primary\", \"date\", \
             \"primary_type\", \"district\", \"community_area\", \"case_number\"]"
        ],
        "{}",
        report.render()
    );
}

#[test]
fn a_wrong_entry_count_fails_the_manifest_tables() {
    let fixture = Fixture::new();
    edit_manifest(
        &fixture,
        "\"name\": \"district\",\n      \"entries\": 5",
        "\"name\": \"district\",\n      \"entries\": 99",
    );

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-TABLES"], "{}", report.render());
    assert_eq!(
        check(&report, "MANIFEST-TABLES").details,
        ["district: manifest 99 entries, tree 5"],
        "{}",
        report.render()
    );
}

#[test]
fn a_file_the_base_lacks_fails_the_manifest_files_but_a_changed_size_does_not() {
    let fixture = Fixture::new();
    edit_manifest(&fixture, "\"size_bytes\": 0\n", "\"size_bytes\": 12345\n");

    let resized = verify_output(&fixture, &fixture.canopy, Options::default());

    assert!(resized.passed(), "{}", resized.render());

    edit_manifest(
        &fixture,
        "  \"files\": [\n",
        "  \"files\": [\n    {\n      \"path\": \"canopydb/WAL-0000000009\",\n      \
         \"size_bytes\": 0\n    },\n",
    );

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-FILES"], "{}", report.render());
    assert_eq!(
        check(&report, "MANIFEST-FILES").details,
        ["canopydb/WAL-0000000009 is in the manifest but not in canopydb/"],
        "{}",
        report.render()
    );
}

#[test]
fn a_stray_file_in_the_version_folder_fails_the_layout_and_the_rest_still_runs() {
    let fixture = Fixture::new();
    fs::write(fixture.canopy_version().join("notes.txt"), "not a table")
        .expect("the file can be added");

    let report = verify_output(&fixture, &fixture.canopy, Options::default());

    assert_eq!(failed(&report), ["LAYOUT"], "{}", report.render());
    assert_eq!(report.checks.len(), CRITERIA.len(), "{}", report.render());
}

#[test]
fn an_sstable_dataset_is_not_a_canopy_dataset_and_layout_is_the_only_row() {
    let fixture = Fixture::new();

    let report = verify_output(&fixture, &fixture.sstable, Options::default());

    assert_eq!(report.checks.len(), 1, "{}", report.render());
    let layout = check(&report, "LAYOUT");
    assert!(layout.failed());
    assert_eq!(layout.summary, "not a CanopyDB dataset");

    let (code, stdout) = run(&[
        OsStr::new("--csv"),
        fixture.csv.as_os_str(),
        OsStr::new("--dataset"),
        fixture.sstable.as_os_str(),
    ]);
    assert_eq!(code, Some(1), "{stdout}");
    assert_eq!(
        outcome_lines(&stdout),
        ["FAIL  LAYOUT  not a CanopyDB dataset"],
        "{stdout}"
    );
    assert!(
        stdout.ends_with("RESULT: FAIL (1 of 1 checks failed)\n"),
        "{stdout}"
    );
}

#[test]
fn a_folder_without_a_dataset_fails_the_layout_alone() {
    let fixture = Fixture::new();
    let empty = fixture.dir.path().join("empty");
    fs::create_dir(&empty).expect("the folder can be created");

    let report = verify_output(&fixture, &empty, Options::default());

    assert_eq!(report.checks.len(), 1, "{}", report.render());
    assert!(check(&report, "LAYOUT").failed(), "{}", report.render());
}

#[test]
fn reading_the_base_twice_adds_no_file_and_leaves_the_manifest_identical() {
    let fixture = Fixture::new();
    let copy = fixture.dir.path().join("copy");
    support::copy_dir(&fixture.canopy, &copy);
    let version = fixture.canopy_version();
    let names_before = tree_names(&version);
    let manifest_before = fs::read(version.join("manifest.json")).expect("the manifest exists");

    for _ in 0..2 {
        let report = verify_output(&fixture, &fixture.canopy, Options::default());
        assert!(report.passed(), "{}", report.render());
    }

    assert_eq!(tree_names(&version), names_before);
    assert_eq!(
        fs::read(version.join("manifest.json")).expect("the manifest exists"),
        manifest_before
    );
    let compared = crime_canopy_acceptance::compare(&fixture.canopy, &copy, &mut |_| {});
    assert!(compared.passed(), "{}", compared.render());
    assert_eq!(compared.checks.len(), 6, "{}", compared.render());
}
