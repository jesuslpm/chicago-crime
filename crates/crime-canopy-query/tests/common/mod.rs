//! Fixtures of the CanopyDB query app tests: the rows of the original query app's tests,
//! published by the CanopyDB generator (and by the original one, to compare both apps), and
//! runs of either app with stdout and stderr captured.

// Each test crate uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use crime_canopy_generator::CanopyStorage;
use crime_generator::cancel::CancelFlag;
use crime_generator::cli::Config;
use crime_generator::index::DEFAULT_INDEX_SEGMENT_SIZE;
use crime_generator::memory::DEFAULT_MEMORY_LIMIT_MIB;
use crime_generator::preflight::create_log;
use crime_generator::sort::DEFAULT_SEGMENT_SIZE;
use crime_query::App;
use crime_query::cli::{self, Parsed};
use tempfile::TempDir;

/// The fixtures of the original query app's tests: its rows and its SSTable datasets.
#[path = "../../../crime-query/tests/common/mod.rs"]
pub mod original;

#[allow(unused_imports)]
pub use original::{Row, csv_text, fixture_rows, many_rows, row};

/// A temporary folder where the CanopyDB generator publishes datasets into `<dir>/out`.
pub struct CanopyFixture {
    pub dir: TempDir,
    runs: usize,
}

impl CanopyFixture {
    /// A fixture with `rows` already published.
    pub fn with_rows(rows: &[Row]) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        fs::create_dir(dir.path().join("tmp")).expect("the temporary folder can be created");
        let mut fixture = Self { dir, runs: 0 };
        fixture.publish(rows);
        fixture
    }

    /// The output folder: the folder the analyst gives the query app.
    pub fn output(&self) -> PathBuf {
        self.dir.path().join("out")
    }

    /// The version folder that `CURRENT` names, read without the dataset crate.
    pub fn version_dir(&self) -> PathBuf {
        let current = fs::read_to_string(self.output().join("CURRENT")).expect("CURRENT exists");
        self.output().join(current.trim_end())
    }

    /// The manifest of the active version.
    pub fn manifest_path(&self) -> PathBuf {
        self.version_dir().join("manifest.json")
    }

    /// Publishes `rows` with the CanopyDB generator, replacing the dataset already there.
    pub fn publish(&mut self, rows: &[Row]) {
        self.runs += 1;
        let input = self.dir.path().join(format!("crimes-{}.csv", self.runs));
        fs::write(&input, csv_text(rows)).expect("the input can be written");
        let config = Config {
            input,
            output: self.output(),
            temp_dir: self.dir.path().join("tmp"),
            log: self.dir.path().join(format!("run-{}.log", self.runs)),
            overwrite: true,
            compression: crime_dataset::Compression::Snappy,
            segment_size: DEFAULT_SEGMENT_SIZE,
            index_segment_size: DEFAULT_INDEX_SEGMENT_SIZE,
            memory_limit_mib: DEFAULT_MEMORY_LIMIT_MIB,
        };
        let mut log = create_log(&config.log, false).expect("the log can be created");
        let storage = CanopyStorage::new(true, &config.output);
        let result = crime_generator::generate_with(
            &config,
            &crime_canopy_generator::APP,
            storage,
            &mut log,
            &CancelFlag::new(),
        );
        drop(log);
        if let Err(abort) = result {
            let content = fs::read_to_string(&config.log).unwrap_or_default();
            panic!("the CanopyDB generator aborted: {abort}\n{content}");
        }
    }
}

/// How one run of a query app ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    /// The stderr lines, with the times of the stats lines masked.
    pub fn stderr_lines(&self) -> Vec<String> {
        self.stderr.lines().map(mask_time).collect()
    }
}

/// A stats line or a batch line with its time replaced by `T`.
pub fn mask_time(line: &str) -> String {
    let timed = line.contains(" resultado") || line.starts_with("Lote completado:");
    let Some(at) = line.find(" en ").filter(|_| timed) else {
        return line.to_owned();
    };
    let rest = &line[at + 4..];
    match rest.find(" s.") {
        Some(end) => format!("{} en T{}", &line[..at], &rest[end..]),
        None => line.to_owned(),
    }
}

/// Runs `app` with `--dataset <dataset>` and `args`, never cancelled.
pub fn run(app: &App, dataset: &Path, args: &[&str]) -> Ran {
    run_cancelled(app, dataset, args, &|| false)
}

/// [`run`] with the cancellation check `cancel`.
pub fn run_cancelled(app: &App, dataset: &Path, args: &[&str], cancel: &dyn Fn() -> bool) -> Ran {
    let mut words: Vec<std::ffi::OsString> =
        vec![app.name.into(), "--dataset".into(), dataset.into()];
    words.extend(args.iter().map(|&arg| arg.into()));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = match cli::parse_for(app, words) {
        Parsed::Run(options) => {
            crime_query::run_with(app, &options, cancel, &mut stdout, &mut stderr)
        }
        Parsed::Batch(options) => {
            crime_query::run_batch_with(app, &options, cancel, &mut stdout, &mut stderr)
        }
        other => panic!("not a query: {other:?}"),
    };
    Ran {
        code,
        stdout: String::from_utf8(stdout).expect("UTF-8 stdout"),
        stderr: String::from_utf8(stderr).expect("UTF-8 stderr"),
    }
}

/// The names under `dir`, recursively, relative and sorted.
pub fn tree(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in fs::read_dir(&folder).expect("the folder can be read") {
            let path = entry.expect("an entry").path();
            names.push(
                path.strip_prefix(dir)
                    .expect("under dir")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    names.sort();
    names
}

/// Copies the folder `from` into `to`, recursively.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy can be created");
    for entry in fs::read_dir(from).expect("the folder can be read") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("the file can be copied");
        }
    }
}

/// Rewrites the manifest at `path` with `change` applied to its JSON.
pub fn edit_manifest(path: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("the manifest can be read"))
            .expect("the manifest is JSON");
    change(&mut manifest);
    fs::write(path, serde_json::to_string_pretty(&manifest).expect("JSON")).expect("written");
}
