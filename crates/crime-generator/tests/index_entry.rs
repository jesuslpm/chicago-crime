//! Index entries: how an accepted record becomes five entries, their sort order and their
//! segment format.

mod common;

use std::io::{self, Cursor};
use std::num::NonZeroUsize;

use common::{distinct_record, sample_record};
use crime_dataset::{IndexKind, encode_value, index_key};
use crime_generator::Abort;
use crime_generator::index::{IndexEntry, entries_of};
use crime_generator::primary::PrimaryEntry;
use crime_generator::sort::{ExternalSort, Sortable};

fn entry(ordinal: u8, key: &[u8]) -> IndexEntry {
    IndexEntry {
        ordinal,
        key: key.to_vec(),
    }
}

#[test]
fn an_accepted_record_yields_one_entry_per_index_in_ordinal_order() {
    let record = sample_record();
    let primary = PrimaryEntry {
        id: record.id,
        line: 2,
        value: encode_value(&record),
    };

    let entries = entries_of(&primary).expect("the value decodes");

    let expected = IndexKind::ALL.map(|kind| IndexEntry {
        ordinal: kind.ordinal(),
        key: index_key(kind, &record),
    });
    assert_eq!(entries, expected);
}

#[test]
fn an_undecodable_primary_value_is_an_internal_error_naming_the_id() {
    let primary = PrimaryEntry {
        id: 77,
        line: 9,
        value: b"not a tuple \xff".to_vec(),
    };

    let abort = entries_of(&primary).expect_err("garbage cannot decode");

    assert_eq!(abort, Abort::UndecodableRecord { id: 77 });
}

#[test]
fn entries_order_by_index_then_by_key_bytes() {
    let mut entries = vec![
        entry(1, b"\x02B"),
        entry(0, b"\x02Z"),
        entry(1, b"\x00"),
        entry(4, b""),
        entry(1, b"\x02A\x00"),
        entry(1, b"\x02A"),
        entry(0, b"\x02A"),
    ];

    entries.sort_by(IndexEntry::compare);

    assert_eq!(
        entries,
        [
            entry(0, b"\x02A"),
            entry(0, b"\x02Z"),
            entry(1, b"\x00"),
            entry(1, b"\x02A"),
            entry(1, b"\x02A\x00"),
            entry(1, b"\x02B"),
            entry(4, b""),
        ]
    );
}

#[test]
fn the_external_sort_merges_index_entries_from_many_segments_in_order() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let records: Vec<_> = (1..=40).rev().map(distinct_record).collect();
    let size = NonZeroUsize::new(9).expect("positive");
    let mut sort = ExternalSort::new(dir.path(), size, IndexEntry::compare).expect("starts");
    let mut expected = Vec::new();
    for record in &records {
        let primary = PrimaryEntry {
            id: record.id,
            line: 2,
            value: encode_value(record),
        };
        for index_entry in entries_of(&primary).expect("decodes") {
            expected.push(index_entry.clone());
            sort.push(index_entry).expect("the entry can be pushed");
        }
    }
    expected.sort_by(|a, b| (a.ordinal, &a.key).cmp(&(b.ordinal, &b.key)));

    let mut sorted = sort.finish().expect("the sort can finish");

    assert_eq!(
        sorted.segment_count(),
        23,
        "200 entries in segments of nine"
    );
    let merged: Vec<IndexEntry> = sorted
        .by_ref()
        .collect::<io::Result<_>>()
        .expect("the merged stream can be read");
    assert_eq!(merged, expected);
    sorted.close().expect("the sort folder can be removed");
}

#[test]
fn encoded_entries_decode_to_themselves() {
    let long_key = vec![0xab; 70_000];
    let entries = [
        entry(0, b""),
        entry(4, &long_key),
        entry(2, b"\x00\x15\x08"),
    ];
    let mut bytes = Vec::new();
    for entry in &entries {
        entry
            .encode(&mut bytes)
            .expect("encoding to memory succeeds");
    }

    let mut reader = Cursor::new(bytes);
    for expected in &entries {
        assert_eq!(
            IndexEntry::decode(&mut reader).expect("decodes").as_ref(),
            Some(expected)
        );
    }
    assert_eq!(
        IndexEntry::decode(&mut reader).expect("a clean end is not an error"),
        None
    );
}

#[test]
fn a_truncated_entry_is_corruption_not_the_end_of_the_segment() {
    let mut bytes = Vec::new();
    entry(3, b"\x02THEFT\x00\x15\x07")
        .encode(&mut bytes)
        .expect("encoding to memory succeeds");

    for cut in [1, 4, 5, bytes.len() - 1] {
        let error = IndexEntry::decode(&mut Cursor::new(&bytes[..cut]))
            .expect_err("a partial entry cannot decode");
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidData,
            "cut after {cut} bytes"
        );
    }
}

#[test]
fn an_unknown_index_ordinal_is_corruption() {
    let mut bytes = Vec::new();
    entry(5, b"\x15\x01")
        .encode(&mut bytes)
        .expect("encoding to memory succeeds");

    let error = IndexEntry::decode(&mut Cursor::new(bytes)).expect_err("there are five indexes");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}
