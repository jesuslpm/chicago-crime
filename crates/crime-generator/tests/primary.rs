//! Generation of the primary table: external sort, duplicate rule (D5), compression (G2) and
//! determinism (G5).

mod common;

use std::borrow::Cow;
use std::fs;
use std::num::NonZeroUsize;

use common::{
    Scenario, distinct_record, distinct_row, folder_entries, generate, plain_csv_line, read_table,
    row_with_id, sample_row, with_field,
};
use crime_dataset::{Compression, PRIMARY_FILE_NAME, RecordCounts, decode_value, primary_key};
use crime_generator::Abort;
use foundationdb_tuple::{Element, unpack};

/// IDs in no particular order, far from sorted.
const UNSORTED_IDS: [i64; 10] = [42, 7, 1_000_000, 3, 19, 600, 8, 77, 5, 64];

fn segment_size(size: usize) -> NonZeroUsize {
    NonZeroUsize::new(size).expect("a positive segment size")
}

/// Asserts that the primary table holds exactly `ids`, in ascending order, each with the record of
/// its input row.
fn assert_primary_holds(scenario_output: &std::path::Path, ids: &[i64]) {
    let entries = read_table(&scenario_output.join(PRIMARY_FILE_NAME));
    let mut expected = ids.to_vec();
    expected.sort_unstable();

    let keys: Vec<Vec<u8>> = entries.iter().map(|(key, _)| key.clone()).collect();
    let expected_keys: Vec<Vec<u8>> = expected.iter().map(|&id| primary_key(id)).collect();
    assert_eq!(keys, expected_keys, "one key per ID, in ascending order");
    for ((_, value), &id) in entries.iter().zip(&expected) {
        let record = decode_value(value).expect("every value decodes");
        assert_eq!(record, distinct_record(id), "value of ID {id}");
    }
}

#[test]
fn unsorted_rows_are_written_in_ascending_id_order() {
    let rows: Vec<_> = UNSORTED_IDS.iter().map(|&id| distinct_row(id)).collect();
    let scenario = Scenario::new(&rows);
    let config = scenario.config("dataset");

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(
        summary.counts,
        RecordCounts {
            valid: 10,
            rejected: 0
        }
    );
    assert_eq!(summary.primary.entries, 10);
    assert_eq!(summary.sort_segments, 0, "ten rows are sorted in memory");
    assert_primary_holds(&config.output, &UNSORTED_IDS);
}

#[test]
fn unsorted_rows_merged_from_several_segments_are_in_ascending_id_order() {
    let rows: Vec<_> = UNSORTED_IDS.iter().map(|&id| distinct_row(id)).collect();
    let scenario = Scenario::new(&rows);
    let config = crime_generator::Config {
        segment_size: segment_size(3),
        ..scenario.config("dataset")
    };

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(summary.sort_segments, 4, "ten rows in segments of three");
    assert_eq!(summary.primary.entries, 10);
    assert_primary_holds(&config.output, &UNSORTED_IDS);
}

#[test]
fn the_table_reads_back_with_sstable_and_tuple_unpacking_alone() {
    let without_optionals = with_field(
        with_field(distinct_row(15), "Location Description", b""),
        "Ward",
        b"",
    );
    let scenario = Scenario::new(&[distinct_row(900), without_optionals, distinct_row(2)]);
    let config = scenario.config("dataset");

    let (result, log) = generate(&config);
    result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));

    let entries = read_table(&config.output.join("primary.sst"));
    let ids: Vec<i64> = entries
        .iter()
        .map(|(key, _)| {
            let (id,): (i64,) = unpack(key).expect("the key is a packed 1-tuple");
            id
        })
        .collect();
    assert_eq!(ids, [2, 15, 900]);
    assert!(
        entries.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "keys are strictly increasing as bytes"
    );
    for ((_, value), id) in entries.iter().zip(ids) {
        let fields: Vec<Element<'_>> = unpack(value).expect("the value is a packed tuple");
        assert_eq!(fields.len(), 21, "every column but Location");
        assert_eq!(fields[0], Element::Int(id));
        assert_eq!(fields[1], Element::String(Cow::Owned(format!("JC{id}"))));
        assert_eq!(fields[5], Element::String(Cow::Borrowed("ARSON")));
        assert_eq!(fields[8], Element::Bool(false));
        assert_eq!(
            fields[11],
            Element::String(Cow::Borrowed("003")),
            "District keeps its leading zeros"
        );
        assert_eq!(fields[19], Element::Double(41.777750747));
        if id == 15 {
            assert_eq!(fields[7], Element::Nil, "empty Location Description");
            assert_eq!(fields[12], Element::Nil, "empty Ward");
        } else {
            assert_eq!(
                fields[7],
                Element::String(Cow::Borrowed("VEHICLE NON-COMMERCIAL"))
            );
            assert_eq!(fields[12], Element::Int(20));
        }
    }
}

#[test]
fn duplicates_in_different_segments_keep_the_first_row_and_log_the_rest() {
    let first = with_field(distinct_row(500), "Case Number", b"FIRST");
    let second = with_field(distinct_row(500), "Case Number", b"SECOND");
    let third = with_field(distinct_row(500), "Case Number", b"THIRD");
    let repeated_100 = with_field(distinct_row(100), "Case Number", b"AGAIN");
    // Lines start at 2: the header is line 1.
    let rows = [
        first,                                        // line 2
        distinct_row(100),                            // line 3
        distinct_row(300),                            // line 4
        distinct_row(200),                            // line 5
        with_field(sample_row(), "Year", b"unknown"), // line 6, rejected while reading
        distinct_row(400),                            // line 7
        second.clone(),                               // line 8
        distinct_row(600),                            // line 9
        repeated_100,                                 // line 10
        third,                                        // line 11
    ];
    let scenario = Scenario::new(&rows);
    let config = crime_generator::Config {
        segment_size: segment_size(2),
        ..scenario.config("dataset")
    };

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(
        summary.sort_segments, 5,
        "nine valid rows in segments of two put the repeats in different segments"
    );
    assert_eq!(
        summary.counts,
        RecordCounts {
            valid: 6,
            rejected: 4
        }
    );
    assert_eq!(summary.primary.entries, 6);

    let entries = read_table(&config.output.join(PRIMARY_FILE_NAME));
    let kept: Vec<(i64, String)> = entries
        .iter()
        .map(|(_, value)| {
            let record = decode_value(value).expect("every value decodes");
            (record.id, record.case_number)
        })
        .collect();
    assert_eq!(
        kept,
        [
            (100, "JC100".to_owned()),
            (200, "JC200".to_owned()),
            (300, "JC300".to_owned()),
            (400, "JC400".to_owned()),
            (500, "FIRST".to_owned()),
            (600, "JC600".to_owned()),
        ],
        "the lowest line of each ID is kept"
    );

    let expected_lines = [
        "Fila 10 omitida: el ID 100 ya apareció en la fila 3.".to_owned(),
        format!(
            "Fila 8 omitida: el ID 500 ya apareció en la fila 2. Contenido original: {}",
            plain_csv_line(&second)
        ),
        "Fila 11 omitida: el ID 500 ya apareció en la fila 2.".to_owned(),
        "Generación completada: 6 registros válidos, 4 rechazados (ver log).".to_owned(),
    ];
    for expected in &expected_lines {
        assert!(
            log.lines().any(|line| line.starts_with(expected.as_str())),
            "the log should hold {expected:?}:\n{log}"
        );
    }
    let duplicate_lines: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("ya apareció"))
        .collect();
    assert_eq!(duplicate_lines.len(), 3, "{log}");
    assert!(
        duplicate_lines
            .iter()
            .all(|line| line.contains(". Contenido original: ")),
        "every duplicate shows its row: {log}"
    );
}

#[test]
fn snappy_tables_are_smaller_than_uncompressed_ones_and_both_read_back() {
    let rows: Vec<_> = (1..=3_000).map(row_with_id).collect();
    let scenario = Scenario::new(&rows);
    let snappy = scenario.config("snappy");
    let plain = crime_generator::Config {
        compression: Compression::None,
        ..scenario.config("plain")
    };

    let (snappy_result, snappy_log) = generate(&snappy);
    let (plain_result, plain_log) = generate(&plain);

    let snappy_summary = snappy_result.unwrap_or_else(|abort| panic!("{abort}\n{snappy_log}"));
    let plain_summary = plain_result.unwrap_or_else(|abort| panic!("{abort}\n{plain_log}"));
    let snappy_path = snappy.output.join(PRIMARY_FILE_NAME);
    let plain_path = plain.output.join(PRIMARY_FILE_NAME);
    let snappy_entries = read_table(&snappy_path);
    assert_eq!(snappy_entries.len(), 3_000);
    assert_eq!(
        snappy_entries,
        read_table(&plain_path),
        "same content either way"
    );

    let snappy_size = fs::metadata(&snappy_path).expect("metadata").len();
    let plain_size = fs::metadata(&plain_path).expect("metadata").len();
    assert_eq!(snappy_summary.primary.size_bytes, snappy_size);
    assert_eq!(plain_summary.primary.size_bytes, plain_size);
    assert!(
        snappy_size < plain_size,
        "Snappy {snappy_size} bytes should be smaller than {plain_size} bytes"
    );
}

#[test]
fn the_same_csv_always_produces_a_byte_identical_primary_table() {
    let mut rows: Vec<_> = UNSORTED_IDS.iter().map(|&id| distinct_row(id)).collect();
    rows.push(with_field(distinct_row(19), "Case Number", b"REPEATED"));
    rows.extend((2_000..2_500).rev().map(distinct_row));
    let scenario = Scenario::new(&rows);
    let small_segments = |name| crime_generator::Config {
        segment_size: segment_size(7),
        ..scenario.config(name)
    };
    let first = small_segments("first");
    let second = small_segments("second");
    let in_memory = scenario.config("in-memory");

    for config in [&first, &second, &in_memory] {
        let (result, log) = generate(config);
        result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    }

    let bytes = |config: &crime_generator::Config| {
        fs::read(config.output.join(PRIMARY_FILE_NAME)).expect("the table can be read")
    };
    assert_eq!(bytes(&first), bytes(&second), "two identical runs");
    assert_eq!(
        bytes(&first),
        bytes(&in_memory),
        "the segment size does not change the table"
    );
}

#[test]
fn sort_segments_and_staging_files_are_gone_after_a_successful_run() {
    let rows: Vec<_> = (1..=20).rev().map(distinct_row).collect();
    let scenario = Scenario::new(&rows);
    let config = crime_generator::Config {
        segment_size: segment_size(2),
        ..scenario.config("dataset")
    };

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(summary.sort_segments, 10, "the sort used the disk");
    assert_eq!(
        folder_entries(&scenario.temp_dir),
        Vec::<String>::new(),
        "no sort folder or segment is left"
    );
    assert_eq!(folder_entries(&config.output), [PRIMARY_FILE_NAME]);
}

#[test]
fn an_existing_dataset_is_kept_without_the_overwrite_option() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let config = scenario.config("dataset");
    fs::create_dir(&config.output).expect("the output folder can be created");
    let existing = config.output.join(PRIMARY_FILE_NAME);
    fs::write(&existing, b"previous dataset").expect("the fixture can be written");

    let (result, log) = generate(&config);

    let abort = result.expect_err("an existing dataset needs the overwrite option");
    assert_eq!(abort, Abort::DatasetExists);
    let message = "La carpeta de salida ya contiene un conjunto de datos. Usa la opción de \
                   sobrescritura para reemplazarlo.";
    assert_eq!(abort.to_string(), message);
    assert_eq!(
        fs::read(&existing).expect("the table can be read"),
        b"previous dataset"
    );
    assert_eq!(folder_entries(&config.output), [PRIMARY_FILE_NAME]);
    assert_eq!(folder_entries(&scenario.temp_dir), Vec::<String>::new());
    assert!(!log.contains("Lectura completada"), "no row is read: {log}");
}

#[test]
fn an_existing_dataset_is_replaced_with_the_overwrite_option() {
    let scenario = Scenario::new(&[distinct_row(8), distinct_row(4)]);
    let config = crime_generator::Config {
        overwrite: true,
        ..scenario.config("dataset")
    };
    fs::create_dir(&config.output).expect("the output folder can be created");
    fs::write(config.output.join(PRIMARY_FILE_NAME), b"previous dataset")
        .expect("the fixture can be written");

    let (result, log) = generate(&config);

    result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_primary_holds(&config.output, &[4, 8]);
    assert_eq!(folder_entries(&config.output), [PRIMARY_FILE_NAME]);
}

#[test]
fn a_csv_without_valid_rows_produces_an_empty_readable_table() {
    let scenario = Scenario::new(&[with_field(sample_row(), "ID", b"abc")]);
    let config = scenario.config("dataset");

    let (result, log) = generate(&config);

    let summary = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    assert_eq!(
        summary.counts,
        RecordCounts {
            valid: 0,
            rejected: 1
        }
    );
    assert_eq!(summary.primary.entries, 0);
    assert!(read_table(&config.output.join(PRIMARY_FILE_NAME)).is_empty());
}
