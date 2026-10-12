//! The verifier against small datasets built by the generator from a fixture CSV, intact and
//! then broken on purpose: every defect must turn exactly the checks that cover it into FAIL.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crime_acceptance::{Options, Report, verify};
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
const CRITERIA: [&str; 17] = [
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

/// Five rows out of ID order: a theft without Ward or community area, a row without coordinates
/// that shares its case number with it, a row in the spring-forward hour without a district, and
/// another theft that shares its case number with that one.
fn fixture_rows() -> Vec<Vec<String>> {
    vec![
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Case Number", "JA100"),
            ("Primary Type", "THEFT"),
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
            ("Case Number", "JA200"),
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
        .filter(|check| !check.passed)
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
    assert!(!ac_g3.passed, "{}", report.render());
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
    assert!(check(&report, "AC-G1").passed, "{}", report.render());
    assert!(check(&report, "AC-G4-date").passed, "{}", report.render());
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
        assert!(check(&report, criterion).passed, "{}", report.render());
    }
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

#[test]
fn a_csv_without_a_row_in_the_spring_forward_hour_fails_the_dst_check() {
    let rows: Vec<Vec<String>> = fixture_rows()
        .into_iter()
        .filter(|fields| fields[0] != "14353993")
        .collect();
    let fixture = Fixture::with_rows(&rows);

    let report = fixture.verify();

    assert_eq!(failed(&report), ["AC-G3-DST"], "{}", report.render());
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
    assert!(check(&report, "AC-G3").passed, "{}", report.render());
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
        .filter(|line| line.starts_with("PASS") || line.starts_with("FAIL"))
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
