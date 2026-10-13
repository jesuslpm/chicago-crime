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
//! - [`messages`]: every user-facing text.
//!
//! Nothing here holds IDs or records in memory in proportion to a result (decision C7): streams
//! read one index entry at a time and lookups read one entry.

mod dataset;
mod index;
pub mod messages;
mod tracked;

pub use dataset::{Dataset, OpenError, ReadError};
pub use index::{IdStream, IndexValue};
