//! Errors of the engine (decision KC5, AC-KC5): the detail line with CanopyDB's text, then the
//! original query app's message for a dataset that cannot be read, exit code 1, alone and in a
//! batch, and no half output file.

mod common;

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use common::{CanopyFixture, fixture_rows, run};
use crime_canopy_query::APP;
use crime_dataset::IndexKind;
use crime_query::{App, KeyCursor, OpenError, ReadError, Source, Tables, TablesSource};

const DETAIL: &str = "Detalle del error de CanopyDB: ";

fn unreadable(folder: &Path) -> String {
    format!(
        "No se pudo leer el conjunto de datos en {}.",
        folder.display()
    )
}

/// The data file of the base: the largest file of `canopydb/`.
fn data_file(version: &Path) -> PathBuf {
    common::tree(&version.join("canopydb"))
        .into_iter()
        .map(|name| version.join("canopydb").join(name))
        .filter(|path| path.is_file())
        .max_by_key(|path| fs::metadata(path).expect("readable").len())
        .expect("a data file")
}

#[test]
fn a_damaged_base_is_reported_with_the_engine_detail_alone_and_in_a_batch() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let data = data_file(&fixture.version_dir());
    let mut file = OpenOptions::new()
        .write(true)
        .open(&data)
        .expect("writable");
    file.seek(SeekFrom::Start(0)).expect("seekable");
    file.write_all(&[0; 8192]).expect("written");
    drop(file);

    let ran = run(
        &APP,
        &fixture.output(),
        &["--where", "Primary Type=THEFT", "--count"],
    );
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    let lines: Vec<&str> = ran.stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].starts_with(DETAIL) && lines[0].len() > DETAIL.len(),
        "{lines:?}"
    );
    assert_eq!(lines[1], unreadable(&fixture.output()));

    let batch = fixture.dir.path().join("batch.txt");
    fs::write(&batch, "--where ID=10 --format csv --output never.csv\n").expect("written");
    let ran = run(
        &APP,
        &fixture.output(),
        &["--batch", &batch.to_string_lossy()],
    );
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    let lines: Vec<&str> = ran.stderr.lines().collect();
    assert_eq!(lines.len(), 2, "no header, no line prefix: {lines:?}");
    assert!(lines[0].starts_with(DETAIL), "{lines:?}");
    assert_eq!(lines[1], unreadable(&fixture.output()));
    assert!(!Path::new("never.csv").exists());
}

/// Tables whose every read fails as the engine would, with a detail line.
struct Failing {
    folder: PathBuf,
}

impl Failing {
    fn error(&self) -> ReadError {
        ReadError::with_detail(
            self.folder.clone(),
            "injected".to_owned(),
            format!("{DETAIL}I/O error at page 42"),
        )
    }
}

struct FailingCursor(ReadError);

impl KeyCursor for FailingCursor {
    fn seek(&mut self, _start: &[u8]) -> Result<bool, ReadError> {
        Err(self.0.clone())
    }

    fn advance(&mut self) -> Result<bool, ReadError> {
        Err(self.0.clone())
    }

    fn key(&self) -> &[u8] {
        unreachable!("never positioned")
    }
}

impl Tables for Failing {
    fn primary_value(&self, _key: &[u8]) -> Result<Option<Vec<u8>>, ReadError> {
        Err(self.error())
    }

    fn index_contains(&self, _kind: IndexKind, _key: &[u8]) -> Result<bool, ReadError> {
        Err(self.error())
    }

    fn index_cursor(&self, _kind: IndexKind) -> Box<dyn KeyCursor> {
        Box::new(FailingCursor(self.error()))
    }
}

fn open_failing(
    folder: &Path,
    _cancel: &dyn Fn() -> bool,
    _stderr: &mut dyn Write,
) -> Result<Box<dyn Source>, OpenError> {
    Ok(Box::new(TablesSource::new(
        folder.to_path_buf(),
        "dataset-20261022-000000-1".to_owned(),
        "2026-10-22T00:00:00+01:00".to_owned(),
        serde_json::Value::from("canopydb-2"),
        Failing {
            folder: folder.to_path_buf(),
        },
    )))
}

const FAILING: App = App {
    open: open_failing,
    ..APP
};

#[test]
fn a_read_that_fails_during_a_query_aborts_with_the_detail_and_no_output_file() {
    let dir = tempfile::tempdir().expect("a folder");
    let output = dir.path().join("thefts.csv");
    let output_text = output.to_string_lossy().into_owned();
    for query in [
        vec!["--where", "Primary Type=THEFT", "--count"],
        vec!["--where", "ID=10", "--format", "json"],
        vec![
            "--where",
            "Primary Type=THEFT",
            "--format",
            "csv",
            "--output",
            &output_text,
        ],
    ] {
        let ran = run(&FAILING, dir.path(), &query);
        assert_eq!(ran.code, 1, "{query:?}");
        assert_eq!(
            ran.stderr,
            format!("{DETAIL}I/O error at page 42\n{}\n", unreadable(dir.path())),
            "{query:?}"
        );
        assert!(!output.exists(), "{query:?}");
    }
    let batch = dir.path().join("batch.txt");
    fs::write(&batch, "--where \"Primary Type=THEFT\" --count\n").expect("written");
    let ran = run(&FAILING, dir.path(), &["--batch", &batch.to_string_lossy()]);
    assert_eq!(ran.code, 1);
    assert_eq!(
        ran.stderr,
        format!(
            "--- Consulta 1 de 1 (línea 1) ---\n{DETAIL}I/O error at page 42\n{}\n",
            unreadable(dir.path())
        )
    );
}

/// Read-only files on Windows: CanopyDB cannot open its internal files for writing, so the
/// dataset is rejected as an error of the engine and its files do not change.
#[cfg(windows)]
#[test]
fn a_base_with_read_only_files_is_rejected_as_an_engine_error() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let base = fixture.version_dir().join("canopydb");
    let files: Vec<PathBuf> = common::tree(&base)
        .into_iter()
        .map(|name| base.join(name))
        .filter(|path| path.is_file())
        .collect();
    let set_read_only = |read_only: bool| {
        for path in &files {
            let mut permissions = fs::metadata(path).expect("readable").permissions();
            permissions.set_readonly(read_only);
            fs::set_permissions(path, permissions).expect("permissions set");
        }
    };
    let before: Vec<Vec<u8>> = files
        .iter()
        .map(|path| fs::read(path).expect("readable"))
        .collect();
    set_read_only(true);
    let ran = run(
        &APP,
        &fixture.output(),
        &["--where", "Primary Type=THEFT", "--count"],
    );
    let after: Vec<Vec<u8>> = files
        .iter()
        .map(|path| fs::read(path).expect("readable"))
        .collect();
    set_read_only(false);
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    let lines: Vec<&str> = ran.stderr.lines().collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with(DETAIL), "{lines:?}");
    assert_eq!(lines[1], unreadable(&fixture.output()));
    assert!(before == after, "the files did not change");
}
