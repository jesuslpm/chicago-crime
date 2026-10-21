//! Independent acceptance verifier of a CanopyDB Chicago crime dataset (canopy-db-generador PRD,
//! "Verificador CanopyDB").
//!
//! It reads the CanopyDB base with CanopyDB alone and runs on it the content checks of the
//! original verifier (`crime-acceptance`), shared through its [`Tables`] trait, plus the
//! CanopyDB layout and manifest checks. Its comparison mode compares two datasets of either
//! engine entry by entry, reading an SSTable dataset with the original verifier's LevelDB table
//! reader. It shares no code with the generators.
//!
//! - [`canopy`]: the CanopyDB base, opened for reading only;
//! - [`manifest`]: the CanopyDB manifest and the engine a manifest names;
//! - [`verify`](mod@verify): the CSV mode;
//! - [`compare`](mod@compare): the comparison mode.
//!
//! [`Tables`]: crime_acceptance::dataset::Tables

pub mod canopy;
pub mod compare;
pub mod manifest;
pub mod verify;

pub use compare::compare;
pub use verify::verify;
