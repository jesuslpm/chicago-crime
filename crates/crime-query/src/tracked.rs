//! Table files read through a wrapper that remembers every read failure.
//!
//! The `sstable` 0.11 reader cannot be trusted to report a failed read, which would turn a damaged
//! or vanishing file into a silently shorter result (the child PRD asks to abort "sin salida
//! parcial engañosa"):
//!
//! - `TableIterator::advance` swallows the error of a block it cannot read and goes on with the
//!   next block, and `seek` turns the error into an invalid iterator, the same state as a seek
//!   past the last key;
//! - block reads ignore the byte count `read_at` returns, so a short read at the end of a
//!   truncated file becomes a block of zeros that fails its checksum and is skipped as above.
//!
//! [`TrackedFile`] therefore reads every requested byte or fails, and records the first failure
//! in a [`ReadHealth`] shared by every table of the dataset. Every dataset operation checks it
//! after it used a table and turns a recorded failure into an error, whatever the table reader
//! returned. The check is sticky: once a read failed, the snapshot is no longer trusted.
//!
//! Known limit: a block whose bytes are read completely but fail their checksum (corruption on
//! disk) is still skipped silently by the iterator, because no read failed.

use std::fs::File;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use sstable::{RandomAccess, Status};

/// Whether any read of the dataset has failed, and why the first one did.
#[derive(Debug, Default)]
pub(crate) struct ReadHealth {
    failed: AtomicBool,
    first_cause: Mutex<Option<String>>,
}

impl ReadHealth {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Records a failed read; only the first cause is kept.
    fn record(&self, cause: String) {
        let mut first = self
            .first_cause
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        first.get_or_insert(cause);
        self.failed.store(true, Ordering::Release);
    }

    /// The cause of the first failed read, if any read failed.
    pub(crate) fn failure(&self) -> Option<String> {
        if !self.failed.load(Ordering::Acquire) {
            return None;
        }
        let first = self
            .first_cause
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        Some(first.clone().unwrap_or_default())
    }
}

/// An opened table file that reads every requested byte or records why it could not.
pub(crate) struct TrackedFile {
    file: File,
    /// File name inside the version folder, for the recorded cause.
    name: &'static str,
    health: Arc<ReadHealth>,
}

impl TrackedFile {
    pub(crate) fn new(file: File, name: &'static str, health: Arc<ReadHealth>) -> Self {
        Self { file, name, health }
    }
}

impl RandomAccess for TrackedFile {
    fn read_at(&self, offset: usize, destination: &mut [u8]) -> sstable::Result<usize> {
        match read_exact_at(&self.file, offset as u64, destination) {
            Ok(()) => Ok(destination.len()),
            Err(error) => {
                self.health
                    .record(format!("{} at offset {offset}: {error}", self.name));
                Err(Status::from(error))
            }
        }
    }
}

/// Fills `destination` from `offset` on; reaching the end of the file first is an error.
pub(crate) fn read_exact_at(file: &File, offset: u64, destination: &mut [u8]) -> io::Result<()> {
    let mut filled = 0;
    while filled < destination.len() {
        match read_once_at(file, offset + filled as u64, &mut destination[filled..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the file ended before the requested bytes",
                ));
            }
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn read_once_at(file: &File, offset: u64, destination: &mut [u8]) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, destination, offset)
}

#[cfg(unix)]
fn read_once_at(file: &File, offset: u64, destination: &mut [u8]) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, destination, offset)
}
