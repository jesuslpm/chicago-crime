//! Fixtures of the CanopyDB generator tests: a CSV built from rows in code, generations of both
//! generators, and readers of the published tables that do not use the generators' code.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use canopydb::Environment;
use crime_canopy_generator::{
    APP, BASE_DIR_NAME, CanopyStorage, CanopyWritten, DATABASE_NAME, database_options,
    environment_options,
};
use crime_dataset::{Compression, HEADER};
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::Config;
use crime_generator::index::DEFAULT_INDEX_SEGMENT_SIZE;
use crime_generator::memory::DEFAULT_MEMORY_LIMIT_MIB;
use crime_generator::preflight::create_log;
use crime_generator::sort::DEFAULT_SEGMENT_SIZE;
use crime_generator::{Abort, Built};
use sstable::{Options, SSIterator};
use tempfile::TempDir;

/// The tables of a dataset, in manifest order.
pub const TABLES: [&str; 6] = [
    "primary",
    "date",
    "primary_type",
    "district",
    "community_area",
    "case_number",
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

/// The sample row with its own ID, case number, block and primary type, so rows stay
/// distinguishable after they are stored and the indexes get several values.
pub fn distinct_row(id: i64) -> Vec<String> {
    let mut row: Vec<String> = SAMPLE_ROW.iter().map(|field| (*field).to_owned()).collect();
    row[0] = id.to_string();
    row[1] = format!("JC{id}");
    row[3] = format!("{id:05}XX S STATE ST");
    row[5] = ["ARSON", "THEFT", "BATTERY"][id.rem_euclid(3) as usize].to_owned();
    if id % 4 == 0 {
        // Community area without a value.
        row[13] = String::new();
    }
    row
}

/// A CSV with the expected header followed by one [`distinct_row`] per ID.
pub fn csv(ids: &[i64]) -> Vec<u8> {
    let mut writer = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Always)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer
        .write_record(HEADER)
        .expect("writing to memory succeeds");
    for &id in ids {
        writer
            .write_record(distinct_row(id))
            .expect("writing to memory succeeds");
    }
    writer.into_inner().expect("flushing to memory succeeds")
}

/// A folder with an input CSV and a temporary folder, where several generations can run.
pub struct Scenario {
    pub dir: TempDir,
    pub input: PathBuf,
    pub temp_dir: PathBuf,
}

impl Scenario {
    pub fn new(ids: &[i64]) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let input = dir.path().join("crimes.csv");
        fs::write(&input, csv(ids)).expect("the fixture can be written");
        let temp_dir = dir.path().join("tmp");
        fs::create_dir(&temp_dir).expect("the temporary folder can be created");
        Self {
            dir,
            input,
            temp_dir,
        }
    }

    /// Default options with an output folder and a log of their own, both named after `name`.
    pub fn config(&self, name: &str) -> Config {
        Config {
            input: self.input.clone(),
            output: self.dir.path().join(name),
            temp_dir: self.temp_dir.clone(),
            log: self.dir.path().join(format!("{name}.log")),
            overwrite: false,
            compression: Compression::Snappy,
            segment_size: DEFAULT_SEGMENT_SIZE,
            index_segment_size: DEFAULT_INDEX_SEGMENT_SIZE,
            memory_limit_mib: DEFAULT_MEMORY_LIMIT_MIB,
        }
    }
}

/// The same run with another log, `<name>.log` next to the original one, and the given overwrite
/// option.
pub fn rerun(config: &Config, name: &str, overwrite: bool) -> Config {
    let log = config
        .log
        .parent()
        .expect("the log has a folder")
        .join(format!("{name}.log"));
    Config {
        log,
        overwrite,
        ..config.clone()
    }
}

/// The storage the binary builds for `config`.
pub fn storage(config: &Config) -> CanopyStorage {
    CanopyStorage::new(config.compression != Compression::None, &config.output)
}

/// Runs a CanopyDB generation with a log that is not echoed to the console; returns the result
/// and the log content.
pub fn generate(config: &Config) -> (Result<Built<CanopyWritten>, Abort>, String) {
    let mut log = create_log(&config.log, false).expect("the log can be created");
    let result =
        crime_generator::generate_with(config, &APP, storage(config), &mut log, &CancelFlag::new());
    drop(log);
    let content = fs::read_to_string(&config.log).expect("the log can be read");
    (result, content)
}

/// [`generate`], panicking with the log if the run aborts.
pub fn generate_ok(config: &Config) -> (Built<CanopyWritten>, String) {
    let (result, log) = generate(config);
    let built = result.unwrap_or_else(|abort| panic!("{abort}\n{log}"));
    (built, log)
}

/// Runs the original generator; returns the result and the log.
pub fn generate_original(config: &Config) -> (Result<(), Abort>, String) {
    let mut log = create_log(&config.log, false).expect("the log can be created");
    let result = crime_generator::generate(config, &mut log, &CancelFlag::new()).map(|_| ());
    drop(log);
    let content = fs::read_to_string(&config.log).expect("the log can be read");
    (result, content)
}

/// The published version folder of `output`, found by reading `CURRENT` directly.
pub fn dataset_dir(output: &Path) -> PathBuf {
    let current = fs::read_to_string(output.join("CURRENT"))
        .unwrap_or_else(|error| panic!("{} has no CURRENT: {error}", output.display()));
    let name = current
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("CURRENT ends with a line feed: {current:?}"));
    output.join(name)
}

/// The names in `dir`, sorted.
pub fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{} can be listed: {error}", dir.display()))
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Every file under `dir` as a `/`-separated path relative to `root`, with its size, in byte
/// order of the path.
pub fn files_under(root: &Path, dir: &Path) -> Vec<(String, u64)> {
    fn walk(root: &Path, dir: &Path, all: &mut Vec<(String, u64)>) {
        for entry in fs::read_dir(dir).expect("the folder can be listed") {
            let entry = entry.expect("an entry");
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, all);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                all.push((relative, entry.metadata().expect("metadata").len()));
            }
        }
    }
    let mut all = Vec::new();
    walk(root, dir, &mut all);
    all.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    all
}

/// Every file and folder under `dir` with the content of each file, sorted: equal snapshots
/// prove that nothing changed, byte for byte.
pub fn snapshot(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    fn walk(root: &Path, dir: &Path, all: &mut Vec<(String, Option<Vec<u8>>)>) {
        for entry in fs::read_dir(dir).expect("the folder can be listed") {
            let path = entry.expect("an entry").path();
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                all.push((relative, None));
                walk(root, &path, all);
            } else {
                all.push((
                    relative,
                    Some(fs::read(&path).expect("the file can be read")),
                ));
            }
        }
    }
    let mut all = Vec::new();
    walk(dir, dir, &mut all);
    all.sort();
    all
}

/// A table by name, with every key and value in order.
pub type Table = (String, Vec<(Vec<u8>, Vec<u8>)>);

/// Every key and value of each table of the CanopyDB base in the version folder `version`, in
/// manifest order.
pub fn read_canopy(version: &Path) -> Vec<Table> {
    let base = version.join(BASE_DIR_NAME);
    let scratch = tempfile::tempdir().expect("a temporary directory can be created");
    let environment = Environment::with_options(environment_options(&base, scratch.path()))
        .expect("the base opens");
    let database = environment
        .get_database_with(DATABASE_NAME, database_options())
        .expect("the database opens")
        .expect("the database exists");
    let transaction = database.begin_read().expect("a read transaction starts");
    TABLES
        .iter()
        .map(|&name| {
            let tree = transaction
                .get_tree(name.as_bytes())
                .expect("the tree opens")
                .unwrap_or_else(|| panic!("the tree {name} exists"));
            let entries = tree
                .iter()
                .expect("the tree can be iterated")
                .map(|entry| {
                    let (key, value) = entry.expect("an entry");
                    (key.to_vec(), value.to_vec())
                })
                .collect();
            (name.to_owned(), entries)
        })
        .collect()
}

/// Every key and value of each LevelDB table in the version folder `version`, in manifest order.
pub fn read_sst(version: &Path) -> Vec<Table> {
    TABLES
        .iter()
        .map(|&name| {
            let file = if name == "primary" {
                "primary.sst".to_owned()
            } else {
                format!("index-{name}.sst")
            };
            let path = version.join(file);
            let table = sstable::Table::new_from_file(Options::default(), &path)
                .unwrap_or_else(|status| panic!("{} should open: {status:?}", path.display()));
            let mut iterator = table.iter();
            let mut entries = Vec::new();
            while let Some(entry) = SSIterator::next(&mut iterator) {
                entries.push(entry);
            }
            (name.to_owned(), entries)
        })
        .collect()
}

/// A process that has ended but whose ID cannot be taken by another process while this value
/// lives.
pub struct EndedProcess(Child);

impl EndedProcess {
    #[cfg(windows)]
    pub fn new() -> Self {
        let mut child = Command::new("cmd")
            .args(["/C", "exit 0"])
            .spawn()
            .expect("cmd can be started");
        child.wait().expect("cmd ends");
        Self(child)
    }

    /// The child stays a zombie, not reaped, so its ID stays taken.
    #[cfg(not(windows))]
    pub fn new() -> Self {
        let child = Command::new("true").spawn().expect("true can be started");
        let stat = format!("/proc/{}/stat", child.id());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !fs::read_to_string(&stat).is_ok_and(|line| line.contains(") Z ")) {
            assert!(std::time::Instant::now() < deadline, "true ends");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Self(child)
    }

    pub fn id(&self) -> u32 {
        self.0.id()
    }
}
