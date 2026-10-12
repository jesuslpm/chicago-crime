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

/// Two version folder names as the generator writes them.
const VERSION: &str = "dataset-20261012-101500-4242";
const OTHER_VERSION: &str = "dataset-20261012-101501-4242";

#[test]
fn version_folder_names_have_exactly_the_shape_the_generator_writes() {
    for valid in [
        VERSION,
        OTHER_VERSION,
        // The number appended when the folder of that second already exists.
        "dataset-20261012-101500-4242-1",
        "dataset-20261012-101500-4242-100",
        // Process IDs have any number of digits.
        "dataset-20261012-101500-7",
        "dataset-20261012-101500-4294967295",
    ] {
        assert!(is_version_dir_name(valid), "{valid:?} is valid");
    }
    for invalid in [
        "",
        "dataset-",
        "dataset",
        "Dataset-20261012-101500-4242",
        "other-20261012-101500-4242",
        // Folders a user may keep next to a dataset: never versions, so never deleted.
        "dataset-backup",
        "dataset-2024",
        "dataset-20261012",
        "dataset-1",
        "dataset-x",
        "dataset-ABC-123",
        // Missing or extra parts.
        "dataset-20261012-101500",
        "dataset-20261012-101500-",
        "dataset-20261012--4242",
        "dataset-20261012-101500-4242-",
        "dataset-20261012-101500-4242-1-2",
        // Wrong widths for the date or the time.
        "dataset-2026101-101500-4242",
        "dataset-202610120-101500-4242",
        "dataset-20261012-10150-4242",
        "dataset-20261012-1015000-4242",
        // Anything but ASCII digits in a numeric part.
        "dataset-2026101x-101500-4242",
        "dataset-20261012-10150x-4242",
        "dataset-20261012-101500-42a2",
        "dataset-20261012-101500-4242-a",
        "dataset-20261012-101500-+4242",
        "dataset-20261012-101500-٤٢",
        "dataset_20261012-101500-4242",
        "dataset-20261012_101500-4242",
        // Paths and decorated names.
        "dataset-20261012-101500-4242/..",
        "dataset-20261012-101500-4242\\x",
        "../dataset-20261012-101500-4242",
        "dataset-20261012-101500-4242.tmp",
        "dataset-20261012-101500-4242 ",
        " dataset-20261012-101500-4242",
        "dataset-20261012-101500-4242\n",
        "C:dataset-20261012-101500-4242",
    ] {
        assert!(!is_version_dir_name(invalid), "{invalid:?} is not valid");
    }
}

#[test]
fn current_resolves_to_the_version_folder_it_names() {
    let dir = TestDir::new();
    let version = dir.version(VERSION);

    for content in [
        format!("{VERSION}\n"),
        format!("{VERSION}\r\n"),
        VERSION.to_owned(),
    ] {
        dir.current(content.as_bytes());

        let resolved = resolve_current(dir.path()).expect("CURRENT names an existing folder");

        assert_eq!(resolved, version, "{content:?}");
    }
}

#[test]
fn a_folder_without_current_holds_no_dataset() {
    let dir = TestDir::new();
    dir.version(VERSION);

    let error = resolve_current(dir.path()).expect_err("there is no CURRENT");

    assert!(matches!(error, CurrentError::NoDataset), "{error:?}");
}

#[test]
fn current_must_hold_exactly_one_valid_name() {
    let dir = TestDir::new();
    dir.version(VERSION);
    dir.version(OTHER_VERSION);
    // A user folder with the version prefix is not a version, even when it exists.
    dir.version("dataset-backup");

    let mut contents: Vec<Vec<u8>> = [
        String::new(),
        "\n".to_owned(),
        format!("{VERSION}\n\n"),
        format!("{VERSION}\n{OTHER_VERSION}\n"),
        format!(" {VERSION}\n"),
        format!("../{VERSION}\n"),
        format!("{VERSION}/../{OTHER_VERSION}\n"),
        format!("..\\{VERSION}\n"),
        format!("C:\\{VERSION}\n"),
        "other\n".to_owned(),
        "dataset-backup\n".to_owned(),
    ]
    .into_iter()
    .map(String::into_bytes)
    .collect();
    contents.push(b"dataset-\xff\n".to_vec());

    for content in contents {
        dir.current(&content);

        let error = resolve_current(dir.path()).expect_err("an invalid CURRENT");

        assert!(
            matches!(error, CurrentError::InvalidName(_)),
            "{:?}: {error:?}",
            String::from_utf8_lossy(&content)
        );
    }
}

#[test]
fn current_naming_a_missing_folder_or_a_file_is_refused() {
    let dir = TestDir::new();
    fs::write(dir.path().join(OTHER_VERSION), b"not a folder").expect("fixture");

    for name in [VERSION, OTHER_VERSION] {
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
