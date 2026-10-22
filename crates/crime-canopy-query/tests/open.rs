//! Opening a dataset with the CanopyDB query app: which datasets it accepts and the message and
//! exit code of each one it rejects (canopy-db-consultor PRD, "Validaciones y mensajes",
//! AC-KC2, AC-KC3, AC-KC4), and its help (AC-KC9).

mod common;

use std::fs;
use std::path::Path;

use common::original::Fixture;
use common::{CanopyFixture, edit_manifest, fixture_rows, run};
use crime_canopy_query::APP;
use crime_query::cli::{self, Parsed};
use serde_json::{Value, json};

/// One change of a manifest.
type Change = Box<dyn Fn(&mut Value)>;

const QUERY: &[&str] = &["--where", "Primary Type=THEFT", "--count"];

fn not_found(folder: &Path) -> String {
    format!(
        "No se encontró un conjunto de datos válido en {}.\n",
        folder.display()
    )
}

/// Every file under the version folder with its content.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    common::tree(dir)
        .into_iter()
        .filter(|name| dir.join(name).is_file())
        .map(|name| {
            let bytes = fs::read(dir.join(&name)).expect("readable");
            (name, bytes)
        })
        .collect()
}

/// Asserts that querying `fixture` after `change` gives `expected` on stderr with exit code 1,
/// nothing on stdout, and leaves the version folder as it was after the change.
fn rejected_after(fixture: &CanopyFixture, expected: &str, change: impl FnOnce(&Path)) {
    change(&fixture.version_dir());
    let before = snapshot(&fixture.version_dir());
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""), "{}", ran.stderr);
    assert_eq!(ran.stderr, expected);
    assert_eq!(snapshot(&fixture.version_dir()), before);
}

#[test]
fn an_sstable_dataset_of_any_schema_is_not_a_dataset_of_this_app() {
    let sst = Fixture::with_rows(&fixture_rows());
    let ran = run(&APP, &sst.output(), QUERY);
    assert_eq!((ran.code, ran.stderr), (1, not_found(&sst.output())));

    let manifest = sst.version_dir().join("manifest.json");
    let text = fs::read_to_string(&manifest).expect("readable");
    fs::write(
        &manifest,
        text.replace("\"schema_version\": 2", "\"schema_version\": 1"),
    )
    .expect("written");
    let ran = run(&APP, &sst.output(), QUERY);
    assert_eq!((ran.code, ran.stderr), (1, not_found(&sst.output())));
}

#[test]
fn a_canopydb_dataset_of_another_schema_names_its_version_as_written() {
    for (schema, shown) in [("canopydb-3", "3"), ("canopydb-02", "02")] {
        let fixture = CanopyFixture::with_rows(&fixture_rows());
        let expected = format!(
            "El conjunto usa el esquema v{shown} y este consultor admite v2. Regenera el conjunto.\n"
        );
        rejected_after(&fixture, &expected, |version| {
            edit_manifest(&version.join("manifest.json"), |m| {
                m["schema_version"] = json!(schema)
            });
        });
    }
}

#[test]
fn a_manifest_without_the_canopydb_shape_is_not_a_dataset() {
    let changes: Vec<Change> = vec![
        Box::new(|m| m["schema_version"] = json!("canopydb-x")),
        Box::new(|m| m["schema_version"] = json!("canopydb-")),
        Box::new(|m| m["schema_version"] = json!("CANOPYDB-2")),
        Box::new(|m| m["schema_version"] = json!(2)),
        Box::new(|m| m["schema_version"] = json!("2")),
        Box::new(|m| {
            m.as_object_mut().expect("object").remove("schema_version");
        }),
        Box::new(|m| {
            m.as_object_mut().expect("object").remove("tables");
        }),
        Box::new(|m| {
            m["tables"].as_array_mut().expect("array").pop();
        }),
        Box::new(|m| m["tables"].as_array_mut().expect("array").swap(2, 3)),
        Box::new(|m| m["engine"]["name"] = json!("rocksdb")),
        Box::new(|m| {
            m["engine"]
                .as_object_mut()
                .expect("object")
                .remove("fork_commit");
        }),
    ];
    for change in changes {
        let fixture = CanopyFixture::with_rows(&fixture_rows());
        rejected_after(&fixture, &not_found(&fixture.output()), |version| {
            edit_manifest(&version.join("manifest.json"), |m| change(m));
        });
    }
}

#[test]
fn a_manifest_that_is_not_json_is_not_a_dataset() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(&fixture, &not_found(&fixture.output()), |version| {
        fs::write(version.join("manifest.json"), "{").expect("written");
    });
}

#[test]
fn an_unknown_field_of_the_manifest_is_tolerated() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    edit_manifest(&fixture.manifest_path(), |m| m["comment"] = json!("added"));
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!(
        (ran.code, ran.stdout.as_str()),
        (0, "3\n"),
        "{}",
        ran.stderr
    );
}

#[test]
fn a_dataset_of_another_canopydb_version_is_rejected_before_opening_the_base() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(
        &fixture,
        "El conjunto se escribió con CanopyDB 0.2.4 y este consultor usa CanopyDB 0.2.5. \
         Regenera el conjunto.\n",
        |version| {
            edit_manifest(&version.join("manifest.json"), |m| {
                m["engine"]["version"] = json!("0.2.4")
            });
        },
    );
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(
        &fixture,
        &format!(
            "El conjunto se escribió con CanopyDB 0.2.5 (fork {commit}) y este consultor usa \
             CanopyDB 0.2.5. Regenera el conjunto.\n"
        ),
        |version| {
            edit_manifest(&version.join("manifest.json"), |m| {
                m["engine"]["fork_commit"] = json!(commit)
            });
        },
    );
}

#[test]
fn a_base_with_a_missing_file_is_not_a_dataset_and_nothing_is_created() {
    // The lock file is one of the files the manifest lists: CanopyDB would create it again.
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(&fixture, &not_found(&fixture.output()), |version| {
        fs::remove_file(version.join("canopydb/LOCK")).expect("removed");
    });

    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(&fixture, &not_found(&fixture.output()), |version| {
        fs::remove_dir_all(version.join("canopydb")).expect("removed");
    });

    // A listed path that is a folder.
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    rejected_after(&fixture, &not_found(&fixture.output()), |version| {
        fs::remove_file(version.join("canopydb/LOCK")).expect("removed");
        fs::create_dir(version.join("canopydb/LOCK")).expect("created");
    });
}

#[test]
fn an_unlisted_extra_file_in_the_base_is_tolerated() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    fs::write(fixture.version_dir().join("canopydb/notes.txt"), "extra").expect("written");
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!(
        (ran.code, ran.stdout.as_str()),
        (0, "3\n"),
        "{}",
        ran.stderr
    );
}

#[test]
fn a_base_without_one_of_the_six_tables_is_not_a_dataset_and_no_table_is_created() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let base = fixture.version_dir().join("canopydb");
    delete_tree(&base, "case_number");
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    assert_eq!(ran.stderr, not_found(&fixture.output()));
    assert!(!has_tree(&base, "case_number"), "no table was created");
}

#[test]
fn no_dataset_and_a_missing_version_are_not_a_dataset() {
    let folder = tempfile::tempdir().expect("a folder");
    let ran = run(&APP, folder.path(), QUERY);
    assert_eq!((ran.code, ran.stderr), (1, not_found(folder.path())));

    let fixture = CanopyFixture::with_rows(&fixture_rows());
    fs::remove_dir_all(fixture.version_dir()).expect("removed");
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!((ran.code, ran.stderr), (1, not_found(&fixture.output())));
}

#[test]
fn the_help_is_the_original_one_with_the_name_the_description_and_the_wait() {
    let help = |app: &crime_query::App| match cli::parse_for(app, [app.name, "--help"]) {
        Parsed::Help(text) => text,
        other => panic!("not the help: {other:?}"),
    };
    let original = help(&crime_query::APP);
    let ours = help(&APP);
    let expected = original
        .replace(
            "Consulta el conjunto de datos de delitos de Chicago publicado por el generador.",
            "Consulta el conjunto de datos de delitos de Chicago (base CanopyDB) publicado por el \
             generador CanopyDB.",
        )
        .replace("crime-query --dataset", "crime-canopy-query --dataset")
        .trim_end()
        .to_owned()
        + "\n\nSi otro proceso tiene abierto el conjunto de datos, la consulta espera hasta 30 s \
           antes de\nrechazarse con 1.\n";
    assert_eq!(ours, expected);
    assert!(original.contains("Uso: crime-query --dataset <CARPETA> [opciones]"));
}

/// Deletes the tree `name` of the `crime` database of `base`, keeping the stored options.
fn delete_tree(base: &Path, name: &str) {
    let (environment, database) = open_base(base);
    let transaction = database.begin_write().expect("a write transaction");
    assert!(transaction.delete_tree(name.as_bytes()).expect("deleted"));
    transaction.commit().expect("committed");
    // The base has no write-ahead log: only a checkpoint keeps the change.
    database.checkpoint().expect("checkpointed");
    drop(database);
    drop(environment);
}

fn has_tree(base: &Path, name: &str) -> bool {
    let (environment, database) = open_base(base);
    let found = database
        .begin_read()
        .expect("a read transaction")
        .get_tree(name.as_bytes())
        .expect("readable")
        .is_some();
    drop(database);
    drop(environment);
    found
}

fn open_base(base: &Path) -> (canopydb::Environment, canopydb::Database) {
    let options_dir = fs::read_dir(base)
        .expect("readable")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .expect("the database folder");
    let stored: canopydb::DbOptions =
        serde_json::from_slice(&fs::read(options_dir.join("OPTIONS")).expect("readable"))
            .expect("the options");
    let environment = canopydb::Environment::new(base).expect("the base opens");
    let database = environment
        .get_database_with("crime", stored)
        .expect("readable")
        .expect("the database");
    (environment, database)
}
