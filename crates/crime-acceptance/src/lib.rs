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
//! - [`dataset`]: `CURRENT`, the version folder, the manifest and the tables on disk, read through
//!   the [`dataset::Tables`] trait that another storage engine can implement;
//! - [`verify`]: every check of AC-G1, AC-G3 and AC-G4 plus the manifest; the content checks
//!   ([`verify_content`]) are shared with the CanopyDB verifier (`crime-canopy-acceptance`);
//! - [`report`]: one PASS, FAIL or SKIP line per check;
//! - [`select`]: the independent filtering of the CSV that the query app's acceptance compares
//!   against (counts, ID lists and source rows).

pub mod civil;
pub mod dataset;
pub mod fields;
pub mod indexes;
pub mod report;
pub mod select;
pub mod verify;

pub use report::{Check, Outcome, Report};
pub use verify::{ContentFacts, Options, TableCounts, verify, verify_content};
