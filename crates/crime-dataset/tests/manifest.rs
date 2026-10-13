use crime_dataset::{
    Compression, IndexInfo, IndexKind, MANIFEST_FILE_NAME, Manifest, PRIMARY_FILE_NAME,
    RecordCounts, SCHEMA_VERSION, SourceInfo, TableInfo,
};
use serde_json::{Value, json};

fn sample_manifest() -> Manifest {
    Manifest {
        schema_version: SCHEMA_VERSION,
        generated_at: "2026-10-12T10:15:00+02:00".to_owned(),
        source: SourceInfo {
            file_name: "chicago-crime-data.csv".to_owned(),
            size_bytes: 2_345_678_901,
            sha256: "ab".repeat(32),
        },
        records: RecordCounts {
            valid: 8_656_900,
            rejected: 4,
        },
        compression: Compression::Snappy,
        primary: TableInfo {
            file: PRIMARY_FILE_NAME.to_owned(),
            entries: 8_656_900,
            size_bytes: 600_000_000,
        },
        indexes: IndexKind::ALL
            .iter()
            .map(|kind| IndexInfo {
                name: kind.name().to_owned(),
                file: kind.file_name().to_owned(),
                entries: 8_656_900,
                size_bytes: 90_000_000,
            })
            .collect(),
    }
}

#[test]
fn manifest_round_trips_through_json() {
    let manifest = sample_manifest();

    assert_eq!(
        Manifest::from_json(&manifest.to_json()).ok(),
        Some(manifest)
    );
}

#[test]
fn manifest_json_uses_the_documented_field_names() {
    let mut manifest = sample_manifest();
    manifest.compression = Compression::None;
    let json: Value = serde_json::from_str(&manifest.to_json()).expect("valid JSON");

    assert_eq!(SCHEMA_VERSION, 2);
    assert_eq!(MANIFEST_FILE_NAME, "manifest.json");
    assert_eq!(json["schema_version"], json!(2));
    assert_eq!(json["generated_at"], json!("2026-10-12T10:15:00+02:00"));
    assert_eq!(json["source"]["file_name"], json!("chicago-crime-data.csv"));
    assert_eq!(json["source"]["size_bytes"], json!(2_345_678_901_u64));
    assert_eq!(json["source"]["sha256"], json!("ab".repeat(32)));
    assert_eq!(
        json["records"],
        json!({ "valid": 8_656_900, "rejected": 4 })
    );
    assert_eq!(json["compression"], json!("none"));
    assert_eq!(json["primary"]["file"], json!(PRIMARY_FILE_NAME));
    assert_eq!(json["primary"]["entries"], json!(8_656_900));
    assert_eq!(json["indexes"][0]["name"], json!("date"));
    assert_eq!(
        json["indexes"][4]["file"],
        json!(IndexKind::CaseNumber.file_name())
    );
    assert_eq!(json["indexes"][4]["size_bytes"], json!(90_000_000));
}

#[test]
fn snappy_compression_is_written_in_lowercase() {
    let json: Value = serde_json::from_str(&sample_manifest().to_json()).expect("valid JSON");

    assert_eq!(json["compression"], json!("snappy"));
}

#[test]
fn from_json_rejects_text_that_is_not_a_manifest() {
    assert!(Manifest::from_json("{}").is_err());
    assert!(Manifest::from_json("not json").is_err());
}
