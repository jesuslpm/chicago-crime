//! External sort of primary entries: order, segments on disk and their cleanup.

mod common;

use std::io::{self, Cursor};
use std::num::NonZeroUsize;
use std::path::Path;

use common::folder_entries;
use crime_generator::primary::PrimaryEntry;
use crime_generator::sort::{ExternalSort, SORT_DIR_PREFIX, Sortable, Sorted};

fn entry(id: i64, line: u64) -> PrimaryEntry {
    PrimaryEntry {
        id,
        line,
        value: format!("value of {id} at line {line}").into_bytes(),
    }
}

fn sort(temp_dir: &Path, segment_size: usize, entries: Vec<PrimaryEntry>) -> Sorted<PrimaryEntry> {
    let size = NonZeroUsize::new(segment_size).expect("a positive segment size");
    let mut sort =
        ExternalSort::new(temp_dir, size, PrimaryEntry::compare).expect("the sort can start");
    for entry in entries {
        sort.push(entry).expect("the entry can be pushed");
    }
    sort.finish().expect("the sort can finish")
}

fn collect(sorted: &mut Sorted<PrimaryEntry>) -> Vec<PrimaryEntry> {
    sorted
        .by_ref()
        .collect::<io::Result<_>>()
        .expect("the merged stream can be read")
}

#[test]
fn entries_come_back_ordered_by_id_then_line_across_segments() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = vec![
        entry(30, 2),
        entry(10, 9),
        entry(20, 4),
        entry(10, 3),
        entry(-5, 7),
        entry(30, 1),
        entry(10, 5),
        PrimaryEntry {
            id: 0,
            line: 8,
            value: Vec::new(),
        },
    ];
    let mut expected = input.clone();
    expected.sort_by_key(|entry| (entry.id, entry.line));

    let mut sorted = sort(dir.path(), 3, input);

    assert_eq!(
        sorted.segment_count(),
        3,
        "eight entries in segments of three"
    );
    assert_eq!(collect(&mut sorted), expected);
    sorted.close().expect("the sort folder can be removed");
}

#[test]
fn inputs_that_fill_their_last_segment_exactly_are_merged_in_full() {
    // `extsort` 0.5.0 returned an empty stream in exactly these cases.
    for (count, segment_size) in [(6, 2), (9, 3), (7, 7), (8, 1), (500, 7)] {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let input: Vec<_> = (0..count).rev().map(|id| entry(id, 1)).collect();

        let mut sorted = sort(dir.path(), segment_size, input);

        let segments = (count as usize).div_ceil(segment_size);
        assert_eq!(
            sorted.segment_count(),
            segments,
            "{count} in {segment_size}"
        );
        let ids: Vec<i64> = collect(&mut sorted).iter().map(|entry| entry.id).collect();
        assert_eq!(
            ids,
            (0..count).collect::<Vec<_>>(),
            "{count} in {segment_size}"
        );
    }
}

#[test]
fn equal_items_keep_their_input_order_across_segments() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let by_id = |a: &PrimaryEntry, b: &PrimaryEntry| a.id.cmp(&b.id);
    let input: Vec<_> = [(2, 1), (1, 2), (2, 3), (1, 4), (2, 5), (1, 6), (2, 7)]
        .into_iter()
        .map(|(id, line)| entry(id, line))
        .collect();
    let size = NonZeroUsize::new(2).expect("positive");
    let mut sort = ExternalSort::new(dir.path(), size, by_id).expect("the sort can start");
    for item in input {
        sort.push(item).expect("the entry can be pushed");
    }

    let mut sorted = sort.finish().expect("the sort can finish");

    let order: Vec<(i64, u64)> = collect(&mut sorted)
        .iter()
        .map(|entry| (entry.id, entry.line))
        .collect();
    assert_eq!(
        order,
        [(1, 2), (1, 4), (1, 6), (2, 1), (2, 3), (2, 5), (2, 7)]
    );
}

#[test]
fn fewer_entries_than_a_segment_never_touch_the_disk() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");

    let mut sorted = sort(dir.path(), 4, vec![entry(2, 2), entry(1, 3), entry(3, 4)]);

    assert_eq!(sorted.segment_count(), 0);
    assert_eq!(
        collect(&mut sorted),
        [entry(1, 3), entry(2, 2), entry(3, 4)]
    );
}

#[test]
fn segments_live_in_their_own_folder_inside_the_temp_dir_until_closed() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let unrelated = dir.path().join("0");
    std::fs::write(&unrelated, b"not a segment").expect("the fixture can be written");

    let mut sorted = sort(
        dir.path(),
        2,
        (1..=6).rev().map(|id| entry(id, 1)).collect(),
    );

    let names = folder_entries(dir.path());
    assert_eq!(
        names.len(),
        2,
        "the unrelated file and one sort folder: {names:?}"
    );
    let sort_dir = names
        .iter()
        .find(|name| name.starts_with(SORT_DIR_PREFIX))
        .unwrap_or_else(|| panic!("a folder named {SORT_DIR_PREFIX}*: {names:?}"));
    assert_eq!(
        folder_entries(&dir.path().join(sort_dir)).len(),
        3,
        "three segments of two"
    );
    assert_eq!(
        std::fs::read(&unrelated).expect("the file can be read"),
        b"not a segment",
        "a file named like a segment is never touched"
    );

    assert_eq!(collect(&mut sorted).len(), 6);
    sorted.close().expect("the sort folder can be removed");
    assert_eq!(folder_entries(dir.path()), ["0"]);
}

#[test]
fn dropping_the_merged_stream_midway_removes_the_segments() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let mut sorted = sort(dir.path(), 2, (1..=9).map(|id| entry(id, 1)).collect());
    assert!(sorted.segment_count() > 0);

    let first = sorted.next().expect("an entry").expect("readable");
    drop(sorted);

    assert_eq!(first, entry(1, 1));
    assert_eq!(folder_entries(dir.path()), Vec::<String>::new());
}

#[test]
fn dropping_an_unfinished_sort_removes_the_segments() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let size = NonZeroUsize::new(1).expect("positive");
    let mut sort =
        ExternalSort::new(dir.path(), size, PrimaryEntry::compare).expect("the sort can start");
    for id in 1..=3 {
        sort.push(entry(id, 1)).expect("the entry can be pushed");
    }

    drop(sort);

    assert_eq!(folder_entries(dir.path()), Vec::<String>::new());
}

#[test]
fn encoded_entries_decode_to_themselves() {
    let entries = [
        entry(i64::MIN, 0),
        entry(i64::MAX, u64::MAX),
        PrimaryEntry {
            id: 1,
            line: 2,
            value: Vec::new(),
        },
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
            PrimaryEntry::decode(&mut reader).expect("decodes").as_ref(),
            Some(expected)
        );
    }
    let end = PrimaryEntry::decode(&mut reader).expect("a clean end is not an error");
    assert_eq!(end, None);
}

#[test]
fn a_truncated_entry_is_corruption_not_the_end_of_the_segment() {
    let mut bytes = Vec::new();
    entry(7, 7)
        .encode(&mut bytes)
        .expect("encoding to memory succeeds");

    for cut in [1, 10, 19, 20, bytes.len() - 1] {
        let error = PrimaryEntry::decode(&mut Cursor::new(&bytes[..cut]))
            .expect_err("a partial entry cannot decode");
        assert_eq!(
            error.kind(),
            io::ErrorKind::InvalidData,
            "cut after {cut} bytes"
        );
    }
}
