//! What a query reads, whatever storage engine holds the tables.
//!
//! The query engine ([`crate::Execution`]), the output and the runs read an open dataset only
//! through [`Source`]. The original query app's [`Dataset`] reads LevelDB tables; another engine
//! implements the few primitive reads of [`Tables`] (a primary value, an index key, a cursor over
//! the keys of an index) and gets the rest from [`TablesSource`]: the keys are the same packed
//! tuples in every engine (decision K2 of the canopy-db PRD).

use std::path::{Path, PathBuf};

use crime_dataset::{CivilDateTime, CrimeRecord, IndexKind, decode_value, primary_key};
use serde_json::Value;

use crate::index::{IdStream, IndexValue, KeyCursor};
use crate::{Dataset, ReadError};

/// One published version of the dataset, open as a snapshot, as the query engine reads it.
pub trait Source {
    /// The output folder exactly as the analyst gave it.
    fn folder(&self) -> &Path;
    /// Name of the open version folder.
    fn version(&self) -> &str;
    /// `generated_at` of the manifest, as the JSON output shows it (decision C9).
    fn generated_at(&self) -> &str;
    /// `schema_version` of the manifest, as the JSON output shows it (decision C9).
    fn schema_version(&self) -> Value;
    /// The record with this ID, every field exactly as the CSV has it.
    fn record(&self, id: i64) -> Result<Option<CrimeRecord>, ReadError>;
    /// Whether a record with this ID exists.
    fn id_exists(&self, id: i64) -> Result<bool, ReadError>;
    /// Every ID whose `kind` index holds exactly `value`, in ascending ID order.
    fn ids_with_value(&self, kind: IndexKind, value: &IndexValue) -> IdStream;
    /// Every ID whose `kind` index holds a text value that starts with the lowercase `prefix`,
    /// in value and then ID order.
    fn ids_with_prefix(&self, kind: IndexKind, prefix: &str) -> IdStream;
    /// Every ID whose date is within `[from, to]`, in date and then ID order.
    fn ids_in_date_range(&self, from: CivilDateTime, to: CivilDateTime) -> IdStream;
    /// Whether the record `id` has `value` in the `kind` index.
    fn has_entry(&self, kind: IndexKind, value: &IndexValue, id: i64) -> Result<bool, ReadError>;

    /// Whether any record has `value` in the `kind` index.
    fn has_value(&self, kind: IndexKind, value: &IndexValue) -> Result<bool, ReadError> {
        let first = self.ids_with_value(kind, value).next().transpose()?;
        Ok(first.is_some())
    }
}

impl Source for Dataset {
    fn folder(&self) -> &Path {
        Dataset::folder(self)
    }

    fn version(&self) -> &str {
        Dataset::version(self)
    }

    fn generated_at(&self) -> &str {
        &self.manifest().generated_at
    }

    fn schema_version(&self) -> Value {
        Value::from(self.manifest().schema_version)
    }

    fn record(&self, id: i64) -> Result<Option<CrimeRecord>, ReadError> {
        Dataset::record(self, id)
    }

    fn id_exists(&self, id: i64) -> Result<bool, ReadError> {
        Dataset::id_exists(self, id)
    }

    fn ids_with_value(&self, kind: IndexKind, value: &IndexValue) -> IdStream {
        Dataset::ids_with_value(self, kind, value)
    }

    fn ids_with_prefix(&self, kind: IndexKind, prefix: &str) -> IdStream {
        Dataset::ids_with_prefix(self, kind, prefix)
    }

    fn ids_in_date_range(&self, from: CivilDateTime, to: CivilDateTime) -> IdStream {
        Dataset::ids_in_date_range(self, from, to)
    }

    fn has_entry(&self, kind: IndexKind, value: &IndexValue, id: i64) -> Result<bool, ReadError> {
        Dataset::has_entry(self, kind, value, id)
    }

    fn has_value(&self, kind: IndexKind, value: &IndexValue) -> Result<bool, ReadError> {
        Dataset::has_value(self, kind, value)
    }
}

/// The primitive reads of the six tables of a dataset in some storage engine.
pub trait Tables {
    /// The value of the primary table for `key`, if any.
    fn primary_value(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ReadError>;
    /// Whether the `kind` index holds `key`.
    fn index_contains(&self, kind: IndexKind, key: &[u8]) -> Result<bool, ReadError>;
    /// A new cursor over the keys of the `kind` index.
    fn index_cursor(&self, kind: IndexKind) -> Box<dyn KeyCursor>;
}

/// A [`Source`] over the [`Tables`] of another storage engine, with the identity of the open
/// version that its manifest gives.
pub struct TablesSource<T> {
    folder: PathBuf,
    version: String,
    generated_at: String,
    schema_version: Value,
    tables: T,
}

impl<T: Tables> TablesSource<T> {
    /// `folder` is the output folder as the analyst gave it; `version`, the name of the open
    /// version folder; `generated_at` and `schema_version`, those of its manifest.
    pub fn new(
        folder: PathBuf,
        version: String,
        generated_at: String,
        schema_version: Value,
        tables: T,
    ) -> Self {
        Self {
            folder,
            version,
            generated_at,
            schema_version,
            tables,
        }
    }

    fn error(&self, cause: String) -> ReadError {
        ReadError::new(self.folder.clone(), cause)
    }
}

impl<T: Tables> Source for TablesSource<T> {
    fn folder(&self) -> &Path {
        &self.folder
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn generated_at(&self) -> &str {
        &self.generated_at
    }

    fn schema_version(&self) -> Value {
        self.schema_version.clone()
    }

    fn record(&self, id: i64) -> Result<Option<CrimeRecord>, ReadError> {
        self.tables
            .primary_value(&primary_key(id))?
            .map(|bytes| {
                decode_value(&bytes)
                    .map_err(|error| self.error(format!("record {id} cannot be decoded: {error}")))
            })
            .transpose()
    }

    fn id_exists(&self, id: i64) -> Result<bool, ReadError> {
        Ok(self.tables.primary_value(&primary_key(id))?.is_some())
    }

    fn ids_with_value(&self, kind: IndexKind, value: &IndexValue) -> IdStream {
        IdStream::for_value(self.tables.index_cursor(kind), value, self.folder.clone())
    }

    fn ids_with_prefix(&self, kind: IndexKind, prefix: &str) -> IdStream {
        IdStream::for_prefix(self.tables.index_cursor(kind), prefix, self.folder.clone())
    }

    fn ids_in_date_range(&self, from: CivilDateTime, to: CivilDateTime) -> IdStream {
        IdStream::for_dates(
            self.tables.index_cursor(IndexKind::Date),
            from.seconds(),
            to.seconds(),
            self.folder.clone(),
        )
    }

    fn has_entry(&self, kind: IndexKind, value: &IndexValue, id: i64) -> Result<bool, ReadError> {
        self.tables.index_contains(kind, &value.key(id))
    }
}
