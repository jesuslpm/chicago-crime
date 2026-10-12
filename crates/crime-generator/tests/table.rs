//! The table writer: LevelDB table format, compression, and key order checked before every add.

mod common;

use std::fs;
use std::io;

use common::read_table;
use crime_dataset::Compression;
use crime_generator::table::{TableError, TableWriter};

fn new_writer(dir: &tempfile::TempDir) -> (std::path::PathBuf, TableWriter) {
    let path = dir.path().join("table.sst");
    let writer = TableWriter::create(&path, Compression::Snappy).expect("the table can be created");
    (path, writer)
}

fn assert_out_of_order(result: Result<(), TableError>, previous: &[u8], key: &[u8]) {
    match result {
        Err(TableError::OutOfOrder {
            previous: found_previous,
            key: found_key,
        }) => {
            assert_eq!(found_previous, previous);
            assert_eq!(found_key, key);
        }
        other => panic!("expected an out-of-order error, got {other:?}"),
    }
}

#[test]
fn an_equal_key_is_refused_without_panicking() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let (_, mut writer) = new_writer(&dir);
    writer.add(b"b", b"1").expect("the first key is accepted");

    assert_out_of_order(writer.add(b"b", b"2"), b"b", b"b");
}

#[test]
fn a_decreasing_key_is_refused_without_panicking() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let (_, mut writer) = new_writer(&dir);
    writer.add(b"b", b"1").expect("the first key is accepted");

    assert_out_of_order(writer.add(b"a", b"2"), b"b", b"a");
}

#[test]
fn a_refused_key_leaves_the_table_usable() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let (path, mut writer) = new_writer(&dir);
    writer.add(b"a", b"1").expect("the first key is accepted");
    writer
        .add(b"a", b"again")
        .expect_err("a repeated key is refused");
    writer.add(b"c", b"3").expect("a greater key is accepted");

    let stats = writer.finish().expect("the table can be finished");

    assert_eq!(stats.entries, 2);
    assert_eq!(
        stats.size_bytes,
        fs::metadata(&path).expect("metadata").len()
    );
    assert_eq!(
        read_table(&path),
        [
            (b"a".to_vec(), b"1".to_vec()),
            (b"c".to_vec(), b"3".to_vec())
        ]
    );
}

#[test]
fn keys_out_of_order_across_many_blocks_are_still_refused() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let (path, mut writer) = new_writer(&dir);
    let value = vec![b'x'; 200];
    for index in 0..1_000u32 {
        writer
            .add(&index.to_be_bytes(), &value)
            .expect("increasing keys are accepted");
    }

    assert_out_of_order(
        writer.add(&10u32.to_be_bytes(), &value),
        &999u32.to_be_bytes(),
        &10u32.to_be_bytes(),
    );
    let stats = writer.finish().expect("the table can be finished");
    assert_eq!(stats.entries, 1_000);
    assert_eq!(read_table(&path).len(), 1_000);
}

#[test]
fn an_existing_file_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let path = dir.path().join("table.sst");
    fs::write(&path, b"keep me").expect("the fixture can be written");

    let error = TableWriter::create(&path, Compression::None)
        .err()
        .expect("an existing file is refused");

    match error {
        TableError::Io(error) => assert_eq!(error.kind(), io::ErrorKind::AlreadyExists),
        other => panic!("expected an I/O error, got {other:?}"),
    }
    assert_eq!(fs::read(&path).expect("the file can be read"), b"keep me");
}
