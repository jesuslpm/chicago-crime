//! The published dataset on disk, read without the dataset contract crate.
//!
//! ```text
//! <output folder>/
//!     CURRENT              name of the active version folder and a line feed
//!     <version folder>/
//!         primary.sst      key pack((id,)), value: the 21 stored fields
//!         index-<name>.sst one per index, key pack((value, id)), empty value
//!         manifest.json
//! ```
//!
//! Folders that `CURRENT` does not name are never read: they are not part of the dataset.
//!
//! The checks read the six tables through [`Tables`]; [`SstableTables`] reads them from the
//! `.sst` files, and the CanopyDB verifier implements it over a CanopyDB base.

use std::fs;
use std::path::{Path, PathBuf};

use foundationdb_tuple::{pack, unpack};
use serde_json::Value;
use sstable::{Options, SSIterator, Status, Table};

use crate::indexes::INDEXES;

/// File in the output folder that names the active version folder.
pub const CURRENT_FILE: &str = "CURRENT";
/// The manifest inside the version folder.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The primary table inside the version folder.
pub const PRIMARY_FILE: &str = "primary.sst";

/// The version folder that `CURRENT` names in `output`.
///
/// `CURRENT` must hold one plain folder name followed by a line feed (`\n` or `\r\n`), and that
/// folder must exist.
pub fn resolve_current(output: &Path) -> Result<PathBuf, String> {
    let path = output.join(CURRENT_FILE);
    let bytes =
        fs::read(&path).map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{CURRENT_FILE} is not UTF-8"))?;
    let name = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .ok_or_else(|| format!("{CURRENT_FILE} does not end with a line feed: {text:?}"))?;
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\r', '\n']);
    if !plain {
        return Err(format!(
            "{CURRENT_FILE} does not hold one folder name: {text:?}"
        ));
    }
    let dir = output.join(name);
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(format!(
            "{CURRENT_FILE} names {name}, which is not a folder"
        ))
    }
}

/// The files a version folder must hold, and nothing else, sorted by name.
pub fn expected_files() -> Vec<&'static str> {
    let mut files: Vec<&str> = INDEXES.iter().map(|spec| spec.file).collect();
    files.extend([MANIFEST_FILE, PRIMARY_FILE]);
    files.sort_unstable();
    files
}

/// Names of everything inside `dir`, sorted; folders end with `/`.
pub fn folder_contents(dir: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|error| format!("{} cannot be listed: {error}", dir.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("{} cannot be listed: {error}", dir.display()))?;
        let mut name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            name.push('/');
        }
        names.push(name);
    }
    names.sort();
    Ok(names)
}

/// Size of a file on disk.
pub fn file_size(path: &Path) -> Result<u64, String> {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))
}

/// A table as the manifest describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFacts {
    pub file: String,
    pub entries: u64,
    pub size_bytes: u64,
}

/// What the manifest says, read field by field from the JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestFacts {
    pub schema_version: u64,
    pub generated_at: String,
    pub source_file_name: String,
    pub source_size_bytes: u64,
    pub source_sha256: String,
    pub valid: u64,
    pub rejected: u64,
    pub compression: String,
    pub primary: TableFacts,
    /// `(name, table)`, in manifest order.
    pub indexes: Vec<(String, TableFacts)>,
}

/// Reads `manifest.json` as plain JSON.
pub fn read_manifest(path: &Path) -> Result<ManifestFacts, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|error| format!("{MANIFEST_FILE}: {error}"))?;
    let indexes = json
        .get("indexes")
        .and_then(Value::as_array)
        .ok_or("the manifest has no \"indexes\" list")?
        .iter()
        .map(|index| Ok((text_at(index, &["name"])?, table_at(index, &[])?)))
        .collect::<Result<_, String>>()?;
    Ok(ManifestFacts {
        schema_version: number_at(&json, &["schema_version"])?,
        generated_at: text_at(&json, &["generated_at"])?,
        source_file_name: text_at(&json, &["source", "file_name"])?,
        source_size_bytes: number_at(&json, &["source", "size_bytes"])?,
        source_sha256: text_at(&json, &["source", "sha256"])?,
        valid: number_at(&json, &["records", "valid"])?,
        rejected: number_at(&json, &["records", "rejected"])?,
        compression: text_at(&json, &["compression"])?,
        primary: table_at(&json, &["primary"])?,
        indexes,
    })
}

/// The JSON value at `path` (object keys from the root), or an error that names the path.
pub fn value_at<'a>(json: &'a Value, path: &[&str]) -> Result<&'a Value, String> {
    path.iter()
        .try_fold(json, |value, key| value.get(key))
        .ok_or_else(|| format!("the manifest has no \"{}\"", path.join(".")))
}

/// The whole number at `path`.
pub fn number_at(json: &Value, path: &[&str]) -> Result<u64, String> {
    value_at(json, path)?
        .as_u64()
        .ok_or_else(|| format!("\"{}\" is not a whole number", path.join(".")))
}

/// The text at `path`.
pub fn text_at(json: &Value, path: &[&str]) -> Result<String, String> {
    value_at(json, path)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("\"{}\" is not text", path.join(".")))
}

fn table_at(json: &Value, path: &[&str]) -> Result<TableFacts, String> {
    let table = value_at(json, path)?;
    Ok(TableFacts {
        file: text_at(table, &["file"])?,
        entries: number_at(table, &["entries"])?,
        size_bytes: number_at(table, &["size_bytes"])?,
    })
}

/// A table failure as text. `Status` is described by hand: its `Display` never returns in
/// `sstable` 0.11.
fn status_text(path: &Path, status: &Status) -> String {
    format!(
        "{} cannot be read: {:?} {}",
        path.display(),
        status.code,
        status.err
    )
}

/// One key and its value.
pub type Entry = (Vec<u8>, Vec<u8>);

/// The name of the primary table; the index tables are named as [`INDEXES`] names them.
pub const PRIMARY: &str = "primary";

/// The six tables of a dataset, wherever they are stored: the content checks read them through
/// this trait alone, so the same checks serve every storage engine.
pub trait Tables: Sync {
    /// How the report names `table` (the primary table or an index name).
    fn label(&self, table: &str) -> String;

    /// Calls `visit` with every key and value of `table`, in key order.
    fn scan(&self, table: &str, visit: &mut dyn FnMut(&[u8], &[u8])) -> Result<(), String>;
}

/// The tables of an SSTable version folder: `primary.sst` and one `index-<name>.sst` per index.
#[derive(Debug, Clone)]
pub struct SstableTables {
    version: PathBuf,
}

impl SstableTables {
    pub fn new(version: &Path) -> Self {
        Self {
            version: version.to_path_buf(),
        }
    }
}

impl Tables for SstableTables {
    /// The file name of the table.
    fn label(&self, table: &str) -> String {
        if table == PRIMARY {
            return PRIMARY_FILE.to_owned();
        }
        INDEXES
            .iter()
            .find(|spec| spec.name == table)
            .map_or_else(|| format!("{table}.sst"), |spec| spec.file.to_owned())
    }

    fn scan(&self, table: &str, visit: &mut dyn FnMut(&[u8], &[u8])) -> Result<(), String> {
        let path = self.version.join(self.label(table));
        let table = Table::new_from_file(Options::default(), &path)
            .map_err(|status| status_text(&path, &status))?;
        let mut iterator = table.iter();
        while let Some((key, value)) = SSIterator::next(&mut iterator) {
            visit(&key, &value);
        }
        Ok(())
    }
}

/// Every key and value of `table`, in key order.
pub fn read_entries(tables: &dyn Tables, table: &str) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    tables.scan(table, &mut |key, value| {
        entries.push((key.to_vec(), value.to_vec()));
    })?;
    Ok(entries)
}

/// The primary table loaded in memory: IDs in file order with their values side by side.
#[derive(Debug, Default)]
pub struct PrimaryTable {
    ids: Vec<i64>,
    /// End of each value in `values`.
    ends: Vec<usize>,
    values: Vec<u8>,
    /// The first key that is not a packed `(id,)` or not greater than the previous one.
    pub order_violation: Option<String>,
}

impl PrimaryTable {
    /// Reads every entry of the primary table of `tables`, checking that each key is exactly
    /// `pack((id,))` and that keys and IDs are strictly increasing.
    pub fn read(tables: &dyn Tables) -> Result<Self, String> {
        let mut primary = Self::default();
        let mut previous_key: Option<Vec<u8>> = None;
        tables.scan(PRIMARY, &mut |key, value| {
            let position = primary.ids.len();
            let id = unpack::<(i64,)>(key)
                .ok()
                .map(|(id,)| id)
                .filter(|&id| pack(&(id,)) == key);
            let Some(id) = id else {
                primary.violation(format!(
                    "entry {position}: the key {key:02x?} is not a packed (id,)"
                ));
                return;
            };
            if previous_key
                .as_deref()
                .is_some_and(|previous| key <= previous)
                || primary.ids.last().is_some_and(|&previous| id <= previous)
            {
                primary.violation(format!(
                    "entry {position}: ID {id} does not follow ID {} in strictly increasing order",
                    primary.ids.last().copied().unwrap_or_default()
                ));
            }
            primary.ids.push(id);
            primary.values.extend_from_slice(value);
            primary.ends.push(primary.values.len());
            previous_key = Some(key.to_vec());
        })?;
        Ok(primary)
    }

    fn violation(&mut self, text: String) {
        if self.order_violation.is_none() {
            self.order_violation = Some(text);
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The ID at `position`, in file order.
    pub fn id(&self, position: usize) -> i64 {
        self.ids[position]
    }

    /// The value at `position`.
    pub fn value(&self, position: usize) -> &[u8] {
        let start = position
            .checked_sub(1)
            .map_or(0, |before| self.ends[before]);
        &self.values[start..self.ends[position]]
    }

    /// The position of `id`, found by binary search: meaningful only when the IDs are in order.
    pub fn find(&self, id: i64) -> Option<usize> {
        self.ids.binary_search(&id).ok()
    }
}
