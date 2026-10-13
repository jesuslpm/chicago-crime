//! Every user-facing text of the query app, in Spanish exactly as the PRDs specify.
//!
//! Keeping them together lets later stages reuse the same wording and keeps the tests honest.
//! A folder is shown exactly as the analyst gave it.

use std::path::Path;

use crime_dataset::SCHEMA_VERSION;

// Opening and reading the dataset (child PRD, flow step 1 and "Aborta sin salida parcial
// engañosa").

pub fn dataset_not_found(folder: &Path) -> String {
    format!(
        "No se encontró un conjunto de datos válido en {}.",
        folder.display()
    )
}

/// `found` is the schema version the manifest declares; the query app reads [`SCHEMA_VERSION`].
pub fn incompatible_schema(found: u32) -> String {
    format!(
        "El conjunto usa el esquema v{found} y este consultor admite v{SCHEMA_VERSION}. Regenera \
         el conjunto."
    )
}

pub fn dataset_unreadable(folder: &Path) -> String {
    format!(
        "No se pudo leer el conjunto de datos en {}.",
        folder.display()
    )
}
