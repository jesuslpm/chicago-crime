mod common;

use std::collections::HashSet;

use common::{datetime, sample_record};
use crime_dataset::{INDEX_FILE_PREFIX, IndexKind, TABLE_FILE_EXTENSION, index_key, index_text};
use foundationdb_tuple::pack;

#[test]
fn keys_pack_the_indexed_value_then_the_id_with_text_values_in_lowercase() {
    let record = sample_record();
    let id = record.id;

    assert_eq!(
        index_key(IndexKind::Date, &record),
        pack(&(record.date.seconds(), id))
    );
    assert_eq!(
        index_key(IndexKind::PrimaryType, &record),
        pack(&("arson", id))
    );
    assert_eq!(index_key(IndexKind::District, &record), pack(&("003", id)));
    assert_eq!(
        index_key(IndexKind::CommunityArea, &record),
        pack(&(42_i64, id))
    );
    assert_eq!(
        index_key(IndexKind::CaseNumber, &record),
        pack(&("jk438266", id))
    );
}

#[test]
fn mixed_case_text_values_are_indexed_in_lowercase() {
    let mut record = sample_record();
    record.primary_type = "Theft".to_owned();
    record.district = Some("01A".to_owned());
    record.case_number = "jA123".to_owned();
    let id = record.id;

    assert_eq!(
        index_key(IndexKind::PrimaryType, &record),
        pack(&("theft", id))
    );
    assert_eq!(index_key(IndexKind::District, &record), pack(&("01a", id)));
    assert_eq!(
        index_key(IndexKind::CaseNumber, &record),
        pack(&("ja123", id))
    );
}

#[test]
fn index_text_lowercases_and_never_pads_or_strips_codes() {
    assert_eq!(index_text("THEFT"), "theft");
    assert_eq!(index_text("Theft"), "theft");
    assert_eq!(index_text("jA123"), "ja123");
    assert_eq!(index_text("008"), "008");
    assert_eq!(index_text("8"), "8");
    assert_eq!(index_text("08B"), "08b");
    assert_eq!(index_text(""), "");
}

#[test]
fn empty_values_are_nil_and_sort_before_any_value() {
    let mut empty = sample_record();
    empty.id = 99_999_999;
    empty.district = None;
    empty.community_area = None;

    // Nil is the single byte 0x00 at the start of the key.
    assert_eq!(index_key(IndexKind::District, &empty)[0], 0x00);
    assert_eq!(index_key(IndexKind::CommunityArea, &empty)[0], 0x00);

    for district in ["", "001", "003", "031"] {
        let mut valued = sample_record();
        valued.id = 1;
        valued.district = Some(district.to_owned());
        assert!(
            index_key(IndexKind::District, &empty) < index_key(IndexKind::District, &valued),
            "{district:?}"
        );
    }
    for area in [i64::MIN, -1, 0, 1, 77] {
        let mut valued = sample_record();
        valued.id = 1;
        valued.community_area = Some(area);
        assert!(
            index_key(IndexKind::CommunityArea, &empty)
                < index_key(IndexKind::CommunityArea, &valued),
            "{area}"
        );
    }
}

#[test]
fn entries_with_the_same_value_sort_by_ascending_id() {
    let ids = [14_353_993, 2, 65_536, 256, 255, 10, 634];
    let mut ascending = ids.to_vec();
    ascending.sort();

    for kind in IndexKind::ALL {
        let mut keyed: Vec<(Vec<u8>, i64)> = ids
            .iter()
            .map(|&id| {
                let mut record = sample_record();
                record.id = id;
                (index_key(kind, &record), id)
            })
            .collect();
        keyed.sort();
        let sorted_ids: Vec<i64> = keyed.into_iter().map(|(_, id)| id).collect();

        assert_eq!(sorted_ids, ascending, "{kind:?}");
    }
}

#[test]
fn date_keys_follow_datetime_order() {
    // Later dates get smaller IDs so that only the date can explain the order.
    let texts = [
        "12/31/1969 11:59:59 PM",
        "01/01/1970 12:00:00 AM",
        "12/31/2001 11:59:59 PM",
        "01/01/2002 12:00:00 AM",
        "10/03/2026 12:00:00 AM",
        "10/03/2026 01:00:00 AM",
        "10/03/2026 12:00:00 PM",
    ];
    let mut keyed: Vec<(Vec<u8>, &str)> = texts
        .iter()
        .enumerate()
        .map(|(position, &text)| {
            let mut record = sample_record();
            record.id = 1_000 - position as i64;
            record.date = datetime(text);
            (index_key(IndexKind::Date, &record), text)
        })
        .collect();
    keyed.sort();
    let by_key: Vec<&str> = keyed.into_iter().map(|(_, text)| text).collect();

    assert_eq!(by_key, texts);
}

#[test]
fn the_five_indexes_have_stable_distinct_names_files_and_ordinals() {
    let names: Vec<&str> = IndexKind::ALL.iter().map(|kind| kind.name()).collect();
    let ordinals: Vec<u8> = IndexKind::ALL.iter().map(|kind| kind.ordinal()).collect();
    let files: HashSet<&str> = IndexKind::ALL.iter().map(|kind| kind.file_name()).collect();

    assert_eq!(
        names,
        [
            "date",
            "primary_type",
            "district",
            "community_area",
            "case_number"
        ]
    );
    assert_eq!(ordinals, [0, 1, 2, 3, 4]);
    assert_eq!(files.len(), IndexKind::ALL.len());
    for kind in IndexKind::ALL {
        assert_eq!(
            kind.file_name(),
            format!("{INDEX_FILE_PREFIX}{}{TABLE_FILE_EXTENSION}", kind.name())
        );
    }
}
