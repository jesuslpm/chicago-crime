//! A version that is deleted before the query app opens it (decision KR7 of the
//! canopy-db-robustez PRD): the app reads `CURRENT` again and answers from the new version,
//! with no second warning and no longer wait.
//!
//! The race is reproduced through `open_resolving`: its resolver answers a stale version folder
//! first, as if the generator had replaced `CURRENT` and started deleting that folder right after
//! the app read it; `CURRENT` itself names the published version.

mod common;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use common::{CanopyFixture, fixture_rows};
use crime_canopy_query::open_resolving;
use crime_dataset::{CurrentError, resolve_current};

/// A resolver that answers `stale` the first time and the real `CURRENT` afterwards.
fn stale_first(stale: PathBuf) -> impl Fn(&Path) -> Result<PathBuf, CurrentError> {
    let calls = Cell::new(0);
    move |folder| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            Ok(stale.clone())
        } else {
            resolve_current(folder)
        }
    }
}

/// Copies the files of `from` into `to`, recursively.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn opened_version(fixture: &CanopyFixture, stale: PathBuf, wait: Duration) -> (String, String) {
    let mut stderr = Vec::new();
    let resolve = stale_first(stale);
    let source = open_resolving(&fixture.output(), &|| false, &mut stderr, wait, &resolve)
        .unwrap_or_else(|error| panic!("the new version opens: {error:?}"));
    (
        source.version().to_owned(),
        String::from_utf8(stderr).unwrap(),
    )
}

fn published_version(fixture: &CanopyFixture) -> String {
    fixture
        .version_dir()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_version_partly_deleted_after_reading_current_opens_the_new_one() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    // What `remove_dir_all` leaves midway: the folder and its manifest, no base.
    let stale = fixture.output().join("dataset-20261022-000000-1");
    fs::create_dir(&stale).unwrap();
    fs::copy(fixture.manifest_path(), stale.join("manifest.json")).unwrap();

    let (version, stderr) = opened_version(&fixture, stale, Duration::from_secs(5));

    assert_eq!(version, published_version(&fixture));
    assert_eq!(stderr, "");
}

#[test]
fn a_replaced_version_held_by_another_process_is_not_waited_for() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let stale = fixture.output().join("dataset-20261022-000000-1");
    copy_dir(&fixture.version_dir(), &stale);
    let _held = canopydb::Environment::new(stale.join("canopydb")).expect("the copy opens");

    let started = Instant::now();
    let (version, stderr) = opened_version(&fixture, stale, Duration::from_secs(10));

    assert!(
        started.elapsed() < Duration::from_secs(3),
        "it waited {:?}",
        started.elapsed()
    );
    assert_eq!(version, published_version(&fixture));
    assert_eq!(stderr, "");
}

#[test]
fn a_version_replaced_during_the_wait_ends_the_wait_and_opens_the_new_one() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let new_name = "dataset-20261022-000000-1";
    copy_dir(&fixture.version_dir(), &fixture.output().join(new_name));
    let _held =
        canopydb::Environment::new(fixture.version_dir().join("canopydb")).expect("the base opens");
    // The generator publishes the copy while the query app waits for the held base.
    let current = fixture.output().join("CURRENT");
    let publisher = thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        fs::write(current, format!("{new_name}\n")).unwrap();
    });

    let started = Instant::now();
    let mut stderr = Vec::new();
    let source = open_resolving(
        &fixture.output(),
        &|| false,
        &mut stderr,
        Duration::from_secs(10),
        &resolve_current,
    )
    .unwrap_or_else(|error| panic!("the new version opens: {error:?}"));
    publisher.join().unwrap();
    let stderr = String::from_utf8(stderr).unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(3),
        "it waited {:?}",
        started.elapsed()
    );
    assert_eq!(source.version(), new_name);
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("Aviso:"))
            .count(),
        1,
        "{stderr}"
    );
}

#[test]
fn an_incomplete_version_that_current_still_names_is_still_not_a_dataset() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let version_dir = fixture.version_dir();
    fs::remove_dir_all(version_dir.join("canopydb")).unwrap();
    let mut stderr = Vec::new();

    let opened = open_resolving(
        &fixture.output(),
        &|| false,
        &mut stderr,
        Duration::from_secs(1),
        &resolve_current,
    );

    assert!(matches!(
        opened,
        Err(crime_query::OpenError::NotFound { .. })
    ));
}
