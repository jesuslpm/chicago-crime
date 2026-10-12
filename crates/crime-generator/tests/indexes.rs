//! Generation of the secondary index tables (decision D2): one entry per valid record in every
//! index, `Nil` for empty values, IDs ascending within a value, determinism (G5) and cleanup.

mod common;

use std::borrow::Cow;
use std::fs;
use std::num::NonZeroUsize;
use std::path::Path;

use common::{
    Row, Scenario, TABLE_FILES, datetime, distinct_row, folder_entries, generate, read_table,
    sample_row, with_field,
};
use crime_dataset::{IndexKind, RecordCounts};
use crime_generator::Abort;
use crime_generator::index::{IndexTable, check_index_counts};
use crime_generator::table::TableStats;
use foundationdb_tuple::{Element, unpack};

/// Index table file names, written out so the read-back does not depend on the dataset crate.
const DATE: &str = "index-date.sst";
const PRIMARY_TYPE: &str = "index-primary_type.sst";
const DISTRICT: &str = "index-district.sst";
const COMMUNITY_AREA: &str = "index-community_area.sst";
const CASE_NUMBER: &str = "index-case_number.sst";
const INDEX_FILES: [&str; 5] = [DATE, PRIMARY_TYPE, DISTRICT, COMMUNITY_AREA, CASE_NUMBER];

/// Valid records of [`fixture_rows`], in ID order.
const VALID_IDS: [i64; 6] = [5, 10, 20, 30, 40, 50];

/// A distinct row with the five indexed fields set.
fn row(
    id: i64,
    date: &str,
    primary_type: &str,
    district: &str,
    community_area: &str,
    case_number: &str,
) -> Row {
    let row = with_field(distinct_row(id), "Date", date.as_bytes());
    let row = with_field(row, "Primary Type", primary_type.as_bytes());
    let row = with_field(row, "District", district.as_bytes());
    let row = with_field(row, "Community Area", community_area.as_bytes());
    with_field(row, "Case Number", case_number.as_bytes())
}

/// Six valid records in no particular order, one row rejected while reading and one duplicate
/// ID, whose values must not reach any index.
fn fixture_rows() -> Vec<Row> {
    let rejected = with_field(
        row(
            60,
            "02/02/2020 02:00:00 AM",
            "REJECTED",
            "555",
            "55",
            "REJECTED",
        ),
        "Year",
        b"unknown",
    );
    vec![
        row(30, "03/15/2020 10:00:00 AM", "THEFT", "", "", "SHARED1"),
        row(10, "01/01/2021 12:00:00 AM", "BATTERY", "007", "25", "JC10"),
        row(20, "12/31/2019 11:59:59 PM", "THEFT", "", "8", "SHARED1"),
        rejected,
        row(50, "06/01/2020 08:30:00 PM", "ARSON", "001", "", "SHARED1"),
        row(40, "06/01/2020 08:30:00 PM", "THEFT", "001", "8", "JC40"),
        row(
            10,
            "01/01/1999 01:00:00 AM",
            "DUPLICATE",
            "999",
            "77",
            "DUPLICATE",
        ),
        row(5, "07/04/1965 09:15:00 AM", "BATTERY", "007", "25", "JC5"),
    ]
}

fn segment_size(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).expect("a positive segment size")
}

/// Generates the fixture into the output folder `name` with both sorts forced onto the disk.
fn generate_fixture(scenario: &Scenario, name: &str) -> crime_generator::RunSummary {
    let config = crime_generator::Config {
        segment_size: segment_size(2),
        index_segment_size: segment_size(4),
        ..scenario.config(name)
    };
    let (result, log) = generate(&config);
    result.unwrap_or_else(|abort| panic!("{abort}\n{log}"))
}

/// Every key of an index table as `(value, id)`, read with `sstable` and tuple unpacking alone.
/// Index values are always empty.
fn index_keys(output: &Path, file: &str) -> Vec<(Element<'static>, i64)> {
    read_table(&output.join(file))
        .into_iter()
        .map(|(key, value)| {
            assert!(value.is_empty(), "index values are empty in {file}");
            let (element, id): (Element<'_>, i64) =
                unpack(&key).unwrap_or_else(|error| panic!("{file}: {key:02x?}: {error:?}"));
            (element.into_owned(), id)
        })
        .collect()
}

fn ids(keys: &[(Element<'static>, i64)]) -> Vec<i64> {
    keys.iter().map(|&(_, id)| id).collect()
}

fn text(value: &'static str) -> Element<'static> {
    Element::String(Cow::Borrowed(value))
}

#[test]
fn every_index_has_one_entry_per_valid_record_and_skips_rejected_and_duplicate_rows() {
    let scenario = Scenario::new(&fixture_rows());
    let config = crime_generator::Config {
        segment_size: segment_size(2),
        index_segment_size: segment_size(4),
        ..scenario.config("dataset")
    };

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(
        summary.counts,
        RecordCounts {
            valid: 6,
            rejected: 2
        }
    );
    let counts: Vec<(IndexKind, u64)> = summary
        .indexes
        .iter()
        .map(|table| (table.kind, table.stats.entries))
        .collect();
    assert_eq!(counts, IndexKind::ALL.map(|kind| (kind, 6)));
    for (file, table) in INDEX_FILES.iter().zip(&summary.indexes) {
        let keys = index_keys(&config.output, file);
        let mut found = ids(&keys);
        found.sort_unstable();
        assert_eq!(found, VALID_IDS, "{file} holds every valid ID once");
        let size = fs::metadata(config.output.join(file))
            .expect("metadata")
            .len();
        assert_eq!(table.stats.size_bytes, size, "{file}");
        assert!(
            !keys.iter().any(|(value, _)| {
                [
                    text("DUPLICATE"),
                    text("REJECTED"),
                    text("999"),
                    text("555"),
                ]
                .contains(value)
                    || *value == Element::Int(77)
            }),
            "no value of the duplicate or rejected rows in {file}: {keys:?}"
        );
    }
    let expected = "Índices generados: date 6, primary_type 6, district 6, community_area 6, \
                    case_number 6.";
    assert!(log.lines().any(|line| line == expected), "{log}");
}

#[test]
fn keys_are_strictly_increasing_and_ids_ascend_within_each_value() {
    let scenario = Scenario::new(&fixture_rows());
    let summary = generate_fixture(&scenario, "dataset");
    assert!(
        summary.index_sort_segments > 1,
        "the index sort used the disk"
    );
    let output = scenario.dir.path().join("dataset");

    for file in INDEX_FILES {
        let raw: Vec<Vec<u8>> = read_table(&output.join(file))
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert!(
            raw.windows(2).all(|pair| pair[0] < pair[1]),
            "{file}: keys strictly increasing"
        );
        let keys = index_keys(&output, file);
        for pair in keys.windows(2) {
            if pair[0].0 == pair[1].0 {
                assert!(pair[0].1 < pair[1].1, "{file}: IDs ascend within a value");
            }
        }
    }
}

#[test]
fn empty_district_and_community_area_are_indexed_as_nil_and_come_first() {
    let scenario = Scenario::new(&fixture_rows());
    generate_fixture(&scenario, "dataset");
    let output = scenario.dir.path().join("dataset");

    let district = index_keys(&output, DISTRICT);
    let nil_districts: Vec<i64> = district
        .iter()
        .filter(|(value, _)| *value == Element::Nil)
        .map(|&(_, id)| id)
        .collect();
    assert_eq!(nil_districts, [20, 30], "rows with an empty District");
    assert!(
        district[..2]
            .iter()
            .all(|(value, _)| *value == Element::Nil)
    );

    let community_area = index_keys(&output, COMMUNITY_AREA);
    let nil_areas: Vec<i64> = community_area
        .iter()
        .filter(|(value, _)| *value == Element::Nil)
        .map(|&(_, id)| id)
        .collect();
    assert_eq!(nil_areas, [30, 50], "rows with an empty Community Area");
    assert!(
        community_area[..2]
            .iter()
            .all(|(value, _)| *value == Element::Nil)
    );
}

#[test]
fn a_shared_case_number_lists_every_id_that_shares_it() {
    let scenario = Scenario::new(&fixture_rows());
    generate_fixture(&scenario, "dataset");
    let output = scenario.dir.path().join("dataset");

    let shared: Vec<i64> = index_keys(&output, CASE_NUMBER)
        .into_iter()
        .filter(|(value, _)| *value == text("SHARED1"))
        .map(|(_, id)| id)
        .collect();

    assert_eq!(shared, [20, 30, 50]);
}

#[test]
fn the_date_index_follows_the_chronological_order_of_the_records() {
    let scenario = Scenario::new(&fixture_rows());
    generate_fixture(&scenario, "dataset");
    let output = scenario.dir.path().join("dataset");
    let mut chronological: Vec<(i64, i64)> = [
        (5, "07/04/1965 09:15:00 AM"),
        (10, "01/01/2021 12:00:00 AM"),
        (20, "12/31/2019 11:59:59 PM"),
        (30, "03/15/2020 10:00:00 AM"),
        (40, "06/01/2020 08:30:00 PM"),
        (50, "06/01/2020 08:30:00 PM"),
    ]
    .into_iter()
    .map(|(id, date)| (datetime(date).seconds(), id))
    .collect();
    chronological.sort_unstable();

    let keys: Vec<(i64, i64)> = index_keys(&output, DATE)
        .into_iter()
        .map(|(value, id)| match value {
            Element::Int(seconds) => (seconds, id),
            other => panic!("a date is stored as an integer: {other:?}"),
        })
        .collect();

    assert_eq!(keys, chronological);
    assert_eq!(ids(&index_keys(&output, DATE)), [5, 20, 30, 40, 50, 10]);
}

#[test]
fn index_tables_read_back_with_sstable_and_tuple_unpacking_alone() {
    let scenario = Scenario::new(&fixture_rows());
    generate_fixture(&scenario, "dataset");
    let output = scenario.dir.path().join("dataset");
    let typed_keys = |file: &str| -> Vec<Vec<u8>> {
        read_table(&output.join(file))
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    };

    let primary_type: Vec<(String, i64)> = typed_keys(PRIMARY_TYPE)
        .iter()
        .map(|key| unpack(key).expect("(text, id)"))
        .collect();
    let district: Vec<(Option<String>, i64)> = typed_keys(DISTRICT)
        .iter()
        .map(|key| unpack(key).expect("(optional text, id)"))
        .collect();
    let community_area: Vec<(Option<i64>, i64)> = typed_keys(COMMUNITY_AREA)
        .iter()
        .map(|key| unpack(key).expect("(optional integer, id)"))
        .collect();
    let case_number: Vec<(String, i64)> = typed_keys(CASE_NUMBER)
        .iter()
        .map(|key| unpack(key).expect("(text, id)"))
        .collect();

    let owned = |pairs: &[(&str, i64)]| -> Vec<(String, i64)> {
        pairs.iter().map(|&(v, id)| (v.to_owned(), id)).collect()
    };
    assert_eq!(
        primary_type,
        owned(&[
            ("ARSON", 50),
            ("BATTERY", 5),
            ("BATTERY", 10),
            ("THEFT", 20),
            ("THEFT", 30),
            ("THEFT", 40),
        ])
    );
    assert_eq!(
        district,
        [
            (None, 20),
            (None, 30),
            (Some("001".to_owned()), 40),
            (Some("001".to_owned()), 50),
            (Some("007".to_owned()), 5),
            (Some("007".to_owned()), 10),
        ]
    );
    assert_eq!(
        community_area,
        [
            (None, 30),
            (None, 50),
            (Some(8), 20),
            (Some(8), 40),
            (Some(25), 5),
            (Some(25), 10),
        ]
    );
    assert_eq!(
        case_number,
        owned(&[
            ("JC10", 10),
            ("JC40", 40),
            ("JC5", 5),
            ("SHARED1", 20),
            ("SHARED1", 30),
            ("SHARED1", 50),
        ])
    );
}

#[test]
fn the_same_csv_always_produces_byte_identical_index_tables() {
    let mut rows = fixture_rows();
    rows.extend((1_000..1_300).rev().map(distinct_row));
    let scenario = Scenario::new(&rows);
    let small_segments = |name| crime_generator::Config {
        segment_size: segment_size(7),
        index_segment_size: segment_size(11),
        ..scenario.config(name)
    };
    let first = small_segments("first");
    let second = small_segments("second");
    let in_memory = scenario.config("in-memory");

    let mut segments = Vec::new();
    for config in [&first, &second, &in_memory] {
        let (result, log) = generate(config);
        let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
        segments.push(summary.index_sort_segments);
    }
    assert!(
        segments[0] > 1,
        "the index sort used the disk: {segments:?}"
    );
    assert_eq!(segments[2], 0, "everything fit in memory: {segments:?}");

    for file in INDEX_FILES {
        let bytes = |config: &crime_generator::Config| {
            fs::read(config.output.join(file)).expect("the table can be read")
        };
        assert_eq!(bytes(&first), bytes(&second), "{file}: two identical runs");
        assert_eq!(
            bytes(&first),
            bytes(&in_memory),
            "{file}: the segment size does not change the table"
        );
    }
}

#[test]
fn every_index_table_exists_and_is_empty_when_every_row_is_rejected() {
    let scenario = Scenario::new(&[
        with_field(sample_row(), "ID", b"abc"),
        with_field(sample_row(), "Date", b"yesterday"),
    ]);
    let config = scenario.config("dataset");

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(summary.counts.valid, 0);
    assert_eq!(
        summary
            .indexes
            .iter()
            .map(|table| (table.kind, table.stats.entries))
            .collect::<Vec<_>>(),
        IndexKind::ALL.map(|kind| (kind, 0))
    );
    for file in INDEX_FILES {
        assert!(
            read_table(&config.output.join(file)).is_empty(),
            "{file} exists and is empty"
        );
    }
    assert_eq!(folder_entries(&config.output), TABLE_FILES);
    let expected = "Índices generados: date 0, primary_type 0, district 0, community_area 0, \
                    case_number 0.";
    assert!(log.lines().any(|line| line == expected), "{log}");
}

#[test]
fn temporary_folders_of_both_sorts_are_gone_after_a_successful_run() {
    let rows: Vec<_> = (1..=20).rev().map(distinct_row).collect();
    let scenario = Scenario::new(&rows);
    let config = crime_generator::Config {
        segment_size: segment_size(3),
        index_segment_size: segment_size(7),
        ..scenario.config("dataset")
    };

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(summary.sort_segments, 7, "20 records in segments of three");
    assert_eq!(
        summary.index_sort_segments, 15,
        "100 index entries in segments of seven"
    );
    assert!(
        log.contains("Ordenación de los índices completada. Segmentos temporales en disco: 15."),
        "{log}"
    );
    assert_eq!(
        folder_entries(&scenario.temp_dir),
        Vec::<String>::new(),
        "no sort folder or segment is left"
    );
    assert_eq!(folder_entries(&config.output), TABLE_FILES);
}

#[test]
fn an_index_with_a_wrong_entry_count_is_an_internal_error() {
    let stats = |entries| TableStats {
        entries,
        size_bytes: 100,
    };
    let mut tables: Vec<IndexTable> = IndexKind::ALL
        .into_iter()
        .map(|kind| IndexTable {
            kind,
            stats: stats(6),
        })
        .collect();
    assert_eq!(check_index_counts(6, &tables), Ok(()));

    tables[3].stats = stats(5);
    let abort = check_index_counts(6, &tables).expect_err("community_area misses an entry");

    assert_eq!(
        abort,
        Abort::IndexCountMismatch {
            index: "community_area",
            entries: 5,
            valid: 6
        }
    );
    assert_eq!(
        abort.to_string(),
        "Error interno: el índice community_area tiene 5 entradas, pero la tabla principal \
         tiene 6 registros válidos."
    );
}
