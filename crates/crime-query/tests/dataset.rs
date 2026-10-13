//! The dataset access layer against small datasets published by the generator: opening and its
//! failures, record reads, index streams, probes, the snapshot of an open dataset and read errors.

mod common;

use std::fs::{self, OpenOptions};
use std::path::Path;

use common::{
    Fixture, TABLE_FILES, datetime, fixture_rows, hourly_date, many_rows, row, write_current,
};
use crime_dataset::{CivilDateTime, IndexKind, SCHEMA_VERSION};
use crime_query::{Dataset, IdStream, IndexValue, OpenError};

fn open(fixture: &Fixture) -> Dataset {
    Dataset::open(&fixture.output())
        .unwrap_or_else(|error| panic!("the dataset should open: {error} ({error:?})"))
}

fn ids(stream: IdStream) -> Vec<i64> {
    stream
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("the stream should be readable: {error} ({error:?})"))
}

fn open_error(folder: &Path) -> OpenError {
    match Dataset::open(folder) {
        Ok(_) => panic!("{} should not open", folder.display()),
        Err(error) => error,
    }
}

fn not_found_message(folder: &Path) -> String {
    format!(
        "No se encontró un conjunto de datos válido en {}.",
        folder.display()
    )
}

// Opening.

#[test]
fn open_reports_the_version_named_by_current_and_its_manifest() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let current = fs::read_to_string(fixture.output().join("CURRENT")).expect("CURRENT exists");
    assert_eq!(dataset.version(), current.trim_end());
    assert_eq!(dataset.manifest().schema_version, SCHEMA_VERSION);
    assert_eq!(dataset.manifest().schema_version, 2);
    assert_eq!(dataset.manifest().records.valid, 7);
    assert_eq!(dataset.folder(), fixture.output());
}

#[test]
fn a_folder_without_current_is_not_a_dataset() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.output()).expect("the folder can be created");

    let error = open_error(&fixture.output());

    assert!(matches!(error, OpenError::NotFound { .. }), "{error:?}");
    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn a_missing_folder_is_not_a_dataset() {
    let fixture = Fixture::new();
    let error = open_error(&fixture.output());
    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn current_naming_a_missing_version_is_not_a_dataset() {
    let fixture = Fixture::with_rows(&fixture_rows());
    write_current(&fixture.output(), "dataset-20200101-000000-1");

    let error = open_error(&fixture.output());

    assert!(matches!(error, OpenError::NotFound { .. }), "{error:?}");
    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn current_with_an_invalid_name_is_not_a_dataset() {
    let fixture = Fixture::with_rows(&fixture_rows());
    write_current(&fixture.output(), "..");

    let error = open_error(&fixture.output());

    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn a_missing_manifest_is_not_a_dataset() {
    let fixture = Fixture::with_rows(&fixture_rows());
    fs::remove_file(fixture.version_dir().join("manifest.json")).expect("the manifest exists");

    let error = open_error(&fixture.output());

    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn a_manifest_that_is_not_json_is_not_a_dataset() {
    let fixture = Fixture::with_rows(&fixture_rows());
    fs::write(fixture.version_dir().join("manifest.json"), "{ not json").expect("writable");

    let error = open_error(&fixture.output());

    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn a_schema_2_manifest_without_its_other_fields_is_not_a_dataset() {
    let fixture = Fixture::with_rows(&fixture_rows());
    fs::write(
        fixture.version_dir().join("manifest.json"),
        r#"{"schema_version": 2}"#,
    )
    .expect("writable");

    let error = open_error(&fixture.output());

    assert_eq!(error.to_string(), not_found_message(&fixture.output()));
}

#[test]
fn every_missing_table_makes_the_folder_not_a_dataset() {
    for file in TABLE_FILES {
        let fixture = Fixture::with_rows(&fixture_rows());
        fs::remove_file(fixture.version_dir().join(file)).expect("the table exists");

        let error = open_error(&fixture.output());

        assert!(
            matches!(error, OpenError::NotFound { .. }),
            "{file}: {error:?}"
        );
        assert_eq!(
            error.to_string(),
            not_found_message(&fixture.output()),
            "{file}"
        );
    }
}

#[test]
fn a_schema_1_dataset_is_rejected_with_both_versions() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let manifest_path = fixture.version_dir().join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).expect("the manifest exists");
    let old = manifest.replace("\"schema_version\": 2", "\"schema_version\": 1");
    assert_ne!(old, manifest, "the manifest should name schema 2");
    fs::write(&manifest_path, old).expect("writable");

    let error = open_error(&fixture.output());

    assert!(
        matches!(error, OpenError::IncompatibleSchema { found: 1 }),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "El conjunto usa el esquema v1 y este consultor admite v2. Regenera el conjunto."
    );
}

#[test]
fn the_schema_is_checked_before_the_rest_of_the_manifest() {
    let fixture = Fixture::with_rows(&fixture_rows());
    fs::write(
        fixture.version_dir().join("manifest.json"),
        r#"{"schema_version": 3, "layout": "something new"}"#,
    )
    .expect("writable");

    let error = open_error(&fixture.output());

    assert_eq!(
        error.to_string(),
        "El conjunto usa el esquema v3 y este consultor admite v2. Regenera el conjunto."
    );
}

#[test]
fn a_table_that_is_not_a_table_cannot_be_read() {
    // Shorter than a table footer, and long enough for one but without the magic number.
    for content in [b"short".to_vec(), vec![b'x'; 100]] {
        let fixture = Fixture::with_rows(&fixture_rows());
        fs::write(fixture.version_dir().join("index-district.sst"), &content).expect("writable");

        let error = open_error(&fixture.output());

        assert!(matches!(error, OpenError::Unreadable { .. }), "{error:?}");
        assert_eq!(
            error.to_string(),
            format!(
                "No se pudo leer el conjunto de datos en {}.",
                fixture.output().display()
            )
        );
    }
}

#[test]
fn opening_never_writes_in_the_dataset_folder() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let before = listing(&fixture.output());

    let dataset = open(&fixture);
    let _ = ids(dataset.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("theft")));
    drop(dataset);

    assert_eq!(listing(&fixture.output()), before);
}

/// Every path under `dir` with its size and content.
fn listing(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut all = Vec::new();
    for entry in fs::read_dir(dir).expect("listable") {
        let path = entry.expect("readable").path();
        if path.is_dir() {
            for (name, content) in listing(&path) {
                all.push((format!("{}/{name}", path.display()), content));
            }
        } else {
            all.push((
                path.display().to_string(),
                fs::read(&path).expect("readable"),
            ));
        }
    }
    all.sort();
    all
}

// Records.

#[test]
fn a_record_comes_back_exactly_as_in_the_csv() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let record = dataset.record(20).expect("readable").expect("ID 20 exists");

    assert_eq!(record.id, 20);
    assert_eq!(record.case_number, "JA100");
    assert_eq!(record.primary_type, "Theft");
    assert_eq!(record.district.as_deref(), Some("8"));
    assert_eq!(record.community_area, None);
    assert_eq!(record.date, datetime("01/01/2020 12:00:01 AM"));
    assert_eq!(record.block, "064XX S ELLIS AVE");
    let lowercase = dataset.record(30).expect("readable").expect("ID 30 exists");
    assert_eq!(
        (
            lowercase.case_number.as_str(),
            lowercase.primary_type.as_str()
        ),
        ("ja300", "theft")
    );
}

#[test]
fn a_missing_id_has_no_record() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    assert_eq!(dataset.record(11).expect("readable"), None);
    assert_eq!(dataset.record(14_348_063).expect("readable"), None);
}

#[test]
fn id_exists_tells_existing_ids_from_missing_ones() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    for id in [5, 10, 15, 20, 30, 40, 50] {
        assert!(dataset.id_exists(id).expect("readable"), "ID {id}");
    }
    for id in [0, 4, 11, 51, i64::MAX, -1] {
        assert!(!dataset.id_exists(id).expect("readable"), "ID {id}");
    }
}

// Equality streams.

#[test]
fn text_values_match_in_any_case_with_ids_ascending() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    for written in ["theft", "Theft", "THEFT", "tHeFt"] {
        let value = IndexValue::text(written);
        assert_eq!(
            value.as_text(),
            Some("theft"),
            "the applied form is lowercase"
        );
        assert_eq!(
            ids(dataset.ids_with_value(IndexKind::PrimaryType, &value)),
            [10, 20, 30],
            "{written}"
        );
    }
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("battery"))),
        [40, 50]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("HOMICIDE"))),
        Vec::<i64>::new()
    );
}

#[test]
fn codes_keep_their_leading_zeros() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::District, &IndexValue::text("008"))),
        [10, 15, 40]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::District, &IndexValue::text("8"))),
        [20]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::District, &IndexValue::text("08"))),
        Vec::<i64>::new()
    );
}

#[test]
fn nil_finds_the_records_without_a_value() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    assert!(IndexValue::nil().is_nil());
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::District, &IndexValue::nil())),
        [30, 50]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::CommunityArea, &IndexValue::nil())),
        [20, 40]
    );
}

#[test]
fn community_areas_match_as_integers() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let value = IndexValue::integer(42);
    assert_eq!(value.as_integer(), Some(42));
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::CommunityArea, &value)),
        [10, 15, 30]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::CommunityArea, &IndexValue::integer(7))),
        [50]
    );
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::CommunityArea, &IndexValue::integer(77))),
        Vec::<i64>::new()
    );
}

#[test]
fn a_shared_case_number_finds_every_record_in_any_case() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    for written in ["JA100", "ja100"] {
        assert_eq!(
            ids(dataset.ids_with_value(IndexKind::CaseNumber, &IndexValue::text(written))),
            [10, 20],
            "{written}"
        );
    }
    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::CaseNumber, &IndexValue::text("JA300"))),
        [30]
    );
}

#[test]
fn a_value_stream_spanning_many_blocks_is_complete_and_ascending() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);

    let all = ids(dataset.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("Theft")));

    assert_eq!(all, (1..=3_000).collect::<Vec<i64>>());
}

// Date ranges.

#[test]
fn a_date_range_includes_both_ends_in_date_order() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    let from = datetime("01/01/2020 12:00:00 AM");
    let to = datetime("01/03/2020 12:00:00 AM");

    assert_eq!(
        ids(dataset.ids_in_date_range(from, to)),
        [10, 20, 15, 30, 40]
    );
    let second = |date: CivilDateTime, delta: i64| {
        CivilDateTime::from_seconds(date.seconds() + delta).expect("in range")
    };
    assert_eq!(
        ids(dataset.ids_in_date_range(second(from, 1), second(to, -1))),
        [20, 15, 30]
    );
    assert_eq!(
        ids(dataset.ids_in_date_range(second(from, -1), second(to, 1))),
        [5, 10, 20, 15, 30, 40, 50]
    );
    assert_eq!(ids(dataset.ids_in_date_range(from, from)), [10]);
    assert_eq!(
        ids(dataset.ids_in_date_range(second(to, 2), second(to, 3600))),
        Vec::<i64>::new()
    );
}

#[test]
fn a_date_range_starting_between_blocks_finds_the_next_entry() {
    // Every start falls right after an existing key: at block boundaries that start lies between
    // the last key of a block and the separator the table keeps for it.
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    let end = hourly_date(3_000);

    for id in 1..3_000 {
        let start = CivilDateTime::from_seconds(hourly_date(id).seconds() + 1).expect("in range");
        let mut stream = dataset.ids_in_date_range(start, end);
        let first = stream.next().map(|result| result.expect("readable"));
        assert_eq!(
            first,
            Some(id + 1),
            "range starting one second after ID {id}"
        );
    }
    let past = CivilDateTime::from_seconds(end.seconds() + 1).expect("in range");
    assert_eq!(
        ids(dataset.ids_in_date_range(past, past)),
        Vec::<i64>::new()
    );
}

// Probes.

#[test]
fn has_entry_tells_whether_an_id_has_a_value() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);
    let theft = IndexValue::text("THEFT");

    assert!(
        dataset
            .has_entry(IndexKind::PrimaryType, &theft, 20)
            .expect("readable")
    );
    assert!(
        !dataset
            .has_entry(IndexKind::PrimaryType, &theft, 40)
            .expect("readable")
    );
    assert!(
        !dataset
            .has_entry(IndexKind::PrimaryType, &theft, 11)
            .expect("readable")
    );
    assert!(
        dataset
            .has_entry(IndexKind::District, &IndexValue::nil(), 30)
            .expect("readable")
    );
    assert!(
        !dataset
            .has_entry(IndexKind::District, &IndexValue::nil(), 10)
            .expect("readable")
    );
    assert!(
        dataset
            .has_entry(IndexKind::CommunityArea, &IndexValue::integer(42), 15)
            .expect("readable")
    );
    assert!(
        !dataset
            .has_entry(IndexKind::District, &IndexValue::text("8"), 10)
            .expect("readable")
    );
    assert!(
        dataset
            .has_entry(IndexKind::CaseNumber, &IndexValue::text("Ja100"), 10)
            .expect("readable")
    );
}

#[test]
fn has_value_tells_whether_any_record_has_a_value() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dataset = open(&fixture);

    assert!(
        dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("ja100"))
            .expect("ok")
    );
    assert!(
        dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("JA300"))
            .expect("ok")
    );
    assert!(
        !dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("JA000000"))
            .expect("ok")
    );
    assert!(
        !dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("JA10"))
            .expect("ok")
    );
    assert!(
        !dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("zzz"))
            .expect("ok")
    );
    assert!(
        dataset
            .has_value(IndexKind::District, &IndexValue::nil())
            .expect("ok")
    );
}

// Snapshot (AC-C12).

#[test]
fn an_open_dataset_keeps_reading_its_version_after_a_new_publication() {
    let mut fixture = Fixture::with_rows(&many_rows(3_000));
    let old_version = fixture.version_dir();
    let dataset = open(&fixture);

    // The new publication shares no ID and no case number with the old one.
    let replacement = vec![
        row(&[
            ("ID", "7000"),
            ("Case Number", "JZ1"),
            ("Primary Type", "THEFT"),
        ]),
        row(&[
            ("ID", "7001"),
            ("Case Number", "JZ2"),
            ("Primary Type", "ARSON"),
        ]),
    ];
    fixture.publish(&replacement);
    assert_ne!(fixture.version_dir(), old_version);
    println!(
        "old version folder still exists after the overwrite: {}\ngenerator log:\n{}",
        old_version.exists(),
        fixture.last_log()
    );

    assert_eq!(
        ids(dataset.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("theft"))),
        (1..=3_000).collect::<Vec<i64>>()
    );
    let last = dataset.record(3_000).expect("readable").expect("old ID");
    assert_eq!(last.block, "03000XX S STATE ST");
    assert!(!dataset.id_exists(7_000).expect("readable"));
    assert_eq!(
        ids(dataset.ids_in_date_range(hourly_date(1), hourly_date(3_000))).len(),
        3_000
    );
    assert!(
        dataset
            .has_value(IndexKind::CaseNumber, &IndexValue::text("JB2999"))
            .expect("ok")
    );

    let fresh = open(&fixture);
    assert_ne!(fresh.version(), dataset.version());
    assert_eq!(
        ids(fresh.ids_with_value(IndexKind::PrimaryType, &IndexValue::text("theft"))),
        [7_000]
    );
    assert!(!fresh.id_exists(3_000).expect("readable"));
}

// Read errors.

/// Cuts a table file of the open dataset to half its size, as a failing disk or a concurrent
/// writer would leave it.
fn truncate_to_half(path: &Path) {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the table can be opened for writing");
    let size = file.metadata().expect("readable").len();
    file.set_len(size / 2).expect("the table can be truncated");
}

#[test]
fn a_scan_over_a_damaged_table_fails_instead_of_returning_fewer_ids() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("index-date.sst"));

    let result: Result<Vec<i64>, _> = dataset
        .ids_in_date_range(hourly_date(1), hourly_date(3_000))
        .collect();

    let error = result.expect_err("the scan should fail");
    assert_eq!(
        error.to_string(),
        format!(
            "No se pudo leer el conjunto de datos en {}.",
            fixture.output().display()
        )
    );
}

#[test]
fn a_value_scan_over_a_damaged_table_fails() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("index-primary_type.sst"));

    let result: Result<Vec<i64>, _> = dataset
        .ids_with_value(IndexKind::PrimaryType, &IndexValue::text("theft"))
        .collect();

    assert!(result.is_err(), "{result:?}");
}

#[test]
fn a_record_read_over_a_damaged_table_fails() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let dataset = open(&fixture);
    truncate_to_half(&fixture.version_dir().join("primary.sst"));

    let error = dataset.record(2_990).expect_err("the read should fail");

    assert_eq!(
        error.to_string(),
        format!(
            "No se pudo leer el conjunto de datos en {}.",
            fixture.output().display()
        )
    );
    assert!(dataset.id_exists(2_995).is_err());
}
