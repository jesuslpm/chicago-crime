//! Publication of the dataset in the output folder (parent flow A, step 8).
//!
//! The layout belongs to the dataset contract (see [`crime_dataset::resolve_current`]): version
//! folders, and a `CURRENT` file that names the active one. Windows cannot atomically replace a
//! folder that has files in it, so a run never touches the published version; it:
//!
//! 1. holds the output lock ([`OutputLock`]) for the whole run, so two generators never share an
//!    output folder;
//! 2. inspects the folder ([`OutputFolder::open`]): anything that is not part of a dataset is
//!    refused and left alone (a folder is a version only if its name has exactly the shape this
//!    generator writes), an existing dataset needs the overwrite option, and version folders
//!    that `CURRENT` does not name, like a `CURRENT.tmp`, are leftovers of an interrupted run and
//!    are deleted;
//! 3. writes every table into a fresh version folder that nothing references yet
//!    ([`OutputFolder::create_version`]);
//! 4. publishes it ([`OutputFolder::publish`]): writes and syncs the manifest, writes and syncs
//!    `CURRENT.tmp`, and renames it over `CURRENT`. The rename is the commit point: a reader sees
//!    either the previous version or the new one, both complete. Only then is the previous
//!    version folder deleted; if that fails, the next run discards it as a leftover.
//!
//! An abort before the rename, a cancellation by the operator included, deletes the new version
//! folder when it is dropped, so `CURRENT` and the previous version stay exactly as they were. A process killed before the rename leaves an
//! unreferenced version folder that the next run deletes. Durability against power loss is not
//! required (parent flow A, step 8), so the output folder itself is never synced.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crime_dataset::{
    CURRENT_FILE_NAME, CurrentError, MANIFEST_FILE_NAME, Manifest, VERSION_DIR_PREFIX,
    is_version_dir_name, resolve_current,
};

use crate::Abort;
use crate::cancel::CancelFlag;
use crate::cli::local_timestamp;
use crate::fail::{self, FailPoint};
use crate::log::RunLog;
use crate::messages;

/// File in the output folder that a running generation holds open exclusively.
pub const LOCK_FILE_NAME: &str = ".generator.lock";

/// `CURRENT` is written under this name first and then renamed over `CURRENT`.
pub const CURRENT_TEMP_FILE_NAME: &str = "CURRENT.tmp";

/// Numbered names tried after the plain one when version folders collide (see
/// [`OutputFolder::create_version`]).
const MAX_VERSION_NAME_ATTEMPTS: u32 = 100;

/// Exclusive hold of an output folder for one run. It is released when dropped, and by the
/// operating system when the process ends, however it ends.
///
/// On Windows the lock file is opened without any sharing, so every other open fails while this
/// handle exists, and with delete-on-close, so the file disappears with the handle and a
/// published output folder holds only `CURRENT` and its version folder. Elsewhere, where the
/// generator is not verified (see the PRD), an advisory [`File::try_lock`] stands in and the lock
/// file stays.
#[derive(Debug)]
pub struct OutputLock {
    _file: File,
}

/// Why the output lock could not be taken.
#[derive(Debug)]
pub enum LockError {
    /// Another holder, normally another generation, has it.
    Busy,
    /// The lock file cannot be created or opened.
    Io(io::Error),
}

impl OutputLock {
    /// Takes the lock of the existing folder `dir`.
    pub fn acquire(dir: &Path) -> Result<Self, LockError> {
        open_exclusive(&dir.join(LOCK_FILE_NAME)).map(|file| Self { _file: file })
    }
}

#[cfg(windows)]
fn open_exclusive(path: &Path) -> Result<File, LockError> {
    use std::os::windows::fs::OpenOptionsExt;

    /// `FILE_FLAG_DELETE_ON_CLOSE` of the Windows API.
    const DELETE_ON_CLOSE: u32 = 0x0400_0000;
    /// `ERROR_SHARING_VIOLATION`: another handle has the file open without sharing it.
    const SHARING_VIOLATION: i32 = 32;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .custom_flags(DELETE_ON_CLOSE)
        .open(path)
        .map_err(|error| {
            if error.raw_os_error() == Some(SHARING_VIOLATION) {
                LockError::Busy
            } else {
                LockError::Io(error)
            }
        })
}

#[cfg(not(windows))]
fn open_exclusive(path: &Path) -> Result<File, LockError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(LockError::Io)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(fs::TryLockError::WouldBlock) => Err(LockError::Busy),
        Err(fs::TryLockError::Error(error)) => Err(LockError::Io(error)),
    }
}

/// The output folder, locked for one run.
pub struct OutputFolder {
    dir: PathBuf,
    /// Always `Some` until dropped: it is released before the folder may be removed.
    lock: Option<OutputLock>,
    /// The version folder that `CURRENT` names, if the folder holds a valid dataset.
    current: Option<String>,
    /// Whether this run created the output folder.
    created: bool,
    published: bool,
}

impl OutputFolder {
    /// Creates the folder if it is missing, takes its lock and checks what it holds, in this
    /// order: anything that is not part of a dataset is refused, and an existing dataset needs
    /// `overwrite`; both refusals change nothing. Then leftovers of interrupted runs are deleted.
    /// The log says once that leftovers were discarded if at least one was, and names each one
    /// that could not be deleted: the run goes on, the new version never uses its name, and the
    /// next run tries again.
    pub fn open(dir: &Path, overwrite: bool, log: &mut RunLog) -> Result<Self, Abort> {
        let unwritable = |error: io::Error| Abort::output_io(dir, &error);
        let created = !dir.exists();
        fs::create_dir_all(dir).map_err(unwritable)?;
        let mut folder = Self {
            dir: dir.to_path_buf(),
            lock: None,
            current: None,
            created,
            published: false,
        };
        folder.lock = Some(OutputLock::acquire(dir).map_err(|error| match error {
            LockError::Busy => Abort::OutputLocked {
                dir: dir.to_path_buf(),
            },
            LockError::Io(error) => unwritable(error),
        })?);

        let contents = Contents::read(dir).map_err(unwritable)?;
        if let Some(entry) = contents.foreign.into_iter().next() {
            return Err(Abort::ForeignContent {
                dir: dir.to_path_buf(),
                entry,
            });
        }
        if contents.has_current && !overwrite {
            return Err(Abort::DatasetExists);
        }
        folder.current = contents.current;
        let mut discarded = false;
        let mut kept = Vec::new();
        for leftover in contents.leftovers {
            let path = dir.join(leftover);
            match fail::check(FailPoint::LeftoverRemoval).and_then(|()| remove_entry(&path)) {
                Ok(()) => discarded = true,
                Err(_) => kept.push(path),
            }
        }
        if discarded {
            log.line(&messages::leftovers_discarded())?;
        }
        for path in kept {
            log.line(&messages::leftover_kept(&path))?;
        }
        Ok(folder)
    }

    /// Creates a new, empty version folder that nothing references yet, named by
    /// [`version_dir_name`] after the local time and this process. If that folder exists (a run
    /// of the same process published a version in the same second, or a leftover with that name
    /// could not be deleted), a number is appended: an existing folder is never reused.
    pub fn create_version(&self) -> io::Result<NewVersion> {
        let timestamp = local_timestamp();
        let process_id = std::process::id();
        let mut attempt = 0;
        loop {
            let name = version_dir_name(&timestamp, process_id, attempt);
            let dir = self.dir.join(&name);
            match fs::create_dir(&dir) {
                Ok(()) => {
                    return Ok(NewVersion {
                        dir,
                        name,
                        published: false,
                    });
                }
                Err(error)
                    if error.kind() == io::ErrorKind::AlreadyExists
                        && attempt < MAX_VERSION_NAME_ATTEMPTS =>
                {
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Publishes `version`, whose tables are complete and synced, with its `manifest` (see the
    /// module documentation for the steps).
    ///
    /// Every failure before `CURRENT` is replaced, including a cancellation raised in `cancel`,
    /// leaves `CURRENT` untouched, and `version` is deleted when it drops. Once `CURRENT` is
    /// replaced the run has succeeded: the previous version folder is deleted if possible, and a
    /// failure there is only logged, best effort.
    pub fn publish(
        mut self,
        mut version: NewVersion,
        manifest: &Manifest,
        log: &mut RunLog,
        cancel: &CancelFlag,
    ) -> Result<Published, Abort> {
        let unwritable = |error: io::Error| Abort::output_io(&self.dir, &error);
        let mut json = manifest.to_json();
        json.push('\n');
        write_synced(&version.dir.join(MANIFEST_FILE_NAME), json.as_bytes()).map_err(unwritable)?;
        self.switch_current(&version.name, cancel)?;
        version.published = true;
        self.published = true;

        if let Some(previous) = self.current.take() {
            let previous = self.dir.join(previous);
            let removed = fail::check(FailPoint::PreviousVersionRemoval)
                .and_then(|()| fs::remove_dir_all(&previous));
            if removed.is_err() {
                let _ = log.line(&messages::previous_version_kept(&previous));
            }
        }
        Ok(Published {
            dir: version.dir.clone(),
            manifest_size_bytes: json.len() as u64,
        })
    }

    /// Points `CURRENT` at the version folder `name`: the new content is written and synced
    /// under a temporary name and renamed over `CURRENT`, which replaces it in one step on the
    /// same volume. The cancellation flag is checked one last time right before the rename, the
    /// commit point. On failure the temporary file is removed and `CURRENT` is unchanged.
    fn switch_current(&self, name: &str, cancel: &CancelFlag) -> Result<(), Abort> {
        let unwritable = |error: io::Error| Abort::output_io(&self.dir, &error);
        let temp = self.dir.join(CURRENT_TEMP_FILE_NAME);
        let switched = write_synced(&temp, format!("{name}\n").as_bytes())
            .and_then(|()| fail::check(FailPoint::BeforeSwitch))
            .map_err(unwritable)
            .and_then(|()| cancel.check(FailPoint::CancelBeforeSwitch))
            .and_then(|()| fs::rename(&temp, self.dir.join(CURRENT_FILE_NAME)).map_err(unwritable));
        if switched.is_err() {
            let _ = fs::remove_file(&temp);
        }
        switched
    }
}

impl Drop for OutputFolder {
    fn drop(&mut self) {
        // The lock goes first: on Windows its file disappears with it.
        drop(self.lock.take());
        if self.created && !self.published {
            // Removes the folder only if it is empty, that is, as missing as this run found it.
            let _ = fs::remove_dir(&self.dir);
        }
    }
}

/// A version that `CURRENT` now names (see [`OutputFolder::publish`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// The version folder.
    pub dir: PathBuf,
    /// Size of the manifest file written into it.
    pub manifest_size_bytes: u64,
}

/// Name of a new version folder: `dataset-<timestamp>-<process id>` for the first `attempt` (0)
/// and `-<attempt>` appended for the next ones. With a timestamp from [`local_timestamp`]
/// (`YYYYMMDD-HHMMSS`), these are exactly the names that [`is_version_dir_name`] accepts.
pub fn version_dir_name(timestamp: &str, process_id: u32, attempt: u32) -> String {
    let name = format!("{VERSION_DIR_PREFIX}{timestamp}-{process_id}");
    if attempt == 0 {
        name
    } else {
        format!("{name}-{attempt}")
    }
}

/// A version folder being written. Unless it is published, dropping it deletes the folder and
/// everything in it, so its tables must be closed first.
pub struct NewVersion {
    dir: PathBuf,
    name: String,
    published: bool,
}

impl NewVersion {
    pub fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for NewVersion {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// What an output folder holds, classified by name and type.
struct Contents {
    /// Whether there is a `CURRENT` file, valid or not.
    has_current: bool,
    /// The version folder that a valid `CURRENT` names.
    current: Option<String>,
    /// Ours, but no longer needed: version folders that `CURRENT` does not name, and
    /// `CURRENT.tmp`; sorted by name.
    leftovers: Vec<String>,
    /// Everything else, sorted by name, including folders that only start like a version folder
    /// (see [`is_version_dir_name`]), such as `dataset-backup`. The lock file is neither.
    foreign: Vec<String>,
}

impl Contents {
    fn read(dir: &Path) -> io::Result<Self> {
        let mut has_current = false;
        let mut versions = Vec::new();
        let mut leftovers = Vec::new();
        let mut foreign = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            // Only names that are not Unicode change here, and none of ours is such a name.
            let name = entry.file_name().to_string_lossy().into_owned();
            match name.as_str() {
                LOCK_FILE_NAME if file_type.is_file() => {}
                CURRENT_FILE_NAME if file_type.is_file() => has_current = true,
                CURRENT_TEMP_FILE_NAME if file_type.is_file() => leftovers.push(name),
                _ if file_type.is_dir() && is_version_dir_name(&name) => versions.push(name),
                _ => foreign.push(name),
            }
        }
        let current = if has_current {
            match resolve_current(dir) {
                Ok(version) => version
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
                // A `CURRENT` that cannot be read right now may still name a valid version:
                // nothing is deleted on a guess.
                Err(CurrentError::Unreadable(error)) => return Err(error),
                // A `CURRENT` that names no version: no folder is the dataset any more.
                Err(_) => None,
            }
        } else {
            None
        };
        leftovers.extend(
            versions
                .into_iter()
                .filter(|name| Some(name) != current.as_ref()),
        );
        leftovers.sort();
        foreign.sort();
        Ok(Self {
            has_current,
            current,
            leftovers,
            foreign,
        })
    }
}

/// Deletes a leftover file or folder.
fn remove_entry(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Writes `bytes` as the whole content of `path` and syncs it to disk before returning, so a
/// failure of the last write is reported here.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}
