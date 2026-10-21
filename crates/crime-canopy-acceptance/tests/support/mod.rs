//! Small datasets for the tests: a fixture CSV, the SSTable dataset the original generator builds
//! from it, and a CanopyDB dataset with the same entries, copied table by table into a CanopyDB
//! base and described by a manifest written as the PRD's "Manifiesto" section defines it. The
//! CanopyDB generator is not needed: these tests check the verifier, not the generator.

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use canopydb::{DbOptions, EnvOptions, Environment, TreeOptions};
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::{self, Config, Parsed};
use crime_generator::preflight::create_log;
use serde::Serialize;
use sstable::{Options as TableOptions, SSIterator, Table};
use tempfile::TempDir;

pub const HEADER: [&str; 22] = [
    "ID",
    "Case Number",
    "Date",
    "Block",
    "IUCR",
    "Primary Type",
    "Description",
    "Location Description",
    "Arrest",
    "Domestic",
    "Beat",
    "District",
    "Ward",
    "Community Area",
    "FBI Code",
    "X Coordinate",
    "Y Coordinate",
    "Year",
    "Updated On",
    "Latitude",
    "Longitude",
    "Location",
];

/// The first data row of the reference CSV.
const SAMPLE_ROW: [&str; 22] = [
    "14348063",
    "JK438266",
    "10/03/2026 12:00:00 AM",
    "064XX S ELLIS AVE",
    "1090",
    "ARSON",
    "ATTEMPT ARSON",
    "VEHICLE NON-COMMERCIAL",
    "false",
    "false",
    "0314",
    "003",
    "20",
    "42",
    "09",
    "1184164",
    "1862455",
    "2026",
    "10/10/2026 03:42:19 PM",
    "41.777750747",
    "-87.600396201",
    "(41.777750747, -87.600396201)",
];

/// The six tables, in manifest order, with their SSTable file names.
pub const TABLES: [(&str, &str); 6] = [
    ("primary", "primary.sst"),
    ("date", "index-date.sst"),
    ("primary_type", "index-primary_type.sst"),
    ("district", "index-district.sst"),
    ("community_area", "index-community_area.sst"),
    ("case_number", "index-case_number.sst"),
];

/// Every criterion of the CSV mode, in report order.
pub const CRITERIA: [&str; 19] = [
    "LAYOUT",
    "MANIFEST-SCHEMA",
    "MANIFEST-FILES",
    "CSV",
    "MANIFEST-SOURCE",
    "MANIFEST-TABLES",
    "AC-G1",
    "AC-G3",
    "AC-G3-SAMPLE",
    "AC-G3-DST",
    "AC-G4-date",
    "AC-G4-primary_type",
    "AC-G4-district",
    "AC-G4-community_area",
    "AC-G4-case_number",
    "AC-G4-THEFT",
    "AC-G4-COMMUNITY-NIL",
    "AC-G4-SHARED-CASES",
    "AC-C18-LOWERCASE",
];

/// The sample row with some columns replaced, by name.
pub fn row(changes: &[(&str, &str)]) -> Vec<String> {
    let mut fields: Vec<String> = SAMPLE_ROW.iter().map(|&field| field.to_owned()).collect();
    for &(name, value) in changes {
        let column = HEADER
            .iter()
            .position(|&column| column == name)
            .unwrap_or_else(|| panic!("{name} is a column"));
        fields[column] = value.to_owned();
    }
    fields
}

/// Five rows out of ID order (the original verifier's fixture): a theft written `Theft` without
/// Ward or community area, a row without coordinates that shares its case number with it, a row
/// in the spring-forward hour without a district, and another theft whose case number is that
/// one's in lowercase.
pub fn fixture_rows() -> Vec<Vec<String>> {
    vec![
        row(&[]),
        row(&[
            ("ID", "634"),
            ("Case Number", "JA100"),
            ("Primary Type", "Theft"),
            ("Ward", ""),
            ("Community Area", ""),
        ]),
        row(&[
            ("ID", "900"),
            ("Case Number", "JA100"),
            ("Location Description", ""),
            ("X Coordinate", ""),
            ("Y Coordinate", ""),
            ("Latitude", ""),
            ("Longitude", ""),
            ("Location", ""),
        ]),
        row(&[
            ("ID", "14353993"),
            ("Case Number", "JA200"),
            ("Date", "03/08/2015 02:30:00 AM"),
            ("District", ""),
            ("Year", "2015"),
        ]),
        row(&[
            ("ID", "5000"),
            ("Case Number", "ja200"),
            ("Primary Type", "THEFT"),
            ("Updated On", "11/01/2015 01:30:00 AM"),
            ("Latitude", "41.7"),
            ("Longitude", "-87.68427754"),
        ]),
    ]
}

pub fn csv_bytes(rows: &[Vec<String>], terminator: &str) -> Vec<u8> {
    let mut text = String::new();
    let header: Vec<String> = HEADER.iter().map(|&name| name.to_owned()).collect();
    for fields in std::iter::once(&header).chain(rows) {
        let quoted: Vec<String> = fields.iter().map(|field| format!("\"{field}\"")).collect();
        text.push_str(&quoted.join(","));
        text.push_str(terminator);
    }
    text.into_bytes()
}

/// One key and its value.
pub type Entry = (Vec<u8>, Vec<u8>);

/// A fixture CSV, the SSTable dataset the original generator built from it and a CanopyDB
/// dataset with the same entries, each in its own output folder.
pub struct Fixture {
    pub dir: TempDir,
    pub csv: PathBuf,
    pub sstable: PathBuf,
    pub canopy: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        Self::with_rows(&fixture_rows())
    }

    pub fn with_rows(rows: &[Vec<String>]) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let csv = dir.path().join("crimes.csv");
        fs::write(&csv, csv_bytes(rows, "\n")).expect("the fixture CSV can be written");
        let sstable = dir.path().join("sstable");
        generate_sstable(&csv, &sstable, dir.path());
        let canopy = dir.path().join("canopy");
        build_canopy(&sstable, &canopy, true, |_, _| {});
        Self {
            dir,
            csv,
            sstable,
            canopy,
        }
    }

    /// Another CanopyDB dataset with the fixture's entries, changed by `edit` table by table.
    pub fn canopy_copy(
        &self,
        name: &str,
        compression: bool,
        edit: impl Fn(&str, &mut Vec<Entry>),
    ) -> PathBuf {
        let output = self.dir.path().join(name);
        build_canopy(&self.sstable, &output, compression, edit);
        output
    }

    /// The version folder of the CanopyDB dataset.
    pub fn canopy_version(&self) -> PathBuf {
        version(&self.canopy)
    }
}

/// The version folder that `CURRENT` names in `output`.
pub fn version(output: &Path) -> PathBuf {
    let current = fs::read_to_string(output.join("CURRENT")).expect("CURRENT exists");
    output.join(current.trim_end())
}

/// Runs the original generator on `csv`.
fn generate_sstable(csv: &Path, output: &Path, temp: &Path) {
    let args: Vec<OsString> = vec![
        "crime-generator".into(),
        "--input".into(),
        csv.into(),
        "--output".into(),
        output.into(),
        "--temp-dir".into(),
        temp.into(),
        "--log".into(),
        temp.join("generator.log").into(),
    ];
    let Parsed::Run(args) = cli::parse(args) else {
        panic!("the generator options are valid");
    };
    let config = Config::from_args(args, "20260101-000000");
    let mut log = create_log(&config.log, false).expect("the log can be created");
    crime_generator::generate(&config, &mut log, &CancelFlag::new())
        .unwrap_or_else(|abort| panic!("the fixture dataset is generated: {abort}"));
}

fn read_table(path: &Path) -> Vec<Entry> {
    let table = Table::new_from_file(TableOptions::default(), path).expect("the table opens");
    let mut iterator = table.iter();
    let mut entries = Vec::new();
    while let Some(entry) = SSIterator::next(&mut iterator) {
        entries.push(entry);
    }
    entries
}

#[derive(Serialize)]
struct Manifest {
    schema_version: &'static str,
    generated_at: String,
    engine: Engine,
    source: Source,
    records: Records,
    compression: &'static str,
    tables: Vec<TableEntry>,
    files: Vec<FileEntry>,
}

#[derive(Serialize)]
struct Engine {
    name: &'static str,
    version: &'static str,
    fork_commit: Option<String>,
}

#[derive(Serialize)]
struct Source {
    file_name: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct Records {
    valid: u64,
    rejected: u64,
}

#[derive(Serialize)]
struct TableEntry {
    name: &'static str,
    entries: u64,
}

#[derive(Serialize)]
struct FileEntry {
    path: String,
    size_bytes: u64,
}

/// Writes in `output` a CanopyDB dataset with the entries of the SSTable dataset in `sstable`,
/// changed by `edit` table by table (keys must stay sorted and unique), under the same version
/// folder name, and its manifest with the source and records of the SSTable manifest.
pub fn build_canopy(
    sstable: &Path,
    output: &Path,
    compression: bool,
    edit: impl Fn(&str, &mut Vec<Entry>),
) {
    let source_version = version(sstable);
    let version_name = source_version.file_name().expect("a version folder name");
    let version = output.join(version_name);
    let base = version.join("canopydb");
    fs::create_dir_all(&base).expect("the base folder can be created");

    let mut counts = Vec::new();
    {
        let mut env_options = EnvOptions::new(&base);
        env_options.page_cache_size = 8 * 1024 * 1024;
        let environment = Environment::with_options(env_options).expect("the base opens");
        let mut db_options = DbOptions::new();
        db_options.use_wal = false;
        let database = environment
            .get_or_create_database_with("crime", db_options)
            .expect("the database is created");
        let transaction = database.begin_write().expect("a write transaction begins");
        for (name, file) in TABLES {
            let mut entries = read_table(&source_version.join(file));
            edit(name, &mut entries);
            let mut tree_options = TreeOptions::new();
            if !compression {
                tree_options.compress_overflow_values = None;
            }
            let mut tree = transaction
                .get_or_create_tree_with(name.as_bytes(), tree_options)
                .expect("the tree is created");
            for (key, value) in &entries {
                tree.insert(key, value).expect("the entry is inserted");
            }
            counts.push((name, entries.len() as u64));
        }
        transaction.commit().expect("the transaction commits");
        database.checkpoint().expect("the database checkpoints");
    }

    let source: serde_json::Value = serde_json::from_slice(
        &fs::read(source_version.join("manifest.json")).expect("the SSTable manifest exists"),
    )
    .expect("the SSTable manifest is JSON");
    let text = |path: &[&str]| -> String {
        path.iter()
            .fold(&source, |value, key| &value[key])
            .as_str()
            .expect("a text field")
            .to_owned()
    };
    let number = |path: &[&str]| -> u64 {
        path.iter()
            .fold(&source, |value, key| &value[key])
            .as_u64()
            .expect("a number field")
    };
    let manifest = Manifest {
        schema_version: "canopydb-2",
        generated_at: text(&["generated_at"]),
        engine: Engine {
            name: "canopydb",
            version: "0.2.5",
            fork_commit: None,
        },
        source: Source {
            file_name: text(&["source", "file_name"]),
            size_bytes: number(&["source", "size_bytes"]),
            sha256: text(&["source", "sha256"]),
        },
        records: Records {
            valid: number(&["records", "valid"]),
            rejected: number(&["records", "rejected"]),
        },
        compression: if compression { "lz4" } else { "none" },
        tables: counts
            .into_iter()
            .map(|(name, entries)| TableEntry { name, entries })
            .collect(),
        files: base_files(&version)
            .into_iter()
            .map(|(path, size_bytes)| FileEntry { path, size_bytes })
            .collect(),
    };
    let mut json = serde_json::to_string_pretty(&manifest).expect("the manifest serializes");
    json.push('\n');
    fs::write(version.join("manifest.json"), json).expect("the manifest can be written");
    fs::write(
        output.join("CURRENT"),
        format!("{}\n", version_name.to_string_lossy()),
    )
    .expect("CURRENT can be written");
}

/// Every file under `canopydb/` of `version`, with its path relative to the version folder
/// separated by `/`, in byte order, and its size.
pub fn base_files(version: &Path) -> Vec<(String, u64)> {
    let mut files = Vec::new();
    let mut pending = vec![(version.join("canopydb"), "canopydb".to_owned())];
    while let Some((dir, relative)) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("the folder can be listed") {
            let entry = entry.expect("the entry can be read");
            let name = format!("{relative}/{}", entry.file_name().to_string_lossy());
            let metadata = entry.metadata().expect("the metadata can be read");
            if metadata.is_dir() {
                pending.push((entry.path(), name));
            } else {
                files.push((name, metadata.len()));
            }
        }
    }
    files.sort();
    files
}

/// Every path under `dir`, relative to it and sorted, folders included.
pub fn tree_names(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), String::new())];
    while let Some((folder, relative)) = pending.pop() {
        for entry in fs::read_dir(&folder).expect("the folder can be listed") {
            let entry = entry.expect("the entry can be read");
            let name = format!("{relative}/{}", entry.file_name().to_string_lossy());
            if entry.file_type().expect("the type can be read").is_dir() {
                pending.push((entry.path(), name.clone()));
            }
            names.push(name);
        }
    }
    names.sort();
    names
}

/// Copies the folder `from` to `to`, recursively.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy folder can be created");
    for entry in fs::read_dir(from).expect("the folder can be listed") {
        let entry = entry.expect("the entry can be read");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("the type can be read").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("the file can be copied");
        }
    }
}

/// Runs the verifier binary with `args` and returns its exit code and standard output.
pub fn run(args: &[&std::ffi::OsStr]) -> (Option<i32>, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_crime-canopy-acceptance"))
        .args(args)
        .output()
        .expect("the verifier runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// The `PASS`, `FAIL` and `SKIP` lines of a report.
pub fn outcome_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| {
            ["PASS  ", "FAIL  ", "SKIP  "]
                .iter()
                .any(|outcome| line.starts_with(outcome))
        })
        .collect()
}
