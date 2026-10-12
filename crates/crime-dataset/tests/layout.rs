//! The output folder layout: `CURRENT` names the active version folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crime_dataset::{
    CURRENT_FILE_NAME, CurrentError, VERSION_DIR_PREFIX, is_version_dir_name, resolve_current,
};

/// A fresh folder under the system temporary folder, deleted on drop. (This crate has no
/// dev-dependencies, so the tests do not use `tempfile`.)
struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "crime-dataset-layout-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        fs::create_dir(&path).expect("a test folder can be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `CURRENT` with `content`.
    fn current(&self, content: &[u8]) {
        fs::write(self.0.join(CURRENT_FILE_NAME), content).expect("CURRENT can be written");
    }

    /// Creates a version folder named `name`.
    fn version(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("the version folder can be created");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_contract_names_are_stable() {
    assert_eq!(CURRENT_FILE_NAME, "CURRENT");
    assert_eq!(VERSION_DIR_PREFIX, "dataset-");
}

#[test]
fn version_folder_names_are_the_prefix_and_safe_characters_only() {
    for valid in [
        "dataset-20261012-101500-4242",
        "dataset-20261012-101500-4242-1",
        "dataset-x",
        "dataset-ABC-123",
    ] {
        assert!(is_version_dir_name(valid), "{valid:?} is valid");
    }
    for invalid in [
        "",
        "dataset-",
        "dataset",
        "Dataset-1",
        "other-1",
        "dataset-1/..",
        "dataset-1\\x",
        "dataset-..",
        "dataset-1.tmp",
        "dataset-1 2",
        "dataset-C:x",
        "../dataset-1",
        "dataset-1\n",
        "dataset-ñ",
    ] {
        assert!(!is_version_dir_name(invalid), "{invalid:?} is not valid");
    }
}

#[test]
fn current_resolves_to_the_version_folder_it_names() {
    let dir = TestDir::new();
    let version = dir.version("dataset-20261012-101500-4242");

    for content in [
        "dataset-20261012-101500-4242\n",
        "dataset-20261012-101500-4242\r\n",
        "dataset-20261012-101500-4242",
    ] {
        dir.current(content.as_bytes());

        let resolved = resolve_current(dir.path()).expect("CURRENT names an existing folder");

        assert_eq!(resolved, version, "{content:?}");
    }
}

#[test]
fn a_folder_without_current_holds_no_dataset() {
    let dir = TestDir::new();
    dir.version("dataset-1");

    let error = resolve_current(dir.path()).expect_err("there is no CURRENT");

    assert!(matches!(error, CurrentError::NoDataset), "{error:?}");
}

#[test]
fn current_must_hold_exactly_one_valid_name() {
    let dir = TestDir::new();
    dir.version("dataset-1");
    dir.version("dataset-2");

    for content in [
        &b""[..],
        b"\n",
        b"dataset-1\n\n",
        b"dataset-1\ndataset-2\n",
        b" dataset-1\n",
        b"../dataset-1\n",
        b"dataset-1/../dataset-2\n",
        b"..\\dataset-1\n",
        b"C:\\dataset-1\n",
        b"other\n",
        b"dataset-\xff\n",
    ] {
        dir.current(content);

        let error = resolve_current(dir.path()).expect_err("an invalid CURRENT");

        assert!(
            matches!(error, CurrentError::InvalidName(_)),
            "{:?}: {error:?}",
            String::from_utf8_lossy(content)
        );
    }
}

#[test]
fn current_naming_a_missing_folder_or_a_file_is_refused() {
    let dir = TestDir::new();
    fs::write(dir.path().join("dataset-file"), b"not a folder").expect("fixture");

    for name in ["dataset-missing", "dataset-file"] {
        dir.current(format!("{name}\n").as_bytes());

        let error = resolve_current(dir.path()).expect_err("no such version folder");

        match error {
            CurrentError::MissingVersion(path) => assert_eq!(path, dir.path().join(name)),
            other => panic!("{name}: expected a missing version, got {other:?}"),
        }
    }
}

#[test]
fn errors_explain_themselves() {
    assert_eq!(
        CurrentError::NoDataset.to_string(),
        "there is no CURRENT file"
    );
    assert_eq!(
        CurrentError::InvalidName("x/y".to_owned()).to_string(),
        "CURRENT does not name a version folder: \"x/y\""
    );
}
