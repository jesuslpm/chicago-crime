//! Opening a CanopyDB dataset (canopy-db-consultor PRD, flow steps 3 to 5), in the PRD's order:
//!
//! 1. resolve `CURRENT` to the active version folder, as the original query app does;
//! 2. read the manifest and check its `schema_version`, its shape and the CanopyDB version that
//!    wrote it (decisions KC3 and KC4), without opening the base;
//! 3. check that every file the manifest lists exists, so that opening the base creates nothing
//!    (CanopyDB would create a missing lock file);
//! 4. open the base, waiting while another process has it open (decision KC2): a warning once,
//!    then up to [`WAIT`], Ctrl+C honored;
//! 5. open the `crime` database with the options stored in it (opening with other options would
//!    rewrite them) and check that its six trees exist; nothing is ever created.
//!
//! As in the original query app, a version that vanished in between (a generator published
//! another one and deleted it) makes the opening start over, a few times at most. So does a
//! version that `CURRENT` no longer names and that looks incomplete or is held by another
//! process: a generator is deleting it (decision KR7 of the canopy-db-robustez PRD).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use canopydb::{DbOptions, EnvOptions, Environment};
use crime_dataset::canopy::{
    BASE_DIR_NAME, CANOPYDB_VERSION, DATABASE_NAME, SCHEMA_VERSION, table_names,
};
use crime_dataset::probe::{self, phase};
use crime_dataset::{CurrentError, MANIFEST_FILE_NAME, resolve_current};
use crime_query::{OpenError, Source, TablesSource};
use serde_json::Value;

use crate::manifest::{self, CanopyManifest, ManifestError};
use crate::messages;
use crate::tables::{CanopyTables, EngineFailure, guarded};

/// How long the query waits while another process has the base open (decision KC2).
pub const WAIT: Duration = Duration::from_secs(30);

/// The page cache of the base. CanopyDB reads its file with positional reads, so the cache is
/// private memory of the process, and it grows on demand: a count of a rare type peaks at about
/// 1 MiB. Measured on the reference CSV (2026-10-22), an 8 MiB cache took the `THEFT` count to
/// 9.9 MiB (ratio 9.9, AC-M8 asks at most 3), 2 MiB to 3.2 MiB (3.8) and 1 MiB to 2.2 MiB (2.3),
/// for 0.43, 0.43 and 0.51 s: the operating system's file cache still holds the pages (K8).
pub const PAGE_CACHE_BYTES: usize = 1 << 20;

/// How many times opening resolves `CURRENT` when the version it named vanished meanwhile.
const OPEN_ATTEMPTS: usize = 3;

/// The pause between two attempts to open a base another process has open.
const RETRY: Duration = Duration::from_millis(100);

/// Added to the wait before the rejection: well inside AC-M5's window of 30 to 35 s.
const DEADLINE_MARGIN: Duration = Duration::from_millis(100);

/// The longest a Ctrl+C waits to be noticed during the wait.
const CANCEL_POLL: Duration = Duration::from_millis(50);

/// The file in a database folder that holds its options.
const OPTIONS_FILE: &str = "OPTIONS";

/// Why one attempt to open failed.
enum Attempt {
    /// The version named by `CURRENT` disappeared before the base was open: try again.
    Vanished,
    /// The version looks incomplete or damaged: if `CURRENT` no longer names it, a generator is
    /// deleting it and the opening starts over (decision KR7 of the canopy-db-robustez PRD);
    /// otherwise this is the answer.
    Broken(OpenError),
    /// The answer, whatever `CURRENT` names now.
    Failed(OpenError),
}

impl From<OpenError> for Attempt {
    fn from(error: OpenError) -> Self {
        Attempt::Broken(error)
    }
}

/// Opens the CanopyDB dataset of the output folder `folder` (the [`crime_query::Opener`] of
/// the CanopyDB query app).
pub fn open(
    folder: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
) -> Result<Box<dyn Source>, OpenError> {
    open_waiting(folder, cancel, stderr, WAIT)
}

/// [`open`] with another wait than [`WAIT`]: the seam the tests use to check the wait quickly.
#[doc(hidden)]
pub fn open_waiting(
    folder: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
    wait: Duration,
) -> Result<Box<dyn Source>, OpenError> {
    open_resolving(folder, cancel, stderr, wait, &resolve_current)
}

/// [`open_waiting`] that finds the active version with `resolve` ([`resolve_current`] in the
/// app): the seam the tests use to reproduce a version deleted right after `CURRENT` was read.
#[doc(hidden)]
pub fn open_resolving(
    folder: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
    wait: Duration,
    resolve: &dyn Fn(&Path) -> Result<PathBuf, CurrentError>,
) -> Result<Box<dyn Source>, OpenError> {
    // When the wait started, so that a retry after a vanished version neither warns again nor
    // waits longer.
    let mut waiting_since = None;
    for _ in 0..OPEN_ATTEMPTS {
        let version_dir = match resolve(folder) {
            Ok(dir) => dir,
            Err(CurrentError::MissingVersion(_)) => continue,
            Err(CurrentError::Unreadable(error)) => {
                return Err(unreadable(folder, format!("CURRENT: {error}")));
            }
            Err(CurrentError::NoDataset | CurrentError::InvalidName(_)) => {
                return Err(not_found(folder));
            }
        };
        match open_version(
            folder,
            &version_dir,
            cancel,
            stderr,
            wait,
            &mut waiting_since,
        ) {
            Ok(source) => return Ok(Box::new(source)),
            Err(Attempt::Vanished) => {}
            Err(Attempt::Broken(_)) if replaced(folder, &version_dir) => {}
            Err(Attempt::Broken(error) | Attempt::Failed(error)) => return Err(error),
        }
    }
    Err(not_found(folder))
}

fn open_version(
    folder: &Path,
    version_dir: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
    wait: Duration,
    waiting_since: &mut Option<Instant>,
) -> Result<TablesSource<CanopyTables>, Attempt> {
    let version_dir = version_dir.to_path_buf();
    let manifest = read_manifest(folder, &version_dir)?;
    check_files(folder, &version_dir, &manifest)?;
    let base = version_dir.join(BASE_DIR_NAME);
    let stored = stored_options(folder, &version_dir, &base)?;
    let environment = open_environment(
        folder,
        &version_dir,
        &base,
        cancel,
        stderr,
        wait,
        waiting_since,
    )?;
    let engine = |failure: EngineFailure| Attempt::Broken(engine_failure(folder, &failure));
    let database = guarded(|| environment.get_database_with(DATABASE_NAME, stored))
        .map_err(engine)?
        .ok_or_else(|| Attempt::Broken(not_found(folder)))?;
    let transaction = guarded(|| database.begin_read()).map_err(engine)?;
    for tree in table_names() {
        let present =
            guarded(|| Ok(transaction.get_tree(tree.as_bytes())?.is_some())).map_err(engine)?;
        if !present {
            return Err(not_found(folder).into());
        }
    }
    // The version is open: from here a generator that deletes it does not change what this run
    // reads (decision KR7 of the canopy-db-robustez PRD).
    probe::mark(phase::OPENED);
    let version = version_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tables = CanopyTables::new(transaction, database, environment, folder.to_path_buf());
    Ok(TablesSource::new(
        folder.to_path_buf(),
        version,
        manifest.generated_at,
        Value::from(SCHEMA_VERSION),
        tables,
    ))
}

/// Reads the manifest and checks it (decisions KC3 and KC4).
fn read_manifest(folder: &Path, version_dir: &Path) -> Result<CanopyManifest, Attempt> {
    let bytes = match fs::read(version_dir.join(MANIFEST_FILE_NAME)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(Attempt::Vanished),
        Err(error) => {
            return Err(unreadable(folder, format!("{MANIFEST_FILE_NAME}: {error}")).into());
        }
    };
    let text = std::str::from_utf8(&bytes).map_err(|_| Attempt::Broken(not_found(folder)))?;
    let manifest = match manifest::parse(text) {
        Ok(manifest) => manifest,
        Err(ManifestError::Invalid) => return Err(not_found(folder).into()),
        Err(ManifestError::OtherSchema(digits)) => {
            let message = crime_query::messages::incompatible_schema(digits);
            return Err(Attempt::Failed(rejected(message)));
        }
    };
    if manifest.engine_version != CANOPYDB_VERSION || manifest.fork_commit.is_some() {
        let dataset =
            messages::version_text(&manifest.engine_version, manifest.fork_commit.as_deref());
        let own = messages::version_text(CANOPYDB_VERSION, None);
        return Err(Attempt::Failed(rejected(messages::other_version(
            &dataset, &own,
        ))));
    }
    Ok(manifest)
}

/// Checks that `canopydb/` and every file of the manifest exist, as files.
fn check_files(
    folder: &Path,
    version_dir: &Path,
    manifest: &CanopyManifest,
) -> Result<(), Attempt> {
    let complete = version_dir.join(BASE_DIR_NAME).is_dir()
        && manifest
            .files
            .iter()
            .all(|path| version_dir.join(path).is_file());
    if complete {
        Ok(())
    } else if version_dir.is_dir() {
        Err(not_found(folder).into())
    } else {
        Err(Attempt::Vanished)
    }
}

/// The options stored with the `crime` database, which CanopyDB replaces with the opener's:
/// passing them back leaves the base as it was.
fn stored_options(folder: &Path, version_dir: &Path, base: &Path) -> Result<DbOptions, Attempt> {
    // CanopyDB 0.2.5 keeps each database of an environment in a folder named
    // `<database>_<random id>_l`; its options are the `OPTIONS` file inside.
    let prefix = format!("{DATABASE_NAME}_");
    let entries = fs::read_dir(base).map_err(|error| {
        if version_dir.is_dir() {
            Attempt::Broken(unreadable(folder, format!("{BASE_DIR_NAME}: {error}")))
        } else {
            Attempt::Vanished
        }
    })?;
    let database_dir = entries
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with("_l"))
        })
        .ok_or_else(|| Attempt::Broken(not_found(folder)))?;
    let options_file = database_dir.join(OPTIONS_FILE);
    let bytes = fs::read(&options_file).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => Attempt::Broken(not_found(folder)),
        _ => Attempt::Broken(unreadable(folder, format!("{OPTIONS_FILE}: {error}"))),
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| Attempt::Broken(unreadable(folder, format!("{OPTIONS_FILE}: {error}"))))
}

/// Opens the environment of `base`, waiting while another process has it open (decision KC2):
/// the warning once, as soon as the first attempt finds it open, then a new attempt every
/// [`RETRY`] until `wait` ([`WAIT`] in the app) after the warning; Ctrl+C ends the wait.
fn open_environment(
    folder: &Path,
    version_dir: &Path,
    base: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
    wait: Duration,
    waiting_since: &mut Option<Instant>,
) -> Result<Environment, Attempt> {
    loop {
        // A version replaced during the wait (a generator published another one and is deleting
        // this one): start over on the new one right away (decision KR7), instead of waiting for
        // it or letting CanopyDB fail on, or create, a missing lock file.
        if !base.is_dir() || replaced(folder, version_dir) {
            return Err(Attempt::Vanished);
        }
        let mut options = EnvOptions::new(base);
        options.page_cache_size = PAGE_CACHE_BYTES;
        // One try of the lock: the wait is ours, so that it can warn and be cancelled.
        options.file_lock_timeout = Duration::ZERO;
        let failure = match guarded(|| Environment::with_options(options)) {
            Ok(environment) => return Ok(environment),
            Err(failure) => failure,
        };
        if !matches!(
            failure,
            EngineFailure::Error(canopydb::Error::EnvironmentLocked)
        ) {
            return Err(engine_failure(folder, &failure).into());
        }
        let since = *waiting_since.get_or_insert_with(|| {
            let _ = writeln!(stderr, "{}", messages::waiting(folder, wait));
            Instant::now()
        });
        // The margin keeps the rejection after the wait as whoever reads the warning sees it,
        // even when the warning reaches them late through a pipe (AC-M5: 30 to 35 s).
        let deadline = since + wait + DEADLINE_MARGIN;
        let next_attempt = Instant::now() + RETRY;
        loop {
            if cancel() {
                return Err(Attempt::Failed(OpenError::Cancelled));
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(Attempt::Failed(rejected(messages::still_open(folder))));
            }
            if now >= next_attempt {
                break;
            }
            thread::sleep(CANCEL_POLL.min(next_attempt - now).min(deadline - now));
        }
    }
}

/// Whether `CURRENT` no longer names `version_dir`: a generator published another version and may
/// be deleting this one (decision KR7 of the canopy-db-robustez PRD).
fn replaced(folder: &Path, version_dir: &Path) -> bool {
    !version_dir.is_dir() || !resolve_current(folder).is_ok_and(|dir| dir == version_dir)
}

/// An error or a panic of the engine (decision KC5): the detail line, then the original query
/// app's message for a dataset that cannot be read.
fn engine_failure(folder: &Path, failure: &EngineFailure) -> OpenError {
    rejected(format!(
        "{}\n{}",
        failure.detail(),
        crime_query::messages::dataset_unreadable(folder)
    ))
}

fn rejected(message: String) -> OpenError {
    OpenError::Rejected(message)
}

fn not_found(folder: &Path) -> OpenError {
    OpenError::NotFound {
        folder: folder.to_path_buf(),
    }
}

fn unreadable(folder: &Path, cause: String) -> OpenError {
    OpenError::Unreadable {
        folder: PathBuf::from(folder),
        cause,
    }
}
