//! Searched index values and the streams of IDs read from an index table.
//!
//! Every index table holds the keys `pack((value, id))` with an empty value (decision D2), so
//! the entries of one value are next to each other, sorted by ascending ID, and a date range is
//! one run of keys sorted by date and then ID. A stream seeks to the first key of its value or
//! range and decodes key after key until the first one outside it; it holds one table iterator
//! and never collects IDs, so its memory does not grow with the result (decision C7).

use std::path::PathBuf;
use std::sync::Arc;

use crime_dataset::index_text;
use foundationdb_tuple::{Element, pack, pack_into, unpack};
use sstable::{SSIterator, Table, TableIterator};

use crate::ReadError;
use crate::tracked::ReadHealth;

/// A key greater than every key of every table: packed tuples start with a type code below
/// `0xff`, and so does the separator the table keeps after its last block.
const PAST_EVERY_KEY: &[u8] = &[0xff];

/// A value searched in an index, in the form the index stores it.
///
/// Text is always converted with [`index_text`] when the value is built (decision C1), so
/// `theft`, `Theft` and `THEFT` search the same entries; codes keep their leading zeros. The
/// applied form is what the query reports as interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexValue(Value);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    Nil,
    Text(String),
    Integer(i64),
}

impl IndexValue {
    /// "Sin valor": the records whose optional field (district, community area) is empty.
    pub fn nil() -> Self {
        Self(Value::Nil)
    }

    /// A text value (primary type, district, case number), lowercased with [`index_text`].
    pub fn text(raw: &str) -> Self {
        Self(Value::Text(index_text(raw)))
    }

    /// An integer value: a community area, or the seconds of a date.
    pub fn integer(value: i64) -> Self {
        Self(Value::Integer(value))
    }

    pub fn is_nil(&self) -> bool {
        self.0 == Value::Nil
    }

    /// The applied (lowercase) text, for a text value.
    pub fn as_text(&self) -> Option<&str> {
        match &self.0 {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self.0 {
            Value::Integer(value) => Some(value),
            _ => None,
        }
    }

    /// The packed 1-tuple `(value,)`: a prefix of every key of this value, and smaller than all
    /// of them.
    pub(crate) fn prefix(&self) -> Vec<u8> {
        match &self.0 {
            Value::Nil => pack(&(None::<i64>,)),
            Value::Text(text) => pack(&(text.as_str(),)),
            Value::Integer(value) => pack(&(*value,)),
        }
    }

    /// The index key of this value for `id`: the packed tuple `(value, id)`, which packs as the
    /// prefix followed by the ID.
    pub(crate) fn key(&self, id: i64) -> Vec<u8> {
        let mut key = self.prefix();
        pack_into(&(id,), &mut key);
        key
    }

    /// Whether a decoded key value is this value.
    fn matches(&self, element: &Element<'_>) -> bool {
        match (&self.0, element) {
            (Value::Nil, Element::Nil) => true,
            (Value::Text(text), Element::String(found)) => text == found,
            (Value::Integer(value), Element::Int(found)) => value == found,
            _ => false,
        }
    }
}

/// Where a stream stops.
#[derive(Debug)]
enum Bound {
    /// At the first key with another value.
    Value(IndexValue),
    /// At the first key whose date is later than these seconds.
    DatesUpTo(i64),
}

/// The IDs of one index value, in ascending ID order, or of one date range, in date and then ID
/// order, read lazily from the index table.
///
/// A read failure anywhere during the stream ends it with one [`ReadError`], never with fewer
/// IDs; nothing follows an error.
pub struct IdStream {
    table: Table,
    entries: TableIterator,
    /// The key to seek to, until the first item is read.
    start: Option<Vec<u8>>,
    bound: Bound,
    health: Arc<ReadHealth>,
    folder: PathBuf,
    finished: bool,
}

impl IdStream {
    pub(crate) fn for_value(
        table: &Table,
        value: &IndexValue,
        health: Arc<ReadHealth>,
        folder: PathBuf,
    ) -> Self {
        Self::new(
            table,
            value.prefix(),
            Bound::Value(value.clone()),
            health,
            folder,
        )
    }

    /// Dates within `[from, to]`, both as seconds of the civil clock.
    pub(crate) fn for_dates(
        table: &Table,
        from: i64,
        to: i64,
        health: Arc<ReadHealth>,
        folder: PathBuf,
    ) -> Self {
        Self::new(table, pack(&(from,)), Bound::DatesUpTo(to), health, folder)
    }

    fn new(
        table: &Table,
        start: Vec<u8>,
        bound: Bound,
        health: Arc<ReadHealth>,
        folder: PathBuf,
    ) -> Self {
        Self {
            table: table.clone(),
            entries: table.iter(),
            start: Some(start),
            bound,
            health,
            folder,
            finished: false,
        }
    }

    /// Moves to the first key not smaller than `start`; `false` when there is none.
    ///
    /// `TableIterator::seek` also leaves the iterator invalid when `start` falls between the
    /// last key of a block and the separator key the table keeps for that block, which is
    /// greater than that last key but smaller than the next block's first key. Then the next
    /// block holds the wanted key: the iterator still points at the exhausted block, so
    /// advancing reads that block again from its start (all its keys are smaller than `start`)
    /// and then the next one. A seek past the last key leaves it reset instead, where
    /// advancing would restart from the first key of the table, so that case is told apart
    /// first through the block the table index names for `start`.
    fn seek(&mut self, start: &[u8]) -> bool {
        self.entries.seek(start);
        if self.entries.valid() {
            return true;
        }
        if self.health.failure().is_some()
            || self.table.approx_offset_of(start) == self.table.approx_offset_of(PAST_EVERY_KEY)
        {
            return false;
        }
        while self.entries.advance() {
            if self.entries.current_key().is_some_and(|key| key >= start) {
                return true;
            }
        }
        false
    }

    /// The ID of the current key, `None` when the key is past the bound.
    fn current(&self) -> Option<Result<i64, ReadError>> {
        let key = self.entries.current_key()?;
        let Some((value, id)) = decode_key(key) else {
            return Some(Err(self.error(format!("malformed index key {key:02x?}"))));
        };
        let within = match &self.bound {
            Bound::Value(wanted) => wanted.matches(&value),
            Bound::DatesUpTo(last) => match value {
                Element::Int(seconds) => seconds <= *last,
                _ => return Some(Err(self.error(format!("malformed date key {key:02x?}")))),
            },
        };
        within.then_some(Ok(id))
    }

    fn error(&self, cause: String) -> ReadError {
        ReadError::new(self.folder.clone(), cause)
    }
}

impl Iterator for IdStream {
    type Item = Result<i64, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let positioned = match self.start.take() {
            Some(start) => self.seek(&start),
            None => self.entries.advance(),
        };
        let mut item = if positioned { self.current() } else { None };
        // The iterator may have skipped an unreadable block to get here, or stopped at one.
        if let Some(cause) = self.health.failure() {
            item = Some(Err(self.error(cause)));
        }
        if !matches!(item, Some(Ok(_))) {
            self.finished = true;
        }
        item
    }
}

/// The indexed value and the ID of an index key, or `None` unless the key is a tuple of exactly
/// two elements whose last one is an integer.
fn decode_key(key: &[u8]) -> Option<(Element<'_>, i64)> {
    let mut elements: Vec<Element<'_>> = unpack(key).ok()?;
    let Some(Element::Int(id)) = elements.pop() else {
        return None;
    };
    let value = elements.pop()?;
    elements.is_empty().then_some((value, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crime_dataset::{CrimeRecord, IndexKind, index_key};

    fn record(district: Option<&str>, community_area: Option<i64>) -> CrimeRecord {
        let date = "01/02/2020 03:04:05 PM".parse().expect("a valid date");
        CrimeRecord {
            id: 1234,
            case_number: "JA100".to_owned(),
            date,
            block: "064XX S ELLIS AVE".to_owned(),
            iucr: "0314".to_owned(),
            primary_type: "Theft".to_owned(),
            description: "OVER $500".to_owned(),
            location_description: None,
            arrest: false,
            domestic: true,
            beat: "0314".to_owned(),
            district: district.map(str::to_owned),
            ward: None,
            community_area,
            fbi_code: "06".to_owned(),
            x_coordinate: None,
            y_coordinate: None,
            year: 2020,
            updated_on: date,
            latitude: None,
            longitude: None,
        }
    }

    #[test]
    fn keys_are_the_keys_the_dataset_contract_writes() {
        let full = record(Some("008"), Some(42));
        let empty = record(None, None);
        let cases = [
            (IndexKind::PrimaryType, IndexValue::text("THEFT"), &full),
            (IndexKind::CaseNumber, IndexValue::text("ja100"), &full),
            (IndexKind::District, IndexValue::text("008"), &full),
            (IndexKind::District, IndexValue::nil(), &empty),
            (IndexKind::CommunityArea, IndexValue::integer(42), &full),
            (IndexKind::CommunityArea, IndexValue::nil(), &empty),
            (
                IndexKind::Date,
                IndexValue::integer(full.date.seconds()),
                &full,
            ),
        ];
        for (kind, value, record) in cases {
            let key = index_key(kind, record);
            assert_eq!(value.key(record.id), key, "{kind:?} {value:?}");
            assert!(key.starts_with(&value.prefix()), "{kind:?} {value:?}");
            let (element, id) = decode_key(&key).expect("a (value, id) key");
            assert!(value.matches(&element), "{kind:?} {value:?}");
            assert_eq!(id, record.id);
        }
    }

    #[test]
    fn a_text_value_is_held_in_its_index_form() {
        assert_eq!(IndexValue::text("JA100"), IndexValue::text("ja100"));
        assert_eq!(IndexValue::text("Ñandú ÉL").as_text(), Some("ñandú él"));
        assert_eq!(IndexValue::text("008").as_text(), Some("008"));
        assert_eq!(IndexValue::text("x").as_integer(), None);
        assert!(!IndexValue::integer(0).is_nil());
    }

    #[test]
    fn values_of_other_types_never_match() {
        assert!(!IndexValue::nil().matches(&Element::Int(0)));
        assert!(!IndexValue::integer(8).matches(&Element::String("8".into())));
        assert!(!IndexValue::text("8").matches(&Element::String("008".into())));
    }
}
