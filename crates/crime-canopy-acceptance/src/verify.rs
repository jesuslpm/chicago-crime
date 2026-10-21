//! The CSV mode: every check of the original verifier on a CanopyDB dataset, in report order.
//!
//! | Criterion | What it proves |
//! |---|---|
//! | `LAYOUT` | `CURRENT` names a version folder that holds only `manifest.json` and `canopydb/`. When the folder holds no CanopyDB dataset, it is the only row, FAIL (`not a CanopyDB dataset` when the manifest is not a CanopyDB one). |
//! | `MANIFEST-SCHEMA` | `schema_version` `canopydb-2`, the PRD's fields in its order, `engine.name` `canopydb`, `compression` `lz4` or `none` and the six tables in order. |
//! | `MANIFEST-FILES` | The paths of `files` are the files of `canopydb/`, listed before the base is opened, in byte order. Sizes are not compared: opening the base can change them. |
//! | `CSV`, `MANIFEST-SOURCE` | As in the original verifier. |
//! | `MANIFEST-TABLES` | Each `entries` of `tables` equals the entries read from its tree. |
//! | `AC-G1` … `AC-C18-LOWERCASE` | The original verifier's content checks, reading the trees instead of the SSTables. |

use std::fs;
use std::path::Path;

use crime_acceptance::dataset::{self, MANIFEST_FILE, PRIMARY};
use crime_acceptance::indexes::INDEXES;
use crime_acceptance::report::thousands;
use crime_acceptance::{Check, Options, Report, TableCounts, verify_content};

use crate::canopy::{BASE_DIR, CanopyTables, TABLES};
use crate::manifest::{self, CanopyManifest, Engine};

/// The summary of `LAYOUT` when the folder holds a dataset that is not a CanopyDB one.
pub const NOT_CANOPY: &str = "not a CanopyDB dataset";
const ENGINE_NAME: &str = "canopydb";
const COMPRESSIONS: [&str; 2] = ["lz4", "none"];

/// Verifies the CanopyDB dataset published in the output folder `dataset` against the CSV `csv`.
/// `progress` receives a short line before each long step.
pub fn verify(
    csv: &Path,
    dataset: &Path,
    options: Options,
    progress: &mut dyn FnMut(&str),
) -> Report {
    let mut report = Report::default();
    progress("resolving CURRENT");
    let version = match dataset::resolve_current(dataset) {
        Ok(version) => version,
        Err(error) => {
            report.checks.push(Check::new("LAYOUT", false, error));
            return report;
        }
    };
    let text = match manifest::identify(&version) {
        Ok((Engine::CanopyDb, text)) => text,
        Ok((Engine::Sstable, _)) => {
            report
                .checks
                .push(
                    Check::new("LAYOUT", false, NOT_CANOPY).with_details(vec![format!(
                        "{MANIFEST_FILE} has schema_version {}, the SSTable one",
                        manifest::SSTABLE_SCHEMA_VERSION
                    )]),
                );
            return report;
        }
        Err(error) => {
            report
                .checks
                .push(Check::new("LAYOUT", false, NOT_CANOPY).with_details(vec![error]));
            return report;
        }
    };
    report.checks.push(layout_check(&version));
    let manifest = CanopyManifest::parse(&text);
    report.checks.push(schema_check(&manifest));
    let Ok(manifest) = manifest else {
        return report;
    };
    // Before the base is opened: opening it creates `LOCK` when it is missing.
    report.checks.push(files_check(&version, &manifest));

    progress("opening the CanopyDB base");
    let tables = match CanopyTables::open(&version.join(BASE_DIR)) {
        Ok(tables) => tables,
        Err(error) => {
            report.checks.push(Check::new("AC-G1", false, error));
            return report;
        }
    };
    report.checks.extend(verify_content(
        csv,
        &tables,
        &manifest.content,
        options,
        progress,
        &|counts| tables_check(&manifest, counts),
    ));
    report
}

fn layout_check(version: &Path) -> Check {
    let name = version
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let expected = [format!("{BASE_DIR}/"), MANIFEST_FILE.to_owned()];
    match dataset::folder_contents(version) {
        Ok(found) if found == expected => Check::new(
            "LAYOUT",
            true,
            format!("CURRENT names {name}, which holds exactly {MANIFEST_FILE} and {BASE_DIR}/"),
        ),
        Ok(found) => Check::new(
            "LAYOUT",
            false,
            format!("{name} holds {found:?}; expected exactly {expected:?}"),
        ),
        Err(error) => Check::new("LAYOUT", false, error),
    }
}

fn schema_check(manifest: &Result<CanopyManifest, String>) -> Check {
    let manifest = match manifest {
        Ok(manifest) => manifest,
        Err(error) => return Check::new("MANIFEST-SCHEMA", false, error.clone()),
    };
    let mut details = manifest.field_order_problems.clone();
    if manifest.schema_version != manifest::SCHEMA_VERSION {
        details.push(format!(
            "the schema version must be {:?}",
            manifest::SCHEMA_VERSION
        ));
    }
    if manifest.engine_name != ENGINE_NAME {
        details.push(format!("the engine must be {ENGINE_NAME:?}"));
    }
    if !COMPRESSIONS.contains(&manifest.compression.as_str()) {
        details.push(format!("the compression must be one of {COMPRESSIONS:?}"));
    }
    let names: Vec<&str> = manifest
        .tables
        .iter()
        .map(|table| table.name.as_str())
        .collect();
    if names != TABLES {
        details.push(format!("the tables are {names:?}, expected {TABLES:?}"));
    }
    Check::new(
        "MANIFEST-SCHEMA",
        details.is_empty(),
        format!(
            "schema_version {}, engine {} {}, compression {}, generated_at {}, tables {}",
            manifest.schema_version,
            manifest.engine_name,
            manifest.engine_version,
            manifest.compression,
            manifest.generated_at,
            names.join(", ")
        ),
    )
    .with_details(details)
}

fn files_check(version: &Path, manifest: &CanopyManifest) -> Check {
    const CRITERION: &str = "MANIFEST-FILES";
    let on_disk = match base_files(version) {
        Ok(files) => files,
        Err(error) => return Check::new(CRITERION, false, error),
    };
    let listed: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let mut details: Vec<String> = listed
        .iter()
        .filter(|path| !on_disk.iter().any(|file| file == *path))
        .map(|path| format!("{path} is in the manifest but not in {BASE_DIR}/"))
        .collect();
    details.extend(
        on_disk
            .iter()
            .filter(|file| !listed.contains(&file.as_str()))
            .map(|file| format!("{file} is in {BASE_DIR}/ but not in the manifest")),
    );
    if details.is_empty() && listed != on_disk {
        details
            .push("the manifest does not list the files in byte order of their paths".to_owned());
    }
    Check::new(
        CRITERION,
        details.is_empty(),
        format!(
            "{} files in the manifest, {} in {BASE_DIR}/; paths compared, not sizes, which can \
             change when the base is opened",
            listed.len(),
            on_disk.len()
        ),
    )
    .with_details(details)
}

/// Every file under `canopydb/` of `version`, as a path relative to the version folder separated
/// by `/`, in byte order.
fn base_files(version: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let mut pending = vec![(version.join(BASE_DIR), BASE_DIR.to_owned())];
    while let Some((dir, relative)) = pending.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|error| format!("{} cannot be listed: {error}", dir.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("{} cannot be listed: {error}", dir.display()))?;
            let path = format!("{relative}/{}", entry.file_name().to_string_lossy());
            if entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
                pending.push((entry.path(), path));
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn tables_check(manifest: &CanopyManifest, counts: &TableCounts) -> Check {
    let read = std::iter::once((PRIMARY, Some(counts.primary))).chain(
        INDEXES
            .iter()
            .map(|spec| spec.name)
            .zip(counts.indexes.iter().copied()),
    );
    let mut details = Vec::new();
    for (name, entries) in read {
        match manifest.tables.iter().find(|table| table.name == name) {
            None => details.push(format!("{name}: not in the manifest")),
            Some(table) if Some(table.entries) != entries => details.push(format!(
                "{name}: manifest {} entries, tree {}",
                thousands(table.entries),
                entries.map_or_else(|| "unreadable".to_owned(), thousands)
            )),
            Some(_) => {}
        }
    }
    Check::new(
        "MANIFEST-TABLES",
        details.is_empty(),
        format!(
            "entries of the {} tables in the manifest compared with the entries read from each \
             tree",
            TABLES.len()
        ),
    )
    .with_details(details)
}
