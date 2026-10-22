//! The CanopyDB manifest as the query app accepts it (decisions KC3 and KC4).
//!
//! The manifest is read as JSON and checked by hand rather than through a typed model, because
//! the rules are about presence and form: every field of the CanopyDB generator's manifest
//! (decision KG3) must be there with its type, `engine.fork_commit` included even though it may
//! be `null`, and unknown fields are tolerated, as in the original query app.

use crime_dataset::IndexKind;
use crime_dataset::canopy::{ENGINE_NAME, PRIMARY_TABLE_NAME, SCHEMA_VERSION};
use serde_json::{Map, Value};

/// The prefix of a CanopyDB `schema_version`, followed by the number of the logical schema.
const SCHEMA_PREFIX: &str = "canopydb-";

/// What the query app needs from a valid manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanopyManifest {
    pub generated_at: String,
    /// `engine.version`, as written.
    pub engine_version: String,
    /// `engine.fork_commit`, `None` when `null`.
    pub fork_commit: Option<String>,
    /// Every path of `files`, relative to the version folder, `/`-separated.
    pub files: Vec<String>,
}

/// Why a manifest is not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Not a manifest of a CanopyDB dataset of this app: not JSON, a `schema_version` that is not
    /// `canopydb-<digits>` (an SSTable dataset), or a `canopydb-2` manifest without the shape.
    Invalid,
    /// A CanopyDB dataset of another schema: the digits after `canopydb-`, as written.
    OtherSchema(String),
}

/// Reads the manifest text of a version folder.
pub fn parse(text: &str) -> Result<CanopyManifest, ManifestError> {
    let root: Value = serde_json::from_str(text).map_err(|_| ManifestError::Invalid)?;
    let root = root.as_object().ok_or(ManifestError::Invalid)?;
    match root.get("schema_version").and_then(Value::as_str) {
        Some(SCHEMA_VERSION) => {}
        Some(other) => {
            return Err(match other.strip_prefix(SCHEMA_PREFIX) {
                Some(digits)
                    if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) =>
                {
                    ManifestError::OtherSchema(digits.to_owned())
                }
                _ => ManifestError::Invalid,
            });
        }
        None => return Err(ManifestError::Invalid),
    }
    shape(root).ok_or(ManifestError::Invalid)
}

/// The fields of a `canopydb-2` manifest, or `None` when one is missing or has the wrong form.
fn shape(root: &Map<String, Value>) -> Option<CanopyManifest> {
    let generated_at = text(root, "generated_at")?.to_owned();

    let engine = root.get("engine")?.as_object()?;
    if text(engine, "name")? != ENGINE_NAME {
        return None;
    }
    let engine_version = non_empty(text(engine, "version")?)?.to_owned();
    let fork_commit = match engine.get("fork_commit")? {
        Value::Null => None,
        Value::String(commit) => Some(non_empty(commit)?.to_owned()),
        _ => return None,
    };

    let source = root.get("source")?.as_object()?;
    text(source, "file_name")?;
    number(source, "size_bytes")?;
    text(source, "sha256")?;
    let records = root.get("records")?.as_object()?;
    number(records, "valid")?;
    number(records, "rejected")?;
    text(root, "compression")?;

    let tables = root.get("tables")?.as_array()?;
    let names =
        std::iter::once(PRIMARY_TABLE_NAME).chain(IndexKind::ALL.iter().map(|kind| kind.name()));
    if tables.len() != 1 + IndexKind::ALL.len() {
        return None;
    }
    for (table, name) in tables.iter().zip(names) {
        let table = table.as_object()?;
        if text(table, "name")? != name {
            return None;
        }
        number(table, "entries")?;
    }

    let mut files = Vec::new();
    for file in root.get("files")?.as_array()? {
        let file = file.as_object()?;
        let path = text(file, "path")?;
        number(file, "size_bytes")?;
        if !is_base_path(path) {
            return None;
        }
        files.push(path.to_owned());
    }

    Some(CanopyManifest {
        generated_at,
        engine_version,
        fork_commit,
        files,
    })
}

/// A path of `files`: inside `canopydb/`, `/`-separated, without empty, `.` or `..` parts.
fn is_base_path(path: &str) -> bool {
    let mut parts = path.split('/');
    parts.next() == Some(crime_dataset::canopy::BASE_DIR_NAME)
        && path.contains('/')
        && parts.all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains('\\'))
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key)?.as_str()
}

fn number(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key)?.as_u64()
}

fn non_empty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest() -> Value {
        json!({
            "schema_version": "canopydb-2",
            "generated_at": "2026-10-22T10:15:00+02:00",
            "engine": {"name": "canopydb", "version": "0.2.5", "fork_commit": null},
            "source": {"file_name": "crimes.csv", "size_bytes": 10, "sha256": "ab"},
            "records": {"valid": 7, "rejected": 0},
            "compression": "lz4",
            "tables": [
                {"name": "primary", "entries": 7},
                {"name": "date", "entries": 7},
                {"name": "primary_type", "entries": 7},
                {"name": "district", "entries": 7},
                {"name": "community_area", "entries": 7},
                {"name": "case_number", "entries": 7}
            ],
            "files": [
                {"path": "canopydb/LOCK", "size_bytes": 0},
                {"path": "canopydb/crime_1_l/DATA", "size_bytes": 4096}
            ]
        })
    }

    /// One change of a test manifest.
    type Change = Box<dyn Fn(&mut Value)>;

    fn parsed(value: &Value) -> Result<CanopyManifest, ManifestError> {
        parse(&value.to_string())
    }

    fn changed(change: impl FnOnce(&mut Value)) -> Result<CanopyManifest, ManifestError> {
        let mut value = manifest();
        change(&mut value);
        parsed(&value)
    }

    #[test]
    fn a_generator_manifest_is_accepted() {
        let manifest = parsed(&manifest()).expect("valid");
        assert_eq!(manifest.engine_version, "0.2.5");
        assert_eq!(manifest.fork_commit, None);
        assert_eq!(manifest.generated_at, "2026-10-22T10:15:00+02:00");
        assert_eq!(manifest.files, ["canopydb/LOCK", "canopydb/crime_1_l/DATA"]);
        assert!(
            changed(|m| m["extra"] = json!(1)).is_ok(),
            "unknown fields are tolerated"
        );
        let fork = changed(|m| m["engine"]["fork_commit"] = json!("abc")).expect("valid");
        assert_eq!(fork.fork_commit.as_deref(), Some("abc"));
    }

    #[test]
    fn schema_versions_of_other_canopydb_schemas_keep_their_digits() {
        for (schema, digits) in [
            ("canopydb-3", "3"),
            ("canopydb-02", "02"),
            ("canopydb-1", "1"),
        ] {
            assert_eq!(
                changed(|m| m["schema_version"] = json!(schema)),
                Err(ManifestError::OtherSchema(digits.to_owned())),
                "{schema}"
            );
        }
    }

    #[test]
    fn anything_else_is_not_a_dataset_of_this_app() {
        let invalid: Vec<Change> = vec![
            Box::new(|m| m["schema_version"] = json!(2)),
            Box::new(|m| m["schema_version"] = json!("2")),
            Box::new(|m| m["schema_version"] = json!("canopydb-")),
            Box::new(|m| m["schema_version"] = json!("CANOPYDB-2")),
            Box::new(|m| m["schema_version"] = json!("canopydb-x")),
            Box::new(|m| m["schema_version"] = json!("canopydb-2.0")),
            Box::new(|m| m["schema_version"] = json!(null)),
            Box::new(|m| {
                m.as_object_mut().expect("object").remove("schema_version");
            }),
            Box::new(|m| {
                m.as_object_mut().expect("object").remove("tables");
            }),
            Box::new(|m| {
                m["tables"].as_array_mut().expect("array").pop();
            }),
            Box::new(|m| m["tables"].as_array_mut().expect("array").swap(1, 2)),
            Box::new(|m| m["tables"][0]["entries"] = json!("7")),
            Box::new(|m| m["engine"]["name"] = json!("rocksdb")),
            Box::new(|m| m["engine"]["version"] = json!("")),
            Box::new(|m| m["engine"]["version"] = json!(25)),
            Box::new(|m| {
                m["engine"]
                    .as_object_mut()
                    .expect("object")
                    .remove("fork_commit");
            }),
            Box::new(|m| m["engine"]["fork_commit"] = json!("")),
            Box::new(|m| {
                m.as_object_mut().expect("object").remove("source");
            }),
            Box::new(|m| {
                m["records"]
                    .as_object_mut()
                    .expect("object")
                    .remove("rejected");
            }),
            Box::new(|m| m["compression"] = json!(null)),
            Box::new(|m| {
                m.as_object_mut().expect("object").remove("files");
            }),
            Box::new(|m| m["files"][0]["path"] = json!("LOCK")),
            Box::new(|m| m["files"][0]["path"] = json!("canopydb/../manifest.json")),
            Box::new(|m| m["files"][0]["path"] = json!("canopydb/./LOCK")),
            Box::new(|m| m["files"][0]["path"] = json!("canopydb/")),
            Box::new(|m| m["files"][0]["path"] = json!("canopydb\\LOCK")),
            Box::new(|m| m["files"][0]["path"] = json!("/canopydb/LOCK")),
        ];
        for (case, change) in invalid.iter().enumerate() {
            assert_eq!(changed(change), Err(ManifestError::Invalid), "case {case}");
        }
        assert_eq!(parse("not json"), Err(ManifestError::Invalid));
        assert_eq!(parse("[]"), Err(ManifestError::Invalid));
    }
}
