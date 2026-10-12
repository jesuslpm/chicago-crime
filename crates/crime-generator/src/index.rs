//! The stream of the secondary indexes (decision D2): every accepted record yields one
//! [`IndexEntry`] per index, and all of them go through one external sort ordered by
//! `(index ordinal, key)`, so the merged stream holds each index table in key order, one index
//! after the other.
//!
//! Index keys end with the ID, and IDs are unique once duplicates are skipped, so no two entries
//! are equal: the order is total and the tables do not depend on the segment size (decision G5).

use std::cmp::Ordering;
use std::io::{self, Read, Write};
use std::num::NonZeroUsize;

use crime_dataset::{IndexKind, decode_value, index_key};

use crate::Abort;
use crate::primary::PrimaryEntry;
use crate::sort::{Sortable, corrupt_item, read_item_rest, read_item_start};
use crate::table::TableStats;

/// Number of secondary indexes.
pub const INDEX_COUNT: usize = IndexKind::ALL.len();

/// Index entries held in memory before a segment of the index sort is written to disk.
///
/// A buffered entry costs at most about 100 bytes: 32 for the [`IndexEntry`] itself, its key in
/// a heap block of its own (keys average about 15 bytes, which the allocator rounds up to 32 to
/// 48 bytes) and up to 16 more for the scratch space of the stable sort. One million entries
/// therefore stay under 100 MiB next to the primary merge and table writer that run at the same
/// time, well under the 512 MiB budget, while the 43.3 million entries of the reference CSV need
/// only 44 segments, each read through a 64 KiB buffer during the merge. Measured on the
/// reference CSV: a peak of 93.4 MiB of private committed memory for the whole run.
pub const DEFAULT_INDEX_SEGMENT_SIZE: NonZeroUsize = NonZeroUsize::new(1_000_000).unwrap();

/// Bytes of an encoded entry before its key: the ordinal and the key length.
const ENTRY_HEADER_LEN: usize = 1 + 4;

/// One key of one index table, tagged with the ordinal of its index ([`IndexKind::ordinal`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub ordinal: u8,
    pub key: Vec<u8>,
}

impl IndexEntry {
    /// Sort order: by index ordinal, then by key bytes, which is the table order.
    pub fn compare(a: &Self, b: &Self) -> Ordering {
        a.ordinal.cmp(&b.ordinal).then_with(|| a.key.cmp(&b.key))
    }
}

/// Segment format: the ordinal as one byte, the key length as a little-endian `u32`, then the
/// key.
impl Sortable for IndexEntry {
    fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let length = u32::try_from(self.key.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "an index key longer than 4 GiB cannot be sorted",
            )
        })?;
        writer.write_all(&[self.ordinal])?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(&self.key)
    }

    /// A segment that ends in the middle of an entry, or names an index that does not exist, is
    /// reported as `InvalidData`.
    fn decode<R: Read>(reader: &mut R) -> io::Result<Option<Self>> {
        let mut header = [0; ENTRY_HEADER_LEN];
        if !read_item_start(reader, &mut header)? {
            return Ok(None);
        }
        let [ordinal, length @ ..] = header;
        if !IndexKind::ALL.iter().any(|kind| kind.ordinal() == ordinal) {
            return Err(corrupt_item(
                "a sort segment holds an entry of an unknown index",
            ));
        }
        let mut key = vec![0; u32::from_le_bytes(length) as usize];
        read_item_rest(reader, &mut key)?;
        Ok(Some(Self { ordinal, key }))
    }
}

/// The index entries of an accepted primary entry, one per index in [`IndexKind::ALL`] order.
///
/// The record is decoded back from the stored value, so the indexes describe exactly what the
/// primary table holds. A value that does not decode is a defect: this run encoded it.
pub fn entries_of(entry: &PrimaryEntry) -> Result<[IndexEntry; INDEX_COUNT], Abort> {
    let record =
        decode_value(&entry.value).map_err(|_| Abort::UndecodableRecord { id: entry.id })?;
    Ok(IndexKind::ALL.map(|kind| IndexEntry {
        ordinal: kind.ordinal(),
        key: index_key(kind, &record),
    }))
}

/// A finished index table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexTable {
    pub kind: IndexKind,
    pub stats: TableStats,
}

/// Every index must hold exactly one entry per record of the primary table (`valid`); the first
/// index that does not is reported as an internal error.
pub fn check_index_counts(valid: u64, tables: &[IndexTable]) -> Result<(), Abort> {
    match tables.iter().find(|table| table.stats.entries != valid) {
        Some(table) => Err(Abort::IndexCountMismatch {
            index: table.kind.name(),
            entries: table.stats.entries,
            valid,
        }),
        None => Ok(()),
    }
}
