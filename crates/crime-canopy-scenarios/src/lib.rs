//! The robustness scenarios of the CanopyDB apps (canopy-db-robustez PRD): a catalog of
//! scenarios R1–R13 that kill the real binaries, press Ctrl+C, run several of them at once and
//! generate a big CSV, and check what the contract says must happen.
//!
//! - [`process`]: start an app with its phase markers, kill it, press Ctrl+C, wait for it;
//! - [`context`]: the binaries, the reference dataset and its copies, clean runs, comparisons;
//! - [`scenarios`]: the catalog;
//! - [`report`]: the state of each variant, `NOT MET` associations, the summary and the rows of
//!   the scenario log;
//! - [`findings`]: the identifiers of the findings log.

pub mod context;
pub mod findings;
pub mod process;
pub mod report;
pub mod scenarios;
