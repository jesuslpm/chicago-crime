//! Query app of the Chicago crime dataset stored in a CanopyDB base (child PRD
//! `canopy-db-consultor`).
//!
//! It is the original query app (decision K1 of the parent PRD, KC9 of the child PRD): the same
//! options, queries, outputs, batch, messages and exit codes, through [`crime_query::main`].
//! Only the dataset differs: [`APP`] opens the base that the CanopyDB generator publishes and
//! reads its six trees as the [`crime_query::Tables`] of a [`crime_query::TablesSource`]:
//!
//! - `manifest`: the CanopyDB manifest it accepts (decisions KC3 and KC4);
//! - `open`: the order of the checks, the wait while another process has the base open
//!   (decision KC2) and the opening that creates nothing (decision K6);
//! - `tables`: the reads, with every error or panic of the engine reported with its detail line
//!   (decision KC5);
//! - `messages`: the three messages only this app has.
//!
//! Its JSON output shows `schema_version` as the manifest's text, `"canopydb-2"` (decision KC6).

mod manifest;
pub mod messages;
mod open;
mod tables;

pub use open::{PAGE_CACHE_BYTES, WAIT, open, open_resolving, open_waiting};

/// The CanopyDB query app (decision KC1): the original's help with its own name, description and
/// a note on the wait.
pub const APP: crime_query::App = crime_query::App {
    name: "crime-canopy-query",
    about: "Consulta el conjunto de datos de delitos de Chicago (base CanopyDB) publicado por el \
            generador CanopyDB.",
    help_note: "Si otro proceso tiene abierto el conjunto de datos, la consulta espera hasta 30 s \
                antes de\nrechazarse con 1.",
    open,
};
