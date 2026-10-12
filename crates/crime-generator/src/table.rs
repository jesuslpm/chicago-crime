//! Writer of one table file in the LevelDB table format (decision D1), on top of the `sstable`
//! crate.
//!
//! Three properties of `sstable` 0.11 shape this wrapper:
//!
//! - `TableBuilder::add` asserts key order and panics on a repeated or decreasing key, so the
//!   order is checked here before every add and reported as an error instead;
//! - the builder writes with `Write::write` and ignores short writes, so the destination is
//!   wrapped in a writer that always writes everything;
//! - its errors are `Status` values that drop the `io::ErrorKind`, so the original I/O error is
//!   kept aside to tell a full disk from other failures.
//!
//! Tables carry no Bloom filter: every query run (decision D6) would read the whole filter, about
//! 10 bits per key, when it opens a table, only to skip one block read for an ID that does not
//! exist; and index tables are only scanned by prefix, where whole-key filters never apply.
//! Tables have no timestamps or other run-dependent data, so the same entries always produce
//! the same bytes (decision G5).

use std::cell::RefCell;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::rc::Rc;

use crime_dataset::Compression;
use sstable::{CompressionType, Options, Status, TableBuilder};

/// Buffer between the table builder, which writes one block at a time, and the file.
const WRITE_BUFFER_BYTES: usize = 1 << 20;

/// Size and entry count of a finished table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableStats {
    pub entries: u64,
    pub size_bytes: u64,
}

/// Why a table could not be written.
#[derive(Debug)]
pub enum TableError {
    /// `key` is not strictly greater than the `previous` key; it was not added.
    OutOfOrder { previous: Vec<u8>, key: Vec<u8> },
    /// Creating or writing the file failed.
    Io(io::Error),
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableError::OutOfOrder { previous, key } => write!(
                f,
                "key {key:02x?} is not greater than the previous key {previous:02x?}"
            ),
            TableError::Io(error) => write!(f, "the table cannot be written: {error}"),
        }
    }
}

impl std::error::Error for TableError {}

/// Writes one table file. Keys must arrive in strictly increasing byte order.
pub struct TableWriter {
    builder: TableBuilder<WriteAll>,
    /// The I/O error behind the last failed write, if any (see [`WriteAll`]).
    failure: Rc<RefCell<Option<io::Error>>>,
    /// Set after a failed write: the builder can be left mid-block and would panic if used again.
    broken: bool,
    last_key: Option<Vec<u8>>,
    entries: u64,
    /// A second handle on the table file, to sync it once the builder has written and flushed
    /// everything: the builder owns the destination and drops it when it finishes.
    sync: Option<File>,
}

impl TableWriter {
    /// Creates the table file, which must not exist yet: a table never replaces another file.
    pub fn create(path: &Path, compression: Compression) -> Result<Self, TableError> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(TableError::Io)?;
        let sync = file.try_clone().map_err(TableError::Io)?;
        Ok(Self::new(Box::new(file), Some(sync), compression))
    }

    /// A table written to any destination, such as a test double. [`TableWriter::finish`]
    /// flushes it; only a table made by [`TableWriter::create`] is also synced to disk.
    pub fn from_writer(destination: impl Write + 'static, compression: Compression) -> Self {
        Self::new(Box::new(destination), None, compression)
    }

    fn new(destination: Box<dyn Write>, sync: Option<File>, compression: Compression) -> Self {
        let failure = Rc::default();
        let destination = WriteAll {
            inner: BufWriter::with_capacity(WRITE_BUFFER_BYTES, destination),
            failure: Rc::clone(&failure),
        };
        let options = Options {
            compression_type: match compression {
                Compression::Snappy => CompressionType::CompressionSnappy,
                Compression::None => CompressionType::CompressionNone,
            },
            ..Options::default()
        };
        Self {
            builder: TableBuilder::new_no_filter(options, destination),
            failure,
            broken: false,
            last_key: None,
            entries: 0,
            sync,
        }
    }

    /// Adds one entry. A key that is not strictly greater than the previous one is refused and
    /// the table stays usable; after an I/O error every later call fails.
    pub fn add(&mut self, key: &[u8], value: &[u8]) -> Result<(), TableError> {
        if self.broken {
            return Err(already_failed());
        }
        if let Some(previous) = &self.last_key
            && key <= previous.as_slice()
        {
            return Err(TableError::OutOfOrder {
                previous: previous.clone(),
                key: key.to_vec(),
            });
        }
        if let Err(status) = self.builder.add(key, value) {
            self.broken = true;
            return Err(original_error(&self.failure, status));
        }
        match &mut self.last_key {
            Some(last_key) => {
                last_key.clear();
                last_key.extend_from_slice(key);
            }
            None => self.last_key = Some(key.to_vec()),
        }
        self.entries += 1;
        Ok(())
    }

    /// Writes the index and the footer, flushes the file and syncs it to disk, so that a failure
    /// of the last writes (a full disk, for example) is reported here and never after the table
    /// is published.
    pub fn finish(self) -> Result<TableStats, TableError> {
        let Self {
            builder,
            failure,
            broken,
            entries,
            sync,
            ..
        } = self;
        if broken {
            return Err(already_failed());
        }
        let size_bytes = builder
            .finish()
            .map_err(|status| original_error(&failure, status))?;
        if let Some(file) = sync {
            file.sync_all().map_err(TableError::Io)?;
        }
        Ok(TableStats {
            entries,
            size_bytes: size_bytes as u64,
        })
    }
}

fn already_failed() -> TableError {
    TableError::Io(io::Error::other("an earlier write to this table failed"))
}

/// The I/O error that caused `status`, or one built from it (compression errors, for example).
fn original_error(failure: &RefCell<Option<io::Error>>, status: Status) -> TableError {
    // `Status` implements `Display` by calling itself, so only its fields are used.
    let error = failure
        .borrow_mut()
        .take()
        .unwrap_or_else(|| io::Error::other(status.err));
    TableError::Io(error)
}

/// Destination of the table builder: every `write` writes the whole buffer or fails, and the
/// original error is kept for [`TableWriter`], because the builder turns it into a `Status`.
struct WriteAll {
    inner: BufWriter<Box<dyn Write>>,
    failure: Rc<RefCell<Option<io::Error>>>,
}

impl WriteAll {
    /// Keeps `error` and returns a copy of its kind for the builder.
    fn keep(&self, error: io::Error) -> io::Error {
        let kind = error.kind();
        *self.failure.borrow_mut() = Some(error);
        io::Error::from(kind)
    }
}

impl Write for WriteAll {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.inner.write_all(buf) {
            Ok(()) => Ok(buf.len()),
            Err(error) => Err(self.keep(error)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush().map_err(|error| self.keep(error))
    }
}
