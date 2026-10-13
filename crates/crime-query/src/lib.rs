//! Query app of the Chicago crime dataset.
//!
//! The query app reads a dataset published by the generator and never writes in its folder.
//! Every stage lives in this library so it can be tested:
//!
//! - [`Dataset`]: the published dataset opened as a snapshot (AC-C12), with the schema check,
//!   record reads by ID and the reads of the secondary indexes the query engine combines
//!   (decision C10): ID streams per value ([`IdStream`], searched with an [`IndexValue`]),
//!   date ranges, and point lookups of one `(value, id)` entry;
//! - `tracked`: the table file wrapper that turns every failed read into an error instead of
//!   the silently shorter result the table reader would give;
//! - [`Field`]: the catalog of the CSV columns, with their types, optionality and indexes;
//! - [`Query`]: the conditions of the command line checked, typed and grouped as the engine
//!   resolves them (decisions C1, C2, C3 and C10), with the filters for the fields without
//!   index and the interpreted form the JSON output shows (decision C9);
//! - [`cli`]: the command line in Spanish, and [`exit_code`];
//! - [`messages`]: every user-facing text.
//!
//! Nothing here holds IDs or records in memory in proportion to a result (decision C7): streams
//! read one index entry at a time and lookups read one entry.

pub mod cli;
mod dataset;
mod fields;
mod index;
pub mod messages;
mod query;
mod tracked;

pub use dataset::{Dataset, OpenError, ReadError};
pub use fields::{Field, FieldType};
pub use index::{IdStream, IndexValue};
pub use query::{
    Applied, AppliedCondition, AppliedInterval, AppliedValue, DateInterval, Equality, Filter,
    Interpreted, Query, QueryError, RawCondition,
};

/// Exit codes of the binary (decision C6), the same as the generator's.
pub mod exit_code {
    /// The query ran, even with no results.
    pub const COMPLETED: u8 = 0;
    /// The query was rejected for the dataset or for its conditions, or it aborted.
    pub const REJECTED: u8 = 1;
    /// The command line is not valid; nothing was queried.
    pub const USAGE: u8 = 2;
    /// The analyst cancelled the query with Ctrl+C. 130 is the code shells give a program
    /// interrupted by Ctrl+C (128 + `SIGINT`).
    pub const CANCELLED: u8 = 130;
}
