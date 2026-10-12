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

/// A destination that accepts `limit` bytes and then fails every write with `kind`, like a disk
/// that fills up.
struct FailingWriter {
    written: usize,
    limit: usize,
    kind: io::ErrorKind,
}

impl FailingWriter {
    fn full_after(limit: usize) -> Self {
        Self {
            written: 0,
            limit,
            kind: io::ErrorKind::StorageFull,
        }
    }
}

impl io::Write for FailingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.written + buf.len() > self.limit {
            return Err(io::Error::from(self.kind));
        }
        self.written += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn assert_io_error(result: Result<impl std::fmt::Debug, TableError>, kind: io::ErrorKind) {
    match result {
        Err(TableError::Io(error)) => assert_eq!(error.kind(), kind, "{error}"),
        other => panic!("expected an I/O error of kind {kind:?}, got {other:?}"),
    }
}

#[test]
fn a_write_failure_while_adding_keeps_its_kind_and_every_later_call_fails() {
    let mut writer = TableWriter::from_writer(FailingWriter::full_after(0), Compression::None);
    let value = vec![b'v'; 64 * 1024];

    // Blocks go through a write buffer, so the destination only fails once the buffer is full.
    let mut index = 0u32;
    let failure = loop {
        match writer.add(&index.to_be_bytes(), &value) {
            Ok(()) => index += 1,
            Err(error) => break error,
        }
        assert!(index < 1_000, "the full destination must fail an add");
    };

    assert_io_error(Err::<(), _>(failure), io::ErrorKind::StorageFull);
    assert_io_error(
        writer.add(&(index + 1).to_be_bytes(), &value),
        io::ErrorKind::Other,
    );
    assert_io_error(writer.finish(), io::ErrorKind::Other);
}

#[test]
fn a_full_disk_at_the_last_write_is_reported_by_finish() {
    let mut writer = TableWriter::from_writer(FailingWriter::full_after(100), Compression::Snappy);
    for key in [b"a", b"b", b"c"] {
        writer
            .add(key, b"small value")
            .expect("small entries stay in the buffer");
    }

    assert_io_error(writer.finish(), io::ErrorKind::StorageFull);
}

#[test]
fn a_table_written_to_any_destination_has_the_same_bytes_as_its_file() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let (path, mut file_writer) = new_writer(&dir);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut memory_writer =
        TableWriter::from_writer(SharedBuffer(shared.clone()), Compression::Snappy);
    for index in 0..500u32 {
        let value = format!("value {index}");
        file_writer
            .add(&index.to_be_bytes(), value.as_bytes())
            .expect("added");
        memory_writer
            .add(&index.to_be_bytes(), value.as_bytes())
            .expect("added");
    }

    let file_stats = file_writer
        .finish()
        .expect("the file table can be finished");
    let memory_stats = memory_writer
        .finish()
        .expect("the memory table can be finished");

    assert_eq!(file_stats, memory_stats);
    assert_eq!(
        *shared.borrow(),
        fs::read(&path).expect("the table can be read")
    );
}

/// An in-memory destination the test can still read after the writer took it.
struct SharedBuffer(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
