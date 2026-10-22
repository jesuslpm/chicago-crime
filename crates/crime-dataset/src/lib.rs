//! Shared contract of the Chicago crime dataset.
//!
//! Both the generator and the query app depend on this crate, so everything that decides how the
//! dataset looks on disk lives here:
//!
//! - the expected CSV [`HEADER`] and the typed [`CrimeRecord`];
//! - [`CivilDateTime`], the naive civil date and time codec (decision D3);
//! - the primary table layout: [`primary_key`], [`encode_value`] and [`decode_value`] (decision D4);
//! - the five secondary indexes, [`IndexKind`], and their [`index_key`] layout (decision D2),
//!   whose text values are stored in the lowercase [`index_text`] form (decision C1), while the
//!   primary table keeps every text field exactly as the CSV has it;
//! - dataset file names and the [`Manifest`] model;
//! - the layout of the output folder: version folders and the [`CURRENT_FILE_NAME`] file that
//!   names the active one, read with [`resolve_current`];
//! - [`canopy`]: the names and the manifest identity of a CanopyDB dataset, and the line that
//!   explains a failure of the engine, shared by the two CanopyDB apps;
//! - [`probe`]: the hidden phase markers the robustness scenarios use to kill an app in a phase.
//!
//! Keys and values use the FoundationDB tuple format only (no FoundationDB database), because its
//! byte order preserves the order of integers, text and `Nil`.

pub mod canopy;
mod datetime;
mod encoding;
mod index;
mod layout;
mod manifest;
pub mod probe;
mod record;

pub use datetime::{CivilDateTime, DateTimeError};
pub use encoding::{DecodeError, VALUE_FIELD_COUNT, decode_value, encode_value, primary_key};
pub use index::{IndexKind, index_key, index_text};
pub use layout::{
    CURRENT_FILE_NAME, CurrentError, VERSION_DIR_PREFIX, is_version_dir_name, resolve_current,
};
pub use manifest::{
    Compression, INDEX_FILE_PREFIX, IndexInfo, MANIFEST_FILE_NAME, Manifest, PRIMARY_FILE_NAME,
    RecordCounts, SCHEMA_VERSION, SourceInfo, TABLE_FILE_EXTENSION, TableInfo,
};
pub use record::{CrimeRecord, HEADER};
