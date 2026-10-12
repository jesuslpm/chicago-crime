//! Shared contract of the Chicago crime dataset.
//!
//! Both the generator and the query app depend on this crate, so everything that decides how the
//! dataset looks on disk lives here:
//!
//! - the expected CSV [`HEADER`] and the typed [`CrimeRecord`];
//! - [`CivilDateTime`], the naive civil date and time codec (decision D3);
//! - the primary table layout: [`primary_key`], [`encode_value`] and [`decode_value`] (decision D4);
//! - the five secondary indexes, [`IndexKind`], and their [`index_key`] layout (decision D2);
//! - dataset file names and the [`Manifest`] model.
//!
//! Keys and values use the FoundationDB tuple format only (no FoundationDB database), because its
//! byte order preserves the order of integers, text and `Nil`.

mod datetime;
mod encoding;
mod index;
mod manifest;
mod record;

pub use datetime::{CivilDateTime, DateTimeError};
pub use encoding::{DecodeError, VALUE_FIELD_COUNT, decode_value, encode_value, primary_key};
pub use index::{IndexKind, index_key};
pub use manifest::{
    Compression, INDEX_FILE_PREFIX, IndexInfo, MANIFEST_FILE_NAME, Manifest, PRIMARY_FILE_NAME,
    RecordCounts, SCHEMA_VERSION, SourceInfo, TABLE_FILE_EXTENSION, TableInfo,
};
pub use record::{CrimeRecord, HEADER};
