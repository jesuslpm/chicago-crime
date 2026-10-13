//! `crime-acceptance select` on a small fixture CSV: counts, ID lists and source rows, the
//! independent side of the query app's acceptance (query app PRD, "El filtrado directo del CSV").

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// Rows out of ID order, with a text containing a quote and a comma; the fourth one has three
/// fields only and cannot be read as a record.
fn fixture_rows() -> Vec<Vec<String>> {
    vec![
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Primary Type", "Theft"),
            ("District", "008"),
            ("Community Area", ""),
            ("Date", "10/01/2026 12:00:00 AM"),
            ("Arrest", "true"),
        ]),
        row(&[
            ("ID", "900"),
            ("Primary Type", "THEFT"),
            ("District", "8"),
            ("Date", "10/31/2026 11:59:59 PM"),
        ]),
        vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
        row(&[
            ("ID", "5000"),
            ("Primary Type", "THEFT"),
            ("District", "008"),
            ("Date", "11/01/2026 12:00:00 AM"),
            ("Block", "001XX \"N\" STATE, ST"),
        ]),
        row(&[
            ("ID", "77"),
            ("Primary Type", "theft"),
            ("District", "008"),
            ("Date", "09/30/2026 11:59:59 PM"),
            ("Location Description", ""),
        ]),
    ]
}

/// One CSV line with every field quoted, as the source writes it.
fn csv_line(fields: &[String], terminator: &str) -> String {
    let quoted: Vec<String> = fields
        .iter()
        .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
        .collect();
    format!("{}{terminator}", quoted.join(","))
}

fn header_line(terminator: &str) -> String {
    let header: Vec<String> = HEADER.iter().map(|&name| name.to_owned()).collect();
    csv_line(&header, terminator)
}

struct Fixture {
    dir: TempDir,
    csv: PathBuf,
    rows: Vec<Vec<String>>,
}

impl Fixture {
    fn new(terminator: &str) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let csv = dir.path().join("crimes.csv");
        let rows = fixture_rows();
        let mut text = header_line(terminator);
        for fields in &rows {
            text.push_str(&csv_line(fields, terminator));
        }
        fs::write(&csv, text).expect("the fixture CSV can be written");
        Self { dir, csv, rows }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn select(&self, args: &[&str]) -> Output {
        select(&self.csv, args)
    }

    /// The source line of the row with `id`.
    fn line(&self, id: &str, terminator: &str) -> String {
        let fields = self
            .rows
            .iter()
            .find(|fields| fields[0] == id)
            .unwrap_or_else(|| panic!("{id} is a fixture row"));
        csv_line(fields, terminator)
    }
}

fn select(csv: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crime-acceptance"))
        .arg("select")
        .arg("--csv")
        .arg(csv)
        .args(args)
        .output()
        .expect("the verifier runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The count printed by a successful run.
fn count(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(0), "{}", stderr(output));
    stdout(output)
}

#[test]
fn the_count_is_a_bare_integer_and_the_unreadable_row_is_reported_apart() {
    let fixture = Fixture::new("\n");

    let theft = fixture.select(&["--where", "primary type=THEFT"]);

    assert_eq!(count(&theft), "4\n");
    let report = stderr(&theft);
    assert!(
        report.contains("6 rows read")
            && report.contains("4 matching")
            && report.contains("1 skipped"),
        "{report}"
    );
    assert_eq!(
        count(&fixture.select(&[])),
        "5\n",
        "no condition counts every readable row"
    );
}

#[test]
fn conditions_combine_as_the_query_app_does() {
    let fixture = Fixture::new("\n");

    let cases: [(&[&str], &str); 6] = [
        (
            &["--where", "Primary Type=theft", "--where", "District=008"],
            "3\n",
        ),
        (&["--where", "District=8"], "1\n"),
        (&["--where", "District=8", "--where", "district=003"], "2\n"),
        (&["--where", "Community Area="], "1\n"),
        (
            &["--where", "Arrest=sí", "--where", "Primary Type=THEFT"],
            "1\n",
        ),
        (&["--range", "Date=2026-10-01..2026-10-31"], "3\n"),
    ];
    for (args, expected) in cases {
        assert_eq!(count(&fixture.select(args)), expected, "{args:?}");
    }
}

#[test]
fn ids_out_lists_the_matching_ids_in_ascending_order() {
    let fixture = Fixture::new("\n");
    let ids = fixture.path("ids.txt");

    let output = fixture.select(&[
        "--range",
        "Date=2026-10-01..2026-10-31",
        "--ids-out",
        ids.to_str().expect("a UTF-8 path"),
    ]);

    assert_eq!(count(&output), "3\n");
    assert_eq!(
        fs::read_to_string(&ids).expect("the ID list is written"),
        "634\n900\n14348063\n"
    );
}

#[test]
fn rows_out_writes_the_source_header_and_rows_by_ascending_id_byte_for_byte() {
    for terminator in ["\n", "\r\n"] {
        let fixture = Fixture::new(terminator);
        let rows = fixture.path("rows.csv");

        let output = fixture.select(&[
            "--where",
            "District=008",
            "--rows-out",
            rows.to_str().expect("a UTF-8 path"),
        ]);

        assert_eq!(count(&output), "3\n", "{terminator:?}");
        let expected = [
            header_line(terminator),
            fixture.line("77", terminator),
            fixture.line("634", terminator),
            fixture.line("5000", terminator),
        ]
        .concat();
        let written = fs::read(&rows).expect("the rows are written");
        assert_eq!(
            String::from_utf8_lossy(&written),
            expected,
            "{terminator:?}"
        );
        let source = fs::read_to_string(&fixture.csv).expect("the source can be read");
        for line in expected.split_inclusive(terminator) {
            assert!(source.contains(line), "{line:?} is a source line");
        }
    }
}

#[test]
fn an_empty_result_writes_empty_lists() {
    let fixture = Fixture::new("\n");
    let (ids, rows) = (fixture.path("ids.txt"), fixture.path("rows.csv"));

    let output = fixture.select(&[
        "--where",
        "Primary Type=BATTERY",
        "--ids-out",
        ids.to_str().expect("a UTF-8 path"),
        "--rows-out",
        rows.to_str().expect("a UTF-8 path"),
    ]);

    assert_eq!(count(&output), "0\n");
    assert_eq!(fs::read_to_string(&ids).expect("written"), "");
    assert_eq!(
        fs::read_to_string(&rows).expect("written"),
        header_line("\n")
    );
}

#[test]
fn a_wrong_command_line_exits_2_and_an_unreadable_file_exits_1() {
    let fixture = Fixture::new("\n");

    for args in [
        &["--where", "Color=red"][..],
        &["--where", "Ward=abc"],
        &["--range", "Ward=1..2"],
        &["--range", "Date=2026-10-02..2026-10-01"],
        &["--where"],
        &["--limit", "3"],
    ] {
        let output = fixture.select(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(stdout(&output).is_empty(), "{args:?}");
        assert!(
            stderr(&output).contains("usage: crime-acceptance select"),
            "{args:?}"
        );
    }

    let missing = select(&fixture.path("missing.csv"), &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        stderr(&missing).contains("missing.csv"),
        "{}",
        stderr(&missing)
    );
    assert!(stdout(&missing).is_empty());

    let unwritable = fixture.select(&["--ids-out", fixture.dir.path().to_str().expect("UTF-8")]);
    assert_eq!(unwritable.status.code(), Some(1), "{}", stderr(&unwritable));

    let no_csv = Command::new(env!("CARGO_BIN_EXE_crime-acceptance"))
        .arg("select")
        .output()
        .expect("the verifier runs");
    assert_eq!(no_csv.status.code(), Some(2));
    assert!(stderr(&no_csv).contains("--csv"));
}
