//! The CanopyDB query app answers exactly as the original query app (decision K1, AC-KC1, AC-M2
//! and AC-M7 on small datasets): the same rows published by both generators, the same queries,
//! the same stdout (in any order, decision C11), the same stderr and exit code, alone and in a
//! batch. Only the JSON `dataset` member differs (decision KC6).

mod common;

use std::fs;

use common::original::Fixture;
use common::{CanopyFixture, Ran, Row, fixture_rows, many_rows, row, run};
use crime_canopy_query::APP;

/// The rows of both fixtures together: thefts enough for every stream to cross several pages of
/// keys, and the mixed cases of the original tests.
fn rows() -> Vec<Row> {
    let mut rows = many_rows(3_000);
    rows.extend(fixture_rows().into_iter().map(|mut fields| {
        // Keep their IDs clear of the many rows.
        let id: i64 = fields[0].parse().expect("an ID");
        fields[0] = (100_000 + id).to_string();
        fields
    }));
    rows.push(row(&[
        ("ID", "200000"),
        ("Case Number", "JA100"),
        ("Arrest", "true"),
    ]));
    rows
}

/// The output of both apps with stdout sorted (the order is unspecified, decision C11) and the
/// times of stderr masked.
fn normalized(ran: &Ran) -> (u8, Vec<String>, Vec<String>) {
    let mut stdout: Vec<String> = ran.stdout.lines().map(str::to_owned).collect();
    stdout.sort();
    (ran.code, stdout, ran.stderr_lines())
}

const QUERIES: &[&[&str]] = &[
    &["--where", "Primary Type=THEFT", "--count"],
    &["--where", "Primary Type=theft", "--format", "csv"],
    &[
        "--where",
        "Primary Type=THEFT",
        "--where",
        "Primary Type=BATTERY",
        "--format",
        "csv",
    ],
    &[
        "--where",
        "Primary Type=THEFT",
        "--where",
        "District=008",
        "--count",
    ],
    &[
        "--where",
        "Primary Type=THEFT",
        "--where",
        "District=",
        "--count",
    ],
    &["--where", "Community Area=", "--count"],
    &[
        "--where",
        "Community Area=42",
        "--where",
        "Community Area=7",
        "--format",
        "csv",
    ],
    &["--startswith", "Case Number=JB1", "--count"],
    &[
        "--startswith",
        "Case Number=jb2",
        "--startswith",
        "Case Number=JB",
        "--count",
    ],
    &[
        "--startswith",
        "Primary Type=BAT",
        "--where",
        "Primary Type=THEFT",
        "--count",
    ],
    &["--range", "Date=2020-01-02..2020-03-31", "--count"],
    &[
        "--range",
        "Date=2020-01-01..2020-01-03",
        "--where",
        "Arrest=true",
        "--format",
        "csv",
    ],
    &[
        "--range",
        "Date=..2020-01-05",
        "--where",
        "Primary Type=THEFT",
        "--format",
        "table",
    ],
    &["--where", "Case Number=JA100", "--format", "csv"],
    &["--where", "Case Number=JA000000", "--count"],
    &[
        "--where", "ID=1", "--where", "ID=3000", "--where", "ID=123", "--format", "csv",
    ],
    &["--where", "Primary Type=THEFT", "--limit", "5", "--count"],
    &["--where", "Primary Type=HOMICIDE", "--count"],
    &["--where", "Arrest=true", "--count"],
    &["--where", "Primary Type=", "--count"],
];

#[test]
fn every_query_gives_the_same_results_as_the_original_app() {
    let rows = rows();
    let sst = Fixture::with_rows(&rows);
    let canopy = CanopyFixture::with_rows(&rows);
    for query in QUERIES {
        let original = run(&crime_query::APP, &sst.output(), query);
        let ours = run(&APP, &canopy.output(), query);
        // The folder appears in some messages: compare with the same folder name.
        let ours = Ran {
            stderr: ours.stderr.replace(
                &canopy.output().display().to_string(),
                &sst.output().display().to_string(),
            ),
            ..ours
        };
        assert_eq!(normalized(&ours), normalized(&original), "{query:?}");
    }
}

#[test]
fn a_batch_gives_the_same_results_as_the_original_app() {
    let rows = rows();
    let sst = Fixture::with_rows(&rows);
    let canopy = CanopyFixture::with_rows(&rows);
    let batch = sst.dir.path().join("batch.txt");
    // A query rejected alone would reject the whole batch: only those that run.
    let lines: Vec<String> = QUERIES
        .iter()
        .filter(|query| run(&crime_query::APP, &sst.output(), query).code == 0)
        .map(|query| {
            query
                .iter()
                .map(|word| format!("\"{word}\""))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    fs::write(&batch, lines.join("\n")).expect("the batch can be written");
    let batch = batch.to_string_lossy().into_owned();
    let original = run(&crime_query::APP, &sst.output(), &["--batch", &batch]);
    let ours = run(&APP, &canopy.output(), &["--batch", &batch]);
    assert_eq!(original.code, 0, "{}", original.stderr);
    assert_eq!(normalized(&ours), normalized(&original));
}

#[test]
fn the_json_output_shows_the_manifest_schema_as_text() {
    let canopy = CanopyFixture::with_rows(&fixture_rows());
    let ran = run(
        &APP,
        &canopy.output(),
        &["--where", "ID=10", "--format", "json"],
    );
    assert_eq!(ran.code, 0, "{}", ran.stderr);
    let json: serde_json::Value = serde_json::from_str(&ran.stdout).expect("valid JSON");
    let version = canopy
        .version_dir()
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(json["dataset"]["schema_version"], "canopydb-2");
    assert_eq!(json["dataset"]["version"], version.as_str());
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(canopy.manifest_path()).expect("readable"))
            .expect("JSON");
    assert_eq!(json["dataset"]["generated_at"], manifest["generated_at"]);
    let keys: Vec<&String> = json["dataset"]
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert_eq!(keys, ["generated_at", "schema_version", "version"]);
    assert_eq!(json["results"][0]["ID"], 10);
}

#[test]
fn querying_adds_no_file_and_keeps_the_manifest() {
    let canopy = CanopyFixture::with_rows(&many_rows(1_500));
    let before = common::tree(&canopy.output());
    let manifest = fs::read(canopy.manifest_path()).expect("readable");
    for query in QUERIES {
        run(&APP, &canopy.output(), query);
    }
    assert_eq!(common::tree(&canopy.output()), before);
    assert_eq!(
        fs::read(canopy.manifest_path()).expect("readable"),
        manifest
    );
    let again = run(
        &APP,
        &canopy.output(),
        &["--where", "Primary Type=THEFT", "--count"],
    );
    assert_eq!(again.stdout, "1500\n");
}
