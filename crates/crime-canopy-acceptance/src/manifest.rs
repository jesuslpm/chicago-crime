//! The CanopyDB manifest (canopy-db-generador PRD, "Manifiesto"), read field by field from the JSON,
//! and the order of its fields, which `serde_json::Value` does not keep.
//!
//! ```json
//! {
//!   "schema_version": "canopydb-2",
//!   "generated_at": "...",
//!   "engine": { "name": "canopydb", "version": "0.2.5", "fork_commit": null },
//!   "source": { "file_name": "...", "size_bytes": 0, "sha256": "..." },
//!   "records": { "valid": 0, "rejected": 0 },
//!   "compression": "lz4",
//!   "tables": [ { "name": "primary", "entries": 0 } ],
//!   "files": [ { "path": "canopydb/LOCK", "size_bytes": 0 } ]
//! }
//! ```

use std::fmt;
use std::fs;
use std::path::Path;

use crime_acceptance::ContentFacts;
use crime_acceptance::dataset::{MANIFEST_FILE, number_at, text_at, value_at};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

/// `schema_version` of a CanopyDB manifest: the engine and the logical schema version 2.
pub const SCHEMA_VERSION: &str = "canopydb-2";
/// `schema_version` of an SSTable manifest.
pub const SSTABLE_SCHEMA_VERSION: u64 = 2;

/// The top-level fields of the manifest, in the order the PRD fixes; the fields of each nested
/// object follow.
const FIELDS: [&str; 8] = [
    "schema_version",
    "generated_at",
    "engine",
    "source",
    "records",
    "compression",
    "tables",
    "files",
];
const ENGINE_FIELDS: [&str; 3] = ["name", "version", "fork_commit"];
const SOURCE_FIELDS: [&str; 3] = ["file_name", "size_bytes", "sha256"];
const RECORDS_FIELDS: [&str; 2] = ["valid", "rejected"];
const TABLE_FIELDS: [&str; 2] = ["name", "entries"];
const FILE_FIELDS: [&str; 2] = ["path", "size_bytes"];

/// Which engine a manifest belongs to, by its `schema_version` alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    /// `schema_version` is the text `canopydb-2`.
    CanopyDb,
    /// `schema_version` is the number 2.
    Sstable,
}

/// The parsed manifest text and the engine its `schema_version` names, or why it names none.
pub fn identify(version: &Path) -> Result<(Engine, String), String> {
    let path = version.join(MANIFEST_FILE);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let json: Value =
        serde_json::from_str(&text).map_err(|error| format!("{MANIFEST_FILE}: {error}"))?;
    match json.get("schema_version") {
        Some(Value::String(schema)) if schema == SCHEMA_VERSION => Ok((Engine::CanopyDb, text)),
        Some(schema) if schema.as_u64() == Some(SSTABLE_SCHEMA_VERSION) => {
            Ok((Engine::Sstable, text))
        }
        Some(schema) => Err(format!(
            "{MANIFEST_FILE} has schema_version {schema}, neither {SCHEMA_VERSION:?} nor \
             {SSTABLE_SCHEMA_VERSION}"
        )),
        None => Err(format!("{MANIFEST_FILE} has no schema_version")),
    }
}

/// One table as `tables` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableFacts {
    pub name: String,
    pub entries: u64,
}

/// One file as `files` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub path: String,
    pub size_bytes: u64,
}

/// What a CanopyDB manifest says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanopyManifest {
    pub schema_version: String,
    pub generated_at: String,
    pub engine_name: String,
    pub engine_version: String,
    pub content: ContentFacts,
    pub compression: String,
    pub tables: Vec<TableFacts>,
    pub files: Vec<FileFacts>,
    /// Objects whose fields are not exactly those of the PRD, in its order.
    pub field_order_problems: Vec<String>,
}

impl CanopyManifest {
    /// Reads every field of the manifest text; a missing field or one of the wrong type is an
    /// error, while fields out of order are listed in [`Self::field_order_problems`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let json: Value =
            serde_json::from_str(text).map_err(|error| format!("{MANIFEST_FILE}: {error}"))?;
        let shape: Shape =
            serde_json::from_str(text).map_err(|error| format!("{MANIFEST_FILE}: {error}"))?;
        let list = |name: &str| {
            json.get(name)
                .and_then(Value::as_array)
                .ok_or_else(|| format!("the manifest has no \"{name}\" list"))
        };
        let tables = list("tables")?
            .iter()
            .map(|table| {
                Ok(TableFacts {
                    name: text_at(table, &["name"])?,
                    entries: number_at(table, &["entries"])?,
                })
            })
            .collect::<Result<_, String>>()?;
        let files = list("files")?
            .iter()
            .map(|file| {
                Ok(FileFacts {
                    path: text_at(file, &["path"])?,
                    size_bytes: number_at(file, &["size_bytes"])?,
                })
            })
            .collect::<Result<_, String>>()?;
        let fork_commit = value_at(&json, &["engine", "fork_commit"])?;
        if !(fork_commit.is_null() || fork_commit.is_string()) {
            return Err("\"engine.fork_commit\" is neither null nor text".to_owned());
        }
        Ok(Self {
            schema_version: text_at(&json, &["schema_version"])?,
            generated_at: text_at(&json, &["generated_at"])?,
            engine_name: text_at(&json, &["engine", "name"])?,
            engine_version: text_at(&json, &["engine", "version"])?,
            content: ContentFacts {
                source_file_name: text_at(&json, &["source", "file_name"])?,
                source_size_bytes: number_at(&json, &["source", "size_bytes"])?,
                source_sha256: text_at(&json, &["source", "sha256"])?,
                valid: number_at(&json, &["records", "valid"])?,
                rejected: number_at(&json, &["records", "rejected"])?,
            },
            compression: text_at(&json, &["compression"])?,
            tables,
            files,
            field_order_problems: field_order_problems(&shape),
        })
    }
}

/// The objects whose fields differ from the PRD's list or order.
fn field_order_problems(shape: &Shape) -> Vec<String> {
    let mut problems = Vec::new();
    let mut expect = |what: &str, shape: Option<&Shape>, expected: &[&str]| {
        let found = shape.map(Shape::keys).unwrap_or_default();
        if found != expected {
            problems.push(format!(
                "the fields of {what} are {found:?}; expected {expected:?}"
            ));
        }
    };
    expect("the manifest", Some(shape), &FIELDS);
    expect("engine", shape.field("engine"), &ENGINE_FIELDS);
    expect("source", shape.field("source"), &SOURCE_FIELDS);
    expect("records", shape.field("records"), &RECORDS_FIELDS);
    for (list, fields) in [("tables", &TABLE_FIELDS[..]), ("files", &FILE_FIELDS[..])] {
        if let Some(Shape::Array(items)) = shape.field(list) {
            for (position, item) in items.iter().enumerate() {
                expect(&format!("{list}[{position}]"), Some(item), fields);
            }
        }
    }
    problems
}

/// The structure of a JSON document with the keys of each object in document order.
#[derive(Debug)]
enum Shape {
    Object(Vec<(String, Shape)>),
    Array(Vec<Shape>),
    Scalar,
}

impl Shape {
    /// The keys of an object, in order; nothing for anything else.
    fn keys(&self) -> Vec<&str> {
        match self {
            Self::Object(fields) => fields.iter().map(|(key, _)| key.as_str()).collect(),
            _ => Vec::new(),
        }
    }

    /// The first field named `key` of an object.
    fn field(&self, key: &str) -> Option<&Shape> {
        match self {
            Self::Object(fields) => fields
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, shape)| shape),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Shape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ShapeVisitor)
    }
}

struct ShapeVisitor;

impl<'de> Visitor<'de> for ShapeVisitor {
    type Value = Shape;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_str<E>(self, _: &str) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_unit<E>(self) -> Result<Shape, E> {
        Ok(Shape::Scalar)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Shape, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Shape>()? {
            items.push(item);
        }
        Ok(Shape::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Shape, A::Error> {
        let mut fields = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Shape>()? {
            fields.push((key, value));
        }
        Ok(Shape::Object(fields))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
  "schema_version": "canopydb-2",
  "generated_at": "2026-10-21T10:15:00+02:00",
  "engine": {
    "name": "canopydb",
    "version": "0.2.5",
    "fork_commit": null
  },
  "source": {
    "file_name": "crimes.csv",
    "size_bytes": 10,
    "sha256": "ab"
  },
  "records": {
    "valid": 1,
    "rejected": 0
  },
  "compression": "lz4",
  "tables": [
    {
      "name": "primary",
      "entries": 1
    }
  ],
  "files": [
    {
      "path": "canopydb/LOCK",
      "size_bytes": 0
    }
  ]
}
"#;

    #[test]
    fn the_prd_example_parses_with_its_fields_in_order() {
        let manifest = CanopyManifest::parse(MANIFEST).expect("the example parses");

        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        assert_eq!(manifest.engine_name, "canopydb");
        assert_eq!(manifest.content.valid, 1);
        assert_eq!(
            manifest.tables,
            [TableFacts {
                name: "primary".to_owned(),
                entries: 1
            }]
        );
        assert_eq!(manifest.files[0].path, "canopydb/LOCK");
        assert!(
            manifest.field_order_problems.is_empty(),
            "{:?}",
            manifest.field_order_problems
        );
    }

    #[test]
    fn fields_out_of_order_are_listed_by_object() {
        let swapped = MANIFEST.replace(
            "\"name\": \"canopydb\",\n    \"version\": \"0.2.5\",",
            "\"version\": \"0.2.5\",\n    \"name\": \"canopydb\",",
        );
        assert_ne!(swapped, MANIFEST);

        let manifest = CanopyManifest::parse(&swapped).expect("the manifest still parses");

        assert_eq!(
            manifest.field_order_problems,
            [
                "the fields of engine are [\"version\", \"name\", \"fork_commit\"]; expected \
                 [\"name\", \"version\", \"fork_commit\"]"
            ]
        );
    }

    #[test]
    fn a_missing_field_is_an_error() {
        let without = MANIFEST.replace("\"compression\": \"lz4\",\n", "");

        let error = CanopyManifest::parse(&without).expect_err("compression is required");

        assert_eq!(error, "the manifest has no \"compression\"");
    }
}
