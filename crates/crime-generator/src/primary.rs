//! The stream of the primary table: each accepted row becomes a [`PrimaryEntry`] that is sorted
//! on disk by `(id, line)`, and the merged stream goes through [`Dedup`], which applies the
//! duplicate rule (decision D5) before the entries reach the table.

use std::cmp::Ordering;
use std::io::{self, Read, Write};

use crime_dataset::{CrimeRecord, decode_value, encode_value};
use csv::ByteRecord;

use crate::ingest::ValidRow;
use crate::messages;
use crate::sort::{Sortable, read_item_rest, read_item_start};
use crate::validation::to_csv_line;

/// Bytes of an encoded entry before its value: ID, line and value length.
const ENTRY_HEADER_LEN: usize = 8 + 8 + 4;

/// An accepted row on its way to the primary table: the ID, the line where the row starts, and
/// the table value, encoded as soon as the row is read so no typed record is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryEntry {
    pub id: i64,
    pub line: u64,
    pub value: Vec<u8>,
}

impl PrimaryEntry {
    pub fn from_row(row: &ValidRow) -> Self {
        Self {
            id: row.record.id,
            line: row.line,
            value: encode_value(&row.record),
        }
    }

    /// Sort order: by ID, then by line. Lines are unique, so the order is total: the output does
    /// not depend on how rows fell into segments, and the first occurrence of an ID in the file
    /// always comes first.
    pub fn compare(a: &Self, b: &Self) -> Ordering {
        (a.id, a.line).cmp(&(b.id, b.line))
    }
}

/// Segment format: ID and line as little-endian 64-bit integers, then the value length as a
/// little-endian `u32`, then the value.
impl Sortable for PrimaryEntry {
    fn encode<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let length = u32::try_from(self.value.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a primary value longer than 4 GiB cannot be sorted",
            )
        })?;
        writer.write_all(&self.id.to_le_bytes())?;
        writer.write_all(&self.line.to_le_bytes())?;
        writer.write_all(&length.to_le_bytes())?;
        writer.write_all(&self.value)
    }

    /// A segment that ends in the middle of an entry is reported as `InvalidData`.
    fn decode<R: Read>(reader: &mut R) -> io::Result<Option<Self>> {
        let mut header = [0; ENTRY_HEADER_LEN];
        if !read_item_start(reader, &mut header)? {
            return Ok(None);
        }
        let (id, rest) = header.split_at(8);
        let (line, length) = rest.split_at(8);
        let id = i64::from_le_bytes(id.try_into().expect("8 bytes"));
        let line = u64::from_le_bytes(line.try_into().expect("8 bytes"));
        let length = u32::from_le_bytes(length.try_into().expect("4 bytes"));
        let mut value = vec![0; length as usize];
        read_item_rest(reader, &mut value)?;
        Ok(Some(Self { id, line, value }))
    }
}

/// What the duplicate rule made of one entry of the merged stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The first occurrence of its ID: it goes into the table.
    Accepted(PrimaryEntry),
    /// A later occurrence of an ID already accepted from `first_line`: it is logged and skipped.
    Duplicate {
        entry: PrimaryEntry,
        first_line: u64,
    },
}

/// Applies the duplicate rule to a stream sorted by `(id, line)`.
///
/// Repeated IDs are adjacent after the merge, however far apart they were in the file, and the
/// lowest line comes first, so remembering the last accepted ID is enough.
pub struct Dedup<I> {
    entries: I,
    /// ID and line of the last accepted entry.
    last_accepted: Option<(i64, u64)>,
}

impl<I> Dedup<I> {
    pub fn new(entries: I) -> Self {
        Self {
            entries,
            last_accepted: None,
        }
    }
}

impl<I: Iterator<Item = io::Result<PrimaryEntry>>> Iterator for Dedup<I> {
    type Item = io::Result<Step>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = match self.entries.next()? {
            Ok(entry) => entry,
            Err(error) => return Some(Err(error)),
        };
        let step = match self.last_accepted {
            Some((id, first_line)) if id == entry.id => Step::Duplicate { entry, first_line },
            _ => {
                self.last_accepted = Some((entry.id, entry.line));
                Step::Accepted(entry)
            }
        };
        Some(Ok(step))
    }
}

/// Log line of a duplicate: the message followed by the row rebuilt from its stored value.
///
/// The original text is not kept through the sort, so the row is rebuilt from the typed fields.
/// It is equivalent in value to the original; only formatting the source may have, such as
/// trailing zeros in decimals or unneeded quotes, can differ.
pub fn duplicate_log_line(entry: &PrimaryEntry, first_line: u64) -> String {
    let reason = messages::duplicate_id(entry.line, entry.id, first_line);
    match decode_value(&entry.value) {
        Ok(record) => messages::rejected_row(&reason, &csv_line(&record)),
        // Values come from `encode_value`; if one cannot decode, the reason alone still
        // identifies the row.
        Err(_) => reason,
    }
}

/// The record as a CSV row in source column order, `Location` included.
fn csv_line(record: &CrimeRecord) -> String {
    fn optional<T: ToString>(value: Option<T>) -> String {
        value.map(|value| value.to_string()).unwrap_or_default()
    }
    let fields = vec![
        record.id.to_string(),
        record.case_number.clone(),
        record.date.to_string(),
        record.block.clone(),
        record.iucr.clone(),
        record.primary_type.clone(),
        record.description.clone(),
        optional(record.location_description.as_deref()),
        record.arrest.to_string(),
        record.domestic.to_string(),
        record.beat.clone(),
        optional(record.district.as_deref()),
        optional(record.ward),
        optional(record.community_area),
        record.fbi_code.clone(),
        optional(record.x_coordinate),
        optional(record.y_coordinate),
        record.year.to_string(),
        record.updated_on.to_string(),
        optional(record.latitude),
        optional(record.longitude),
        optional(record.location()),
    ];
    to_csv_line(&ByteRecord::from(fields))
}
