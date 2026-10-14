//! Sort folders left behind by a generator that was killed: each folder is named after the
//! process that owns it, and a later run discards the folders whose process has ended.

mod common;

use std::cell::RefCell;
use std::fs;
use std::num::NonZeroUsize;

use common::{Scenario, distinct_row, folder_entries, generate_ok};
use crime_generator::messages::stale_sort_dirs_discarded;
use crime_generator::primary::PrimaryEntry;
use crime_generator::sort::{
    ExternalSort, SORT_DIR_PREFIX, StaleSortDirs, discard_stale_sort_dirs, sort_dir_owner,
};

/// A process ID that the tests below declare ended.
const DEAD: u32 = 7_777;

/// Taken by every test that starts a process and by every test that needs a folder lock to be
/// free once this process releases it. Between `fork` and `exec` a child shares the open files of
/// the test process, so a lock released here can stay held by the child for a moment.
#[cfg(not(windows))]
static SPAWNS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(not(windows))]
fn no_spawns() -> std::sync::MutexGuard<'static, ()> {
    SPAWNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn start_sort(temp_dir: &std::path::Path) -> ExternalSort<PrimaryEntry> {
    let size = NonZeroUsize::new(10).expect("positive");
    ExternalSort::new(temp_dir, size, PrimaryEntry::compare).expect("the sort can start")
}

#[test]
fn a_sort_folder_is_named_after_the_process_that_owns_it() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let pid = std::process::id();

    let first = start_sort(dir.path());
    let second = start_sort(dir.path());

    let names = folder_entries(dir.path());
    assert_eq!(names.len(), 2, "one folder per sort: {names:?}");
    for name in &names {
        assert!(
            name.starts_with(&format!("{SORT_DIR_PREFIX}{pid}-")),
            "{name} names this process"
        );
        assert_eq!(sort_dir_owner(name), Some(pid), "{name}");
    }
    drop((first, second));
    assert_eq!(folder_entries(dir.path()), Vec::<String>::new());
}

#[test]
fn only_the_exact_shape_of_a_sort_folder_name_has_an_owner() {
    let owned = [
        ("crime-generator-sort-1-abcdef", 1),
        ("crime-generator-sort-48804-A1b2C3", 48_804),
        ("crime-generator-sort-4294967295-zzzzzz", u32::MAX),
    ];
    for (name, pid) in owned {
        assert_eq!(sort_dir_owner(name), Some(pid), "{name}");
    }

    let foreign = [
        // The shape before process IDs were part of the name: its owner is unknown.
        "crime-generator-sort-jknbKd",
        "crime-generator-sort-",
        "crime-generator-sort-1",
        "crime-generator-sort-1-",
        "crime-generator-sort--abcdef",
        "crime-generator-sort-0-abcdef",
        "crime-generator-sort-012-abcdef",
        "crime-generator-sort-4294967296-abcdef",
        "crime-generator-sort-+1-abcdef",
        "crime-generator-sort-1-abcde",
        "crime-generator-sort-1-abcdefg",
        "crime-generator-sort-1-abc-ef",
        "crime-generator-sort-1-abc.ef",
        "crime-generator-sort-1-abcdé",
        "crime-generator-sort-1-abcdef.bak",
        "crime-generator-sort-1-abcdef-1",
        "crime-generator-sort-١-abcdef",
        "Crime-generator-sort-1-abcdef",
        "xcrime-generator-sort-1-abcdef",
        "crime-generator-sort-1-abcdef/x",
    ];
    for name in foreign {
        assert_eq!(sort_dir_owner(name), None, "{name}");
    }
}

#[test]
fn only_sort_folders_of_ended_processes_are_discarded() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let pid = std::process::id();
    let create_dir = |name: &str| {
        fs::create_dir(dir.path().join(name)).expect("the fixture folder can be created");
    };
    create_dir(&format!("{SORT_DIR_PREFIX}{DEAD}-aaaaaa"));
    fs::write(
        dir.path()
            .join(format!("{SORT_DIR_PREFIX}{DEAD}-aaaaaa"))
            .join("segment-000000"),
        b"segment",
    )
    .expect("the fixture segment can be written");
    create_dir(&format!("{SORT_DIR_PREFIX}{DEAD}-bbbbbb"));
    let live = format!("{SORT_DIR_PREFIX}{pid}-cccccc");
    create_dir(&live);
    // Right name, but a file: a sort never creates one.
    let file = format!("{SORT_DIR_PREFIX}{DEAD}-dddddd");
    fs::write(dir.path().join(&file), b"not a sort folder").expect("the fixture can be written");
    let near_misses = [
        format!("{SORT_DIR_PREFIX}{DEAD}"),
        format!("{SORT_DIR_PREFIX}{DEAD}-eeeeee.bak"),
        format!("{SORT_DIR_PREFIX}xY12Ab"),
        "notes".to_owned(),
    ];
    for name in &near_misses {
        create_dir(name);
    }
    let asked = RefCell::new(Vec::new());

    let outcome = discard_stale_sort_dirs(dir.path(), |owner| {
        asked.borrow_mut().push(owner);
        owner != DEAD
    });

    assert_eq!(
        outcome,
        StaleSortDirs {
            removed: 2,
            kept: Vec::new()
        }
    );
    let mut expected: Vec<String> = near_misses.to_vec();
    expected.extend([live, file]);
    expected.sort();
    assert_eq!(folder_entries(dir.path()), expected);
    let mut asked = asked.into_inner();
    asked.sort_unstable();
    let mut sort_folders = vec![DEAD, DEAD, pid];
    sort_folders.sort_unstable();
    assert_eq!(
        asked, sort_folders,
        "only the owners of folders with the exact shape are looked up"
    );
}

#[test]
fn a_missing_temp_folder_discards_nothing() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");

    let outcome = discard_stale_sort_dirs(&dir.path().join("missing"), |_| false);

    assert_eq!(outcome, StaleSortDirs::default());
}

#[cfg(any(windows, target_os = "linux"))]
mod liveness {
    use std::fs;
    use std::path::Path;
    use std::process::{Child, Command};

    use super::*;
    use crime_generator::messages::stale_sort_dir_kept;
    use crime_generator::process::is_alive;

    /// A process that has already ended, whose ID cannot go to another process during the test.
    /// Windows keeps the ID while the handle is held. On Linux the ended child is left unreaped,
    /// a zombie that keeps its ID until it is waited for when this value is dropped.
    struct EndedProcess(Child);

    impl EndedProcess {
        #[cfg(windows)]
        fn new() -> Self {
            let mut child = Command::new("cmd")
                .args(["/C", "exit 0"])
                .spawn()
                .expect("cmd can be started");
            child.wait().expect("cmd ends");
            Self(child)
        }

        #[cfg(not(windows))]
        fn new() -> Self {
            let spawning = no_spawns();
            let child = Command::new("true").spawn().expect("true can be started");
            let stat = format!("/proc/{}/stat", child.id());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !fs::read_to_string(&stat).is_ok_and(|line| line.contains(") Z ")) {
                assert!(std::time::Instant::now() < deadline, "true ends");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            drop(spawning);
            Self(child)
        }

        fn id(&self) -> u32 {
            self.0.id()
        }
    }

    #[cfg(not(windows))]
    impl Drop for EndedProcess {
        fn drop(&mut self) {
            let _ = self.0.wait();
        }
    }

    /// Keeps `folder` from being deleted while it is alive.
    struct HeldAgainstRemoval {
        #[cfg(windows)]
        _file: fs::File,
        #[cfg(not(windows))]
        folder: std::path::PathBuf,
    }

    impl HeldAgainstRemoval {
        /// `None` when nothing can keep the folder from being deleted, as for the Linux superuser.
        #[cfg(windows)]
        fn new(folder: &Path) -> Option<Self> {
            use std::os::windows::fs::OpenOptionsExt;

            // Open without any sharing: nobody can delete the file while it is held.
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .share_mode(0)
                .open(folder.join("segment-000000"))
                .expect("the fixture segment can be created");
            Some(Self { _file: file })
        }

        /// `None` when nothing can keep the folder from being deleted, as for the Linux superuser.
        #[cfg(not(windows))]
        fn new(folder: &Path) -> Option<Self> {
            use std::os::unix::fs::PermissionsExt;

            // A file in a folder without write permission cannot be deleted, nor can the folder.
            fs::write(folder.join("segment-000000"), b"segment").expect("the segment is written");
            fs::set_permissions(folder, fs::Permissions::from_mode(0o555))
                .expect("the folder can be made read-only");
            let held = Self {
                folder: folder.to_path_buf(),
            };
            match fs::write(folder.join("probe"), b"") {
                Ok(()) => None,
                Err(_) => Some(held),
            }
        }
    }

    #[cfg(not(windows))]
    impl Drop for HeldAgainstRemoval {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;

            let _ = fs::set_permissions(&self.folder, fs::Permissions::from_mode(0o755));
        }
    }

    #[test]
    fn this_process_is_alive_and_an_ended_one_is_not() {
        let ended = EndedProcess::new();

        assert!(is_alive(std::process::id()));
        assert!(!is_alive(ended.id()));
    }

    #[test]
    fn a_folder_that_cannot_be_removed_is_kept_and_named_until_a_later_attempt() {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let stale = dir.path().join(format!("{SORT_DIR_PREFIX}{DEAD}-ffffff"));
        fs::create_dir(&stale).expect("the fixture folder can be created");
        #[cfg(not(windows))]
        let _no_spawns = no_spawns();
        let Some(held) = HeldAgainstRemoval::new(&stale) else {
            eprintln!("skipped: this user can delete any folder");
            return;
        };

        let first = discard_stale_sort_dirs(dir.path(), |owner| owner != DEAD);
        drop(held);
        let second = discard_stale_sort_dirs(dir.path(), |owner| owner != DEAD);

        assert_eq!(
            first,
            StaleSortDirs {
                removed: 0,
                kept: vec![stale.clone()]
            }
        );
        assert_eq!(
            second,
            StaleSortDirs {
                removed: 1,
                kept: Vec::new()
            }
        );
        assert!(!stale.exists());
        assert_eq!(
            stale_sort_dir_kept(&stale),
            format!(
                "Aviso: no se ha podido eliminar la carpeta temporal {}; se reintentará en la \
                 próxima generación.",
                stale.display()
            )
        );
    }

    #[test]
    fn a_run_discards_the_sort_folders_of_a_killed_generator_and_says_so() {
        let scenario = Scenario::new(&[distinct_row(2), distinct_row(1)]);
        let ended = EndedProcess::new();
        let stale = scenario
            .temp_dir
            .join(format!("{SORT_DIR_PREFIX}{}-a1B2c3", ended.id()));
        fs::create_dir(&stale).expect("the fixture folder can be created");
        fs::write(stale.join("segment-000000"), b"segment").expect("the segment can be written");
        let live = format!("{SORT_DIR_PREFIX}{}-zzzzzz", std::process::id());
        fs::create_dir(scenario.temp_dir.join(&live)).expect("the fixture can be created");

        let (summary, log) = generate_ok(&scenario.config("out"));

        assert_eq!(summary.counts.valid, 2);
        assert_eq!(
            folder_entries(&scenario.temp_dir),
            [live],
            "the folder of the ended process is gone; the one of a live process stays"
        );
        assert_eq!(
            log.matches(&stale_sort_dirs_discarded()).count(),
            1,
            "{log}"
        );
        drop(ended);
    }
}

#[test]
fn a_run_without_stale_sort_folders_does_not_mention_them() {
    let scenario = Scenario::new(&[distinct_row(1)]);
    let live = format!("{SORT_DIR_PREFIX}{}-zzzzzz", std::process::id());
    fs::create_dir(scenario.temp_dir.join(&live)).expect("the fixture can be created");

    let (_, log) = generate_ok(&scenario.config("out"));

    assert!(!log.contains(&stale_sort_dirs_discarded()), "{log}");
    assert_eq!(folder_entries(&scenario.temp_dir), [live]);
}

/// On Unix a running sort also holds an advisory lock on its folder, so a run never deletes it
/// even when its owner looks ended from here (another PID namespace, or `/proc` hidden).
#[cfg(not(windows))]
mod folder_lock {
    use std::fs::{self, File, TryLockError};

    use super::*;

    fn is_locked(folder: &std::path::Path) -> bool {
        let file = File::open(folder).expect("the folder can be opened");
        matches!(file.try_lock(), Err(TryLockError::WouldBlock))
    }

    #[test]
    fn a_running_sort_holds_its_folder_until_it_is_dropped() {
        let dir = tempfile::tempdir().expect("a temporary directory can be created");

        let sort = start_sort(dir.path());
        let names = folder_entries(dir.path());
        let folder = dir.path().join(&names[0]);

        assert!(is_locked(&folder), "held while the sort runs");
        let sorted = sort.finish().expect("the sort finishes");
        assert!(is_locked(&folder), "held while the merged stream is read");
        drop(sorted);
        assert!(!folder.exists(), "deleted with the stream");
    }

    #[test]
    fn a_held_folder_is_left_alone_even_when_its_owner_looks_ended() {
        let _no_spawns = no_spawns();
        let dir = tempfile::tempdir().expect("a temporary directory can be created");
        let held = dir.path().join(format!("{SORT_DIR_PREFIX}{DEAD}-h0ld3r"));
        fs::create_dir(&held).expect("the fixture folder can be created");
        fs::write(held.join("segment-000000"), b"segment").expect("the segment can be written");
        let lock = File::open(&held).expect("the folder can be opened");
        lock.lock().expect("the folder can be locked");

        let first = discard_stale_sort_dirs(dir.path(), |_| false);
        drop(lock);
        let second = discard_stale_sort_dirs(dir.path(), |_| false);

        assert_eq!(
            first,
            StaleSortDirs::default(),
            "neither deleted nor reported"
        );
        assert_eq!(
            second,
            StaleSortDirs {
                removed: 1,
                kept: Vec::new()
            }
        );
        assert!(!held.exists());
    }
}
