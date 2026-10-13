//! Table files read through a wrapper that checks every read and remembers every failure.
//!
//! The `sstable` 0.11 reader cannot be trusted to report a failed read, which would turn a damaged
//! or vanishing file into a silently shorter result (the child PRD asks to abort "sin salida
//! parcial engañosa"):
//!
//! - `TableIterator::advance` swallows the error of a block it cannot read, a block that fails
//!   its checksum included, and goes on with the next block, and `seek` turns the error into an
//!   invalid iterator, the same state as a seek past the last key;
//! - block reads ignore the byte count `read_at` returns, so a short read at the end of a
//!   truncated file becomes a block of zeros that fails its checksum and is skipped as above.
//!
//! [`TrackedFile`] therefore reads every requested byte or fails, verifies the checksum of every
//! table block it reads, and records the first failure in a [`ReadHealth`] shared by every table
//! of the dataset. Every dataset operation checks it after it used a table and turns a recorded
//! failure into an error, whatever the table reader returned. The check is sticky: once a read
//! failed, the snapshot is no longer trusted.
//!
//! The table reader makes two kinds of reads (`table_reader.rs` and `table_block.rs` in
//! `sstable`):
//!
//! - the footer: the last [`TABLE_FOOTER_BYTES`] bytes of the file, which have no checksum;
//! - a table block (a data, index or metaindex block): one read of its content, its compression
//!   type byte and the 4-byte masked CRC32C of content and type byte, the layout
//!   `TableBuilder::write_block` writes. A block never reaches the end of the file, because the
//!   footer follows the last one.
//!
//! The reader would also read a filter block, without its trailer, if the metaindex held an
//! entry at or after `filter.<name of the reader's filter policy>`. The generator builds its
//! tables with `TableBuilder::new_no_filter`, whose metaindex holds only `filter._`; the query
//! app opens them with the default Bloom policy, `filter.leveldb.BuiltinBloomFilter2`, which
//! sorts after it, so the lookup finds nothing and no filter block is ever read (a unit test
//! pins that order). A table whose metaindex made the reader read a filter block would fail the
//! block check and be reported unreadable: it is not a table of this dataset format.

use std::fmt::Display;
use std::fs::File;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use sstable::{RandomAccess, Status, StatusCode};

/// Bytes at the end of every table file: two block handles, padding and the magic number.
pub(crate) const TABLE_FOOTER_BYTES: u64 = 48;

/// Bytes after the content of every table block: the compression type and the masked CRC32C.
const BLOCK_TRAILER_BYTES: usize = 5;

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

/// An opened table file that reads every requested byte and verifies every table block it
/// reads, or records why it could not.
pub(crate) struct TrackedFile {
    file: File,
    /// File name inside the version folder, for the recorded cause.
    name: &'static str,
    /// Size of the file when it was opened: the footer is the read that ends there.
    size: u64,
    health: Arc<ReadHealth>,
}

impl TrackedFile {
    pub(crate) fn new(file: File, name: &'static str, size: u64, health: Arc<ReadHealth>) -> Self {
        Self {
            file,
            name,
            size,
            health,
        }
    }

    /// Whether `length` bytes from `offset` on are the footer, the only read without a checksum.
    fn is_footer(&self, offset: u64, length: usize) -> bool {
        length as u64 == TABLE_FOOTER_BYTES
            && offset.checked_add(TABLE_FOOTER_BYTES) == Some(self.size)
    }

    fn fail(&self, offset: usize, cause: impl Display) {
        self.health
            .record(format!("{} at offset {offset}: {cause}", self.name));
    }
}

impl RandomAccess for TrackedFile {
    fn read_at(&self, offset: usize, destination: &mut [u8]) -> sstable::Result<usize> {
        if let Err(error) = read_exact_at(&self.file, offset as u64, destination) {
            self.fail(offset, &error);
            return Err(Status::from(error));
        }
        if !self.is_footer(offset as u64, destination.len())
            && let Err(cause) = verify_block(destination)
        {
            self.fail(offset, &cause);
            return Err(Status::new(StatusCode::Corruption, &cause));
        }
        Ok(destination.len())
    }
}

/// Checks a table block read with its trailer: the masked CRC32C in its last 4 bytes must be
/// the CRC32C of everything before them, the content and the compression type byte.
fn verify_block(block: &[u8]) -> Result<(), String> {
    if block.len() < BLOCK_TRAILER_BYTES {
        return Err(format!(
            "a read of {} bytes is neither the footer nor a table block",
            block.len()
        ));
    }
    let (covered, stored) = block.split_at(block.len() - 4);
    let stored = unmask_crc(u32::from_le_bytes(
        stored.try_into().expect("a 4-byte checksum"),
    ));
    let computed = crc32c(covered);
    if stored == computed {
        Ok(())
    } else {
        Err(format!(
            "a block of {} bytes fails its checksum (stored {stored:08x}, computed {computed:08x})",
            covered.len() - 1
        ))
    }
}

/// The reflected CRC32C (Castagnoli) polynomial.
const CRC32C_POLYNOMIAL: u32 = 0x82f6_3b78;

/// The CRC32C step of every byte value, for the byte-at-a-time computation.
const CRC32C_TABLE: [u32; 256] = crc32c_table();

const fn crc32c_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut crc = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ CRC32C_POLYNOMIAL
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[byte] = crc;
        byte += 1;
    }
    table
}

/// The CRC32C (Castagnoli) of `bytes`, the checksum LevelDB tables keep for every block.
fn crc32c(bytes: &[u8]) -> u32 {
    let crc = bytes.iter().fold(!0u32, |crc, &byte| {
        CRC32C_TABLE[usize::from(crc as u8 ^ byte)] ^ (crc >> 8)
    });
    !crc
}

/// Added by the LevelDB mask after rotating the CRC.
const CRC_MASK_DELTA: u32 = 0xa282_ead8;

/// The CRC32C a block trailer stores in masked form (LevelDB `crc32c::Unmask`).
fn unmask_crc(masked: u32) -> u32 {
    masked.wrapping_sub(CRC_MASK_DELTA).rotate_left(15)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sstable::filter::{BloomPolicy, FilterPolicy, NoFilterPolicy};

    /// The LevelDB mask (`crc32c::Mask`), which table builders apply before storing a CRC.
    fn mask_crc(crc: u32) -> u32 {
        crc.rotate_right(15).wrapping_add(CRC_MASK_DELTA)
    }

    #[test]
    fn crc32c_matches_the_known_vectors() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
        // The vectors of RFC 3720 (iSCSI), also in LevelDB's crc32c tests.
        assert_eq!(crc32c(&[0; 32]), 0x8a91_36aa);
        assert_eq!(crc32c(&[0xff; 32]), 0x62a8_ab43);
        let ascending: Vec<u8> = (0..32).collect();
        assert_eq!(crc32c(&ascending), 0x46dd_794e);
        let descending: Vec<u8> = (0..32).rev().collect();
        assert_eq!(crc32c(&descending), 0x113f_db5c);
    }

    #[test]
    fn unmask_undoes_the_leveldb_mask() {
        assert_eq!(mask_crc(0xe306_9283), 0xc78a_b0e5);
        assert_eq!(unmask_crc(0xc78a_b0e5), 0xe306_9283);
        for crc in [0, 1, 0x8000_0000, 0xffff_ffff, 0x1234_5678] {
            assert_ne!(mask_crc(crc), crc);
            assert_eq!(unmask_crc(mask_crc(crc)), crc);
        }
    }

    /// A block as `TableBuilder::write_block` writes it: content, type byte, masked CRC32C.
    fn block(content: &[u8], compression: u8) -> Vec<u8> {
        let mut block = content.to_vec();
        block.push(compression);
        let crc = mask_crc(crc32c(&block));
        block.extend_from_slice(&crc.to_le_bytes());
        block
    }

    #[test]
    fn a_block_passes_only_with_its_own_checksum() {
        for (content, compression) in [(&b"some block content"[..], 1), (b"", 0)] {
            let good = block(content, compression);
            assert_eq!(verify_block(&good), Ok(()));
            for position in 0..good.len() {
                let mut bad = good.clone();
                bad[position] ^= 0x01;
                assert!(verify_block(&bad).is_err(), "byte {position} flipped");
            }
            assert!(verify_block(&vec![0; good.len()]).is_err());
        }
        assert!(verify_block(&[0; 4]).is_err());
    }

    #[test]
    fn only_the_last_footer_bytes_of_the_file_skip_the_check() {
        let file = tempfile::tempfile().expect("a temporary file can be created");
        let tracked = TrackedFile::new(file, "test", 1_000, ReadHealth::new());
        assert!(tracked.is_footer(952, 48));
        assert!(!tracked.is_footer(951, 48));
        assert!(!tracked.is_footer(953, 47));
        assert!(!tracked.is_footer(900, 100));
        assert!(!tracked.is_footer(u64::MAX, 48));
    }

    /// The generator writes `filter._`; the reader looks up the entry of its default Bloom
    /// policy, and the metaindex lookup only finds a filter at or after that key.
    #[test]
    fn the_filter_of_a_generated_table_sorts_before_the_one_the_reader_looks_up() {
        let written = format!("filter.{}", NoFilterPolicy::new().name());
        let looked_up = format!("filter.{}", BloomPolicy::new(10).name());
        assert_eq!(written, "filter._");
        assert!(written.as_bytes() < looked_up.as_bytes(), "{looked_up}");
        let default_policy = sstable::Options::default().filter_policy;
        assert_eq!(default_policy.name(), BloomPolicy::new(10).name());
    }
}
