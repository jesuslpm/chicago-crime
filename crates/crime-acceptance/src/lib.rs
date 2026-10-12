//! Independent acceptance verifier of a generated Chicago crime dataset (child PRD, "Verificación
//! sin consultor").
//!
//! It reads the published tables directly with the LevelDB table reader (`sstable`), decodes keys
//! and values with the FoundationDB tuple format (`foundationdb-tuple`) and compares them with the
//! source CSV (`csv`). It shares no code with the generator or the dataset contract crate: the
//! layout, the field rules and the index definitions are written here again from the PRDs, so a
//! defect in the generator's contract cannot hide itself.
//!
//! - [`civil`]: the published date text and the spring-forward hour;
//! - [`fields`]: the CSV columns, the field rules and the primary value layout;
//! - [`indexes`]: the five index definitions and their keys;
//! - [`dataset`]: `CURRENT`, the version folder, the manifest and the tables on disk;
//! - [`verify`]: every check of AC-G1, AC-G3 and AC-G4 plus the manifest;
//! - [`report`]: one PASS or FAIL line per check.

pub mod civil;
pub mod dataset;
pub mod fields;
pub mod indexes;
pub mod report;
pub mod verify;

pub use report::{Check, Report};
pub use verify::{Options, verify};
