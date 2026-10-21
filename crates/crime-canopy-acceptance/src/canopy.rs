//! The CanopyDB base of a version folder, opened for reading only.
//!
//! ```text
//! <version folder>/
//!     manifest.json
//!     canopydb/            one CanopyDB environment
//!         LOCK
//!         crime_<id>_l/    the database `crime`: DATA and OPTIONS
//! ```
//!
//! The database holds six trees named as the manifest's `tables`, with the same keys and values
//! as the SSTables. The base is opened with a 64 MiB page cache (CanopyDB's default of 1 GiB
//! gave a 1,161 MiB peak) and with the database options stored in the base, so that opening it
//! rewrites nothing; when they cannot be read, the base is not opened. It never creates a
//! database or a tree: a missing one is an error.

use std::fs;
use std::path::Path;

use canopydb::{Database, DbOptions, EnvOptions, Environment};
use crime_acceptance::dataset::{PRIMARY, Tables};
use crime_acceptance::indexes::INDEXES;

/// The folder of the CanopyDB base inside the version folder.
pub const BASE_DIR: &str = "canopydb";
/// The one database of the base.
pub const DATABASE: &str = "crime";
/// The trees of the database, in manifest order: the primary table and the indexes of the
/// original verifier.
pub const TABLES: [&str; 6] = [
    PRIMARY,
    INDEXES[0].name,
    INDEXES[1].name,
    INDEXES[2].name,
    INDEXES[3].name,
    INDEXES[4].name,
];
/// Page cache of the reader, as the generator's.
pub const PAGE_CACHE_BYTES: usize = 64 * 1024 * 1024;
/// The file in a database folder that holds its options.
const OPTIONS_FILE: &str = "OPTIONS";

/// The six trees of an open CanopyDB base.
pub struct CanopyTables {
    // Dropped before the environment that holds it.
    database: Database,
    _environment: Environment,
}

impl CanopyTables {
    /// Opens the existing base in the folder `base` for reading. Fails, without creating anything,
    /// when the folder does not exist, when the base cannot be opened (for example while another
    /// process holds it: CanopyDB waits 5 seconds for its lock) or when it has no `crime` database.
    pub fn open(base: &Path) -> Result<Self, String> {
        if !base.is_dir() {
            return Err(format!("{} is not a folder", base.display()));
        }
        let cannot_open = |error: canopydb::Error| {
            format!(
                "the CanopyDB base {} cannot be opened: {error}",
                base.display()
            )
        };
        let mut options = EnvOptions::new(base);
        options.page_cache_size = PAGE_CACHE_BYTES;
        // Read before opening: opening with other options would replace the stored ones.
        let stored = stored_options(base)?;
        let environment = Environment::with_options(options).map_err(cannot_open)?;
        let database = environment
            .get_database_with(DATABASE, stored)
            .map_err(cannot_open)?
            .ok_or_else(|| {
                format!(
                    "the CanopyDB base {} has no database {DATABASE:?}",
                    base.display()
                )
            })?;
        Ok(Self {
            database,
            _environment: environment,
        })
    }
}

/// The options stored with the `crime` database, which CanopyDB replaces with the opener's: passing
/// them back leaves the base as it was. An error, and the base stays closed, when they cannot be
/// found or read.
fn stored_options(base: &Path) -> Result<DbOptions, String> {
    let unreadable = |reason: String| {
        format!(
            "the {OPTIONS_FILE} of the database {DATABASE:?} in {} cannot be read: {reason}",
            base.display()
        )
    };
    let entries = fs::read_dir(base).map_err(|error| unreadable(error.to_string()))?;
    let database = entries
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy())
                .is_some_and(|name| {
                    name.starts_with(&format!("{DATABASE}_")) && name.ends_with("_l")
                })
        })
        .ok_or_else(|| unreadable("there is no database folder".to_owned()))?;
    let bytes =
        fs::read(database.join(OPTIONS_FILE)).map_err(|error| unreadable(error.to_string()))?;
    serde_json::from_slice::<DbOptions>(&bytes).map_err(|error| unreadable(error.to_string()))
}

impl Tables for CanopyTables {
    /// The tree name.
    fn label(&self, table: &str) -> String {
        table.to_owned()
    }

    fn scan(&self, table: &str, visit: &mut dyn FnMut(&[u8], &[u8])) -> Result<(), String> {
        let unreadable = |error: canopydb::Error| format!("tree {table} cannot be read: {error}");
        let transaction = self.database.begin_read().map_err(unreadable)?;
        let tree = transaction
            .get_tree(table.as_bytes())
            .map_err(unreadable)?
            .ok_or_else(|| format!("the CanopyDB base has no tree {table:?}"))?;
        for entry in tree.iter().map_err(unreadable)? {
            let (key, value) = entry.map_err(unreadable)?;
            visit(&key, &value);
        }
        Ok(())
    }
}
