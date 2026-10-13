//! The verifier against small datasets built by the generator from a fixture CSV, intact and
//! then broken on purpose: every defect must turn exactly the checks that cover it into FAIL.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crime_acceptance::{Options, Outcome, Report, verify};
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::{self, Config, Parsed};
use crime_generator::preflight::create_log;
use foundationdb_tuple::{Element, pack, unpack};
use sstable::{Options as TableOptions, SSIterator, Table, TableBuilder};
use tempfile::TempDir;

const HEADER: [&str; 22] = [
    "ID",
    "Case Number",
    "Date",
    "Block",
    "IUCR",
    "Primary Type",
    "Description",
    "Location Description",
    "Arrest",
    "Domestic",
    "Beat",
    "District",
    "Ward",
    "Community Area",
    "FBI Code",
    "X Coordinate",
    "Y Coordinate",
    "Year",
    "Updated On",
    "Latitude",
    "Longitude",
    "Location",
];

/// The first data row of the reference CSV.
const SAMPLE_ROW: [&str; 22] = [
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

/// Every criterion the verifier reports, in order.
const CRITERIA: [&str; 18] = [
    "LAYOUT",
    "MANIFEST-SCHEMA",
    "CSV",
    "MANIFEST-SOURCE",
    "MANIFEST-TABLES",
    "AC-G1",
    "AC-G3",
    "AC-G3-SAMPLE",
    "AC-G3-DST",
    "AC-G4-date",
    "AC-G4-primary_type",
    "AC-G4-district",
    "AC-G4-community_area",
    "AC-G4-case_number",
    "AC-G4-THEFT",
    "AC-G4-COMMUNITY-NIL",
    "AC-G4-SHARED-CASES",
    "AC-C18-LOWERCASE",
];

/// The sample row with some columns replaced, by name.
fn row(changes: &[(&str, &str)]) -> Vec<String> {
    let mut fields: Vec<String> = SAMPLE_ROW.iter().map(|&field| field.to_owned()).collect();
    for &(name, value) in changes {
        let column = HEADER
            .iter()
            .position(|&column| column == name)
            .unwrap_or_else(|| panic!("{name} is a column"));
        fields[column] = value.to_owned();
    }
    fields
}

/// Five rows out of ID order: a theft written `Theft` without Ward or community area, a row
/// without coordinates that shares its case number with it, a row in the spring-forward hour
/// without a district, and another theft whose case number is that one's in lowercase.
fn fixture_rows() -> Vec<Vec<String>> {
    vec![
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Case Number", "JA100"),
            ("Primary Type", "Theft"),
            ("Ward", ""),
            ("Community Area", ""),
        ]),
        row(&[
            ("ID", "900"),
            ("Case Number", "JA100"),
            ("Location Description", ""),
            ("X Coordinate", ""),
            ("Y Coordinate", ""),
            ("Latitude", ""),
            ("Longitude", ""),
            ("Location", ""),
        ]),
        row(&[
            ("ID", "14353993"),
            ("Case Number", "JA200"),
            ("Date", "03/08/2015 02:30:00 AM"),
            ("District", ""),
            ("Year", "2015"),
        ]),
        row(&[
            ("ID", "5000"),
            ("Case Number", "ja200"),
            ("Primary Type", "THEFT"),
            ("Updated On", "11/01/2015 01:30:00 AM"),
            ("Latitude", "41.7"),
            ("Longitude", "-87.68427754"),
        ]),
    ]
}

fn csv_bytes(rows: &[Vec<String>], terminator: &str) -> Vec<u8> {
    let mut text = String::new();
    let header: Vec<String> = HEADER.iter().map(|&name| name.to_owned()).collect();
    for fields in std::iter::once(&header).chain(rows) {
        let quoted: Vec<String> = fields.iter().map(|field| format!("\"{field}\"")).collect();
        text.push_str(&quoted.join(","));
        text.push_str(terminator);
    }
    text.into_bytes()
}

/// A fixture CSV and the dataset the generator built from it.
struct Fixture {
    dir: TempDir,
    csv: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        Self::with_rows(&fixture_rows())
    }

    fn with_rows(rows: &[Vec<String>]) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let csv = dir.path().join("crimes.csv");
        fs::write(&csv, csv_bytes(rows, "\n")).expect("the fixture CSV can be written");
        let output = dir.path().join("dataset");
        let args: Vec<OsString> = vec![
            "crime-generator".into(),
            "--input".into(),
            csv.clone().into(),
            "--output".into(),
            output.clone().into(),
            "--temp-dir".into(),
            dir.path().into(),
            "--log".into(),
            dir.path().join("generator.log").into(),
        ];
        let Parsed::Run(args) = cli::parse(args) else {
            panic!("the generator options are valid");
        };
        let config = Config::from_args(args, "20260101-000000");
        let mut log = create_log(&config.log, false).expect("the log can be created");
        crime_generator::generate(&config, &mut log, &CancelFlag::new())
            .unwrap_or_else(|abort| panic!("the fixture dataset is generated: {abort}"));
        Self { dir, csv, output }
    }

    fn verify(&self) -> Report {
        self.verify_with(&self.csv, Options::default())
    }

    fn verify_with(&self, csv: &Path, options: Options) -> Report {
        verify(csv, &self.output, options, &mut |_| {})
    }

    /// The version folder that `CURRENT` names.
    fn version(&self) -> PathBuf {
        let current = fs::read_to_string(self.output.join("CURRENT")).expect("CURRENT exists");
        self.output.join(current.trim_end())
    }
}

fn read_table(path: &Path) -> Vec<(Vec<u8>, Vec<u8>)> {
    let table = Table::new_from_file(TableOptions::default(), path).expect("the table opens");
    let mut iterator = table.iter();
    let mut entries = Vec::new();
    while let Some(entry) = SSIterator::next(&mut iterator) {
        entries.push(entry);
    }
    entries
}

fn write_table(path: &Path, entries: &[(Vec<u8>, Vec<u8>)]) {
    let file = fs::File::create(path).expect("the table can be replaced");
    let mut builder = TableBuilder::new_no_filter(TableOptions::default(), file);
    for (key, value) in entries {
        builder.add(key, value).expect("the entry can be added");
    }
    builder.finish().expect("the table can be finished");
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

#[test]
fn a_dataset_generated_from_its_csv_passes_every_check() {
    let fixture = Fixture::new();

    let report = fixture.verify();

    assert!(report.passed(), "{}", report.render());
    let criteria: Vec<&str> = report
        .checks
        .iter()
        .map(|check| check.criterion.as_str())
        .collect();
    assert_eq!(criteria, CRITERIA);
    assert!(
        check(&report, "AC-G3").summary.contains("5 CSV rows"),
        "{}",
        report.render()
    );
    assert!(
        check(&report, "AC-G3-DST")
            .summary
            .contains("03/08/2015 02:30:00 AM"),
        "{}",
        report.render()
    );
    assert!(
        check(&report, "AC-G4-SHARED-CASES")
            .summary
            .contains("2 shared case numbers"),
        "{}",
        report.render()
    );
    assert!(
        check(&report, "AC-G4-THEFT").summary.contains('2'),
        "{}",
        report.render()
    );
}

#[test]
fn a_changed_field_in_the_primary_table_fails_ac_g3_naming_the_row_and_column() {
    let fixture = Fixture::new();
    let primary = fixture.version().join("primary.sst");
    let mut entries = read_table(&primary);
    let (_, value) = entries
        .iter_mut()
        .find(|(key, _)| unpack::<(i64,)>(key).is_ok_and(|(id,)| id == 900))
        .expect("ID 900 is stored");
    let changed = {
        let mut fields: Vec<Element<'_>> = unpack(value).expect("the value is a tuple");
        fields[3] = Element::String("999XX S NOWHERE".into());
        pack(&fields.as_slice())
    };
    *value = changed;
    write_table(&primary, &entries);

    let report = fixture.verify();

    let ac_g3 = check(&report, "AC-G3");
    assert!(ac_g3.failed(), "{}", report.render());
    assert!(
        ac_g3.summary.contains("1 mismatched"),
        "{}",
        report.render()
    );
    let details = ac_g3.details.join("\n");
    assert!(
        details.contains("ID 900") && details.contains("Block"),
        "{details}"
    );
    assert!(check(&report, "AC-G1").passed(), "{}", report.render());
    assert!(check(&report, "AC-G4-date").passed(), "{}", report.render());
}

#[test]
fn a_missing_index_entry_fails_that_index_only() {
    let fixture = Fixture::new();
    let district = fixture.version().join("index-district.sst");
    let entries = read_table(&district);
    write_table(&district, &entries[1..]);

    let report = fixture.verify();

    let failures = failed(&report);
    assert!(failures.contains(&"AC-G4-district"), "{}", report.render());
    for criterion in ["AC-G1", "AC-G3", "AC-G4-date", "AC-G4-case_number"] {
        assert!(check(&report, criterion).passed(), "{}", report.render());
    }
}

#[test]
fn an_uppercase_value_in_a_text_index_fails_the_lowercase_check() {
    let fixture = Fixture::new();
    let primary_type = fixture.version().join("index-primary_type.sst");
    let mut uppercased: Vec<(Vec<u8>, Vec<u8>)> = read_table(&primary_type)
        .into_iter()
        .map(|(key, value)| {
            let (text, id): (String, i64) = unpack(&key).expect("a (text, id) key");
            (pack(&(text.to_uppercase(), id)), value)
        })
        .collect();
    // Uppercasing can reorder keys; a table must stay sorted to be written at all.
    uppercased.sort();
    write_table(&primary_type, &uppercased);

    let report = fixture.verify();

    // The rewritten table no longer has the size that the manifest records.
    assert_eq!(
        failed(&report),
        [
            "MANIFEST-TABLES",
            "AC-G4-primary_type",
            "AC-G4-THEFT",
            "AC-C18-LOWERCASE"
        ],
        "{}",
        report.render()
    );
    let lowercase = check(&report, "AC-C18-LOWERCASE");
    assert!(
        lowercase.details.join("\n").contains("primary_type: 5"),
        "{}",
        report.render()
    );
    assert!(check(&report, "AC-G3").passed(), "{}", report.render());
}

#[test]
fn the_lowercase_check_passes_with_zero_values_on_a_dataset_without_records() {
    let fixture = Fixture::with_rows(&[]);

    let report = fixture.verify();

    let lowercase = check(&report, "AC-C18-LOWERCASE");
    assert_eq!(lowercase.outcome, Outcome::Pass, "{}", report.render());
    assert!(
        lowercase
            .summary
            .starts_with("0 text values in the primary_type, district, case_number indexes, 0 not"),
        "{}",
        report.render()
    );
    // Consistent with the other counts over the same empty dataset.
    for criterion in ["AC-G4-THEFT", "AC-G4-COMMUNITY-NIL", "AC-G4-SHARED-CASES"] {
        assert_eq!(
            check(&report, criterion).outcome,
            Outcome::Pass,
            "{criterion}\n{}",
            report.render()
        );
    }
}

#[test]
fn a_dataset_with_schema_version_1_fails_the_manifest_schema() {
    let fixture = Fixture::new();
    let manifest = fixture.version().join("manifest.json");
    let text = fs::read_to_string(&manifest).expect("the manifest can be read");
    let old = text.replace("\"schema_version\": 2", "\"schema_version\": 1");
    assert_ne!(old, text, "the generator writes schema version 2");
    fs::write(&manifest, old).expect("the manifest can be replaced");

    let report = fixture.verify();

    assert_eq!(failed(&report), ["MANIFEST-SCHEMA"], "{}", report.render());
    assert!(
        check(&report, "MANIFEST-SCHEMA")
            .details
            .contains(&"the schema version must be 2".to_owned()),
        "{}",
        report.render()
    );
}

#[test]
fn another_csv_with_the_same_rows_fails_only_the_source_identity() {
    let fixture = Fixture::new();
    let crlf = fixture.dir.path().join("crimes-crlf.csv");
    fs::write(&crlf, csv_bytes(&fixture_rows(), "\r\n")).expect("the copy can be written");

    let report = fixture.verify_with(&crlf, Options::default());

    assert_eq!(failed(&report), ["MANIFEST-SOURCE"], "{}", report.render());
}

#[test]
fn a_current_file_that_names_no_folder_fails_the_layout() {
    let fixture = Fixture::new();
    fs::write(
        fixture.output.join("CURRENT"),
        "dataset-20200101-000000-1\n",
    )
    .expect("CURRENT can be replaced");

    let report = fixture.verify();

    assert!(!report.passed());
    assert_eq!(failed(&report), ["LAYOUT"], "{}", report.render());
}

#[test]
fn a_stray_file_in_the_version_folder_fails_the_layout() {
    let fixture = Fixture::new();
    fs::write(fixture.version().join("notes.txt"), "not a table").expect("the file can be added");

    let report = fixture.verify();

    assert!(failed(&report).contains(&"LAYOUT"), "{}", report.render());
}

/// Rows of a compatible CSV with none of the optional sample categories: no Date in the
/// spring-forward hour, and every row with coordinates and a Ward.
fn rows_without_optional_samples() -> Vec<Vec<String>> {
    vec![
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Case Number", "JA100"),
            ("Primary Type", "THEFT"),
        ]),
        row(&[("ID", "5000"), ("Case Number", "JA100")]),
    ]
}

#[test]
fn without_the_reference_missing_sample_categories_are_skipped_and_the_verifier_passes() {
    let fixture = Fixture::with_rows(&rows_without_optional_samples());

    let report = fixture.verify();

    let rendered = report.render();
    assert!(report.passed(), "{rendered}");
    assert_eq!(
        check(&report, "AC-G3-DST").outcome,
        Outcome::Skip,
        "{rendered}"
    );
    assert!(rendered.contains("SKIP  AC-G3-DST  "), "{rendered}");
    let sample = check(&report, "AC-G3-SAMPLE");
    // The first and last IDs are still compared, so the sample as a whole passes.
    assert_eq!(sample.outcome, Outcome::Pass, "{rendered}");
    for category in ["without coordinates", "without Ward"] {
        assert!(
            sample.summary.contains(&format!("{category}: SKIP")),
            "{rendered}"
        );
    }
    assert!(
        check(&report, "AC-G3")
            .summary
            .contains("3 CSV rows compared"),
        "{rendered}"
    );
    assert!(rendered.contains("RESULT: PASS"), "{rendered}");

    let binary = Command::new(env!("CARGO_BIN_EXE_crime-acceptance"))
        .arg("--csv")
        .arg(&fixture.csv)
        .arg("--dataset")
        .arg(&fixture.output)
        .output()
        .expect("the verifier runs");
    let stdout = String::from_utf8_lossy(&binary.stdout);
    assert_eq!(binary.status.code(), Some(0), "{stdout}");
    assert!(stdout.contains("SKIP  AC-G3-DST  "), "{stdout}");
}

#[test]
fn with_the_reference_missing_sample_categories_fail() {
    let fixture = Fixture::with_rows(&rows_without_optional_samples());

    let report = fixture.verify_with(&fixture.csv, Options { reference: true });

    let rendered = report.render();
    let failures = failed(&report);
    for criterion in ["AC-G3-SAMPLE", "AC-G3-DST"] {
        assert!(failures.contains(&criterion), "{rendered}");
    }
    let sample = check(&report, "AC-G3-SAMPLE");
    for category in ["without coordinates", "without Ward"] {
        assert!(
            sample.summary.contains(&format!("{category}: no such row")),
            "{rendered}"
        );
    }
    assert!(!rendered.contains("SKIP"), "{rendered}");
}

#[test]
fn the_reference_option_requires_the_figures_measured_on_the_reference_csv() {
    let fixture = Fixture::new();

    let report = fixture.verify_with(&fixture.csv, Options { reference: true });

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
}

#[test]
fn the_binary_prints_one_line_per_check_and_exits_with_the_outcome() {
    let fixture = Fixture::new();
    let binary = env!("CARGO_BIN_EXE_crime-acceptance");

    let passed = Command::new(binary)
        .arg("--csv")
        .arg(&fixture.csv)
        .arg("--dataset")
        .arg(&fixture.output)
        .output()
        .expect("the verifier runs");
    let stdout = String::from_utf8_lossy(&passed.stdout);
    assert_eq!(passed.status.code(), Some(0), "{stdout}");
    let outcome_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| {
            ["PASS", "FAIL", "SKIP"]
                .iter()
                .any(|outcome| line.starts_with(outcome))
        })
        .collect();
    assert_eq!(outcome_lines.len(), CRITERIA.len(), "{stdout}");
    assert!(stdout.contains("RESULT: PASS"), "{stdout}");

    fs::remove_file(fixture.version().join("index-date.sst")).expect("the table can be removed");
    let failed = Command::new(binary)
        .args([
            OsString::from("--csv"),
            fixture.csv.clone().into(),
            "--dataset".into(),
            fixture.output.clone().into(),
        ])
        .output()
        .expect("the verifier runs");
    let stdout = String::from_utf8_lossy(&failed.stdout);
    assert_eq!(failed.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("FAIL  LAYOUT"), "{stdout}");
    assert!(stdout.contains("RESULT: FAIL"), "{stdout}");

    let usage = Command::new(binary)
        .arg("--csv")
        .output()
        .expect("the verifier runs");
    assert_eq!(usage.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("--dataset"));
}
