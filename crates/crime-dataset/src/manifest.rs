//! Dataset file names and the manifest that describes a published dataset.

use serde::{Deserialize, Serialize};

/// Version of the dataset layout described by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// File name of the manifest inside the dataset folder.
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

/// File name of the primary table inside the dataset folder.
pub const PRIMARY_FILE_NAME: &str = "primary.sst";

/// Index table file names follow `{INDEX_FILE_PREFIX}{index name}{TABLE_FILE_EXTENSION}`.
pub const INDEX_FILE_PREFIX: &str = "index-";

/// Extension shared by every table file (LevelDB table format).
pub const TABLE_FILE_EXTENSION: &str = ".sst";

/// Description of a published dataset, stored as `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`SCHEMA_VERSION`] for datasets written by this crate version.
    pub schema_version: u32,
    /// When the dataset was generated, as written by the generator.
    pub generated_at: String,
    pub source: SourceInfo,
    pub records: RecordCounts,
    pub compression: Compression,
    pub primary: TableInfo,
    pub indexes: Vec<IndexInfo>,
}

/// Identity of the source CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub file_name: String,
    pub size_bytes: u64,
    /// Lowercase hexadecimal SHA-256 of the file content.
    pub sha256: String,
}

/// Records accepted into the dataset and records rejected while reading the CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordCounts {
    pub valid: u64,
    pub rejected: u64,
}

/// Block compression used by every table of the dataset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    Snappy,
    None,
}

/// The primary table file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableInfo {
    pub file: String,
    pub entries: u64,
    pub size_bytes: u64,
}

/// One index table file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub file: String,
    pub entries: u64,
    pub size_bytes: u64,
}

impl Manifest {
    /// Serializes the manifest as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        // Only strings, integers, unit enum variants and lists: serialization cannot fail.
        serde_json::to_string_pretty(self).expect("a manifest always serializes to JSON")
    }

    /// Parses a manifest from JSON.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}
