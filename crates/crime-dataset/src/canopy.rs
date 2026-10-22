//! The contract of a CanopyDB dataset, shared by the CanopyDB generator and the CanopyDB query
//! app (canopy-db PRD and its children `canopy-db-generador` and `canopy-db-consultor`):
//!
//! ```text
//! <version folder>/
//!     manifest.json        the CanopyDB manifest (decision KG3), schema SCHEMA_VERSION
//!     canopydb/            BASE_DIR_NAME: one CanopyDB environment
//!         LOCK
//!         crime_<id>_l/    the database DATABASE_NAME: DATA and OPTIONS
//! ```
//!
//! The database holds six trees: [`PRIMARY_TABLE_NAME`] and one per index, named
//! [`crate::IndexKind::name`], with the same keys and values as the LevelDB tables (decision K2).
//! The CanopyDB verifier keeps its own copy of these names on purpose: it must not share code
//! with the apps it checks.

/// Folder of the CanopyDB base inside the version folder (decision KG2).
pub const BASE_DIR_NAME: &str = "canopydb";

/// The one database of the base. Its trees are named as the tables of the manifest.
pub const DATABASE_NAME: &str = "crime";

/// Name of the primary table, as a tree and in the manifest.
pub const PRIMARY_TABLE_NAME: &str = "primary";

/// `schema_version` of the CanopyDB manifest (decision KG3): the engine and the version of the
/// logical schema, which is the one of the LevelDB tables (decision K2).
pub const SCHEMA_VERSION: &str = "canopydb-2";

/// `engine.name` of the CanopyDB manifest.
pub const ENGINE_NAME: &str = "canopydb";

/// The version of CanopyDB the CanopyDB apps are built with, as `Cargo.lock` names it (a test of
/// the generator checks they agree). The generator writes it in the manifest and the query app
/// only opens a base that names it (decision KC4).
pub const CANOPYDB_VERSION: &str = "0.2.5";

/// The line that explains a failure of the engine (decisions KG7 and KC5): the error as CanopyDB
/// gives it, on one line.
pub fn engine_detail(error: &str) -> String {
    let one_line: Vec<&str> = error
        .split(['\r', '\n'])
        .filter(|part| !part.is_empty())
        .collect();
    format!("Detalle del error de CanopyDB: {}", one_line.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_detail_is_one_line_with_the_engine_text() {
        assert_eq!(
            engine_detail("io error\r\nat page 7\n"),
            "Detalle del error de CanopyDB: io error at page 7"
        );
        assert_eq!(engine_detail(""), "Detalle del error de CanopyDB: ");
    }
}
