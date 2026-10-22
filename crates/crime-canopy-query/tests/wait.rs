//! The wait while another process has the base open (decision KC2, AC-M5 in small): the warning
//! once, the rejection when the wait ends, a normal answer when the base is released, and Ctrl+C.
//!
//! The base is held by an environment opened in the test itself: CanopyDB's lock is per open file
//! (`LockFileEx` on Windows, `flock` on Linux), so a second opening in the same process waits as
//! one in another process would. The wait is shortened through `open_waiting`; the app's own is
//! [`crime_canopy_query::WAIT`].

mod common;

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{CanopyFixture, fixture_rows, run, run_cancelled};
use crime_canopy_query::{APP, WAIT, open_waiting};
use crime_query::{App, OpenError, Source};

const QUERY: &[&str] = &["--where", "Primary Type=THEFT", "--count"];

/// The CanopyDB query app with a wait of one second.
fn open_one_second(
    folder: &Path,
    cancel: &dyn Fn() -> bool,
    stderr: &mut dyn Write,
) -> Result<Box<dyn Source>, OpenError> {
    open_waiting(folder, cancel, stderr, Duration::from_secs(1))
}

const ONE_SECOND: App = App {
    open: open_one_second,
    ..APP
};

/// Holds the base of `fixture` open, as another process would.
fn hold(fixture: &CanopyFixture) -> canopydb::Environment {
    canopydb::Environment::new(fixture.version_dir().join("canopydb")).expect("the base opens")
}

fn warning(folder: &Path, seconds: u64) -> String {
    format!(
        "Aviso: otro proceso tiene abierto el conjunto de datos en {}; se esperará hasta {seconds} s.",
        folder.display()
    )
}

#[test]
fn the_app_waits_thirty_seconds() {
    assert_eq!(WAIT, Duration::from_secs(30));
}

#[test]
fn a_base_still_open_after_the_wait_is_rejected_with_its_message() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let _held = hold(&fixture);
    let started = Instant::now();
    let ran = run(&ONE_SECOND, &fixture.output(), QUERY);
    let elapsed = started.elapsed();
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    assert_eq!(
        ran.stderr_lines(),
        [
            warning(&fixture.output(), 1),
            format!(
                "El conjunto de datos en {} sigue abierto por otro proceso. Inténtalo de nuevo \
                 cuando termine.",
                fixture.output().display()
            ),
        ]
    );
    assert!(
        elapsed >= Duration::from_secs(1) && elapsed < Duration::from_secs(3),
        "{elapsed:?}"
    );
}

#[test]
fn a_base_released_during_the_wait_answers_after_one_warning() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let held = hold(&fixture);
    let (sender, receiver) = mpsc::channel();
    let releaser = thread::spawn(move || {
        let _: () = receiver.recv().expect("the signal");
        thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    let started = AtomicBool::new(false);
    // The query signals the releaser from its first cancellation check, during the wait.
    let cancel = || {
        if !started.swap(true, Ordering::Relaxed) {
            sender.send(()).expect("the releaser listens");
        }
        false
    };
    let mut stderr = Vec::new();
    let mut opened = None;
    let result = open_waiting(
        &fixture.output(),
        &cancel,
        &mut stderr,
        Duration::from_secs(10),
    );
    if let Ok(source) = result {
        opened = Some(source.version().to_owned());
    }
    releaser.join().expect("the releaser ends");
    assert!(opened.is_some(), "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(
        String::from_utf8(stderr).expect("UTF-8"),
        warning(&fixture.output(), 10) + "\n"
    );
    let ran = run(&APP, &fixture.output(), QUERY);
    assert_eq!((ran.code, ran.stdout.as_str()), (0, "3\n"));
}

#[test]
fn ctrl_c_during_the_wait_cancels_within_a_second() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let _held = hold(&fixture);
    let started = Instant::now();
    let cancel = || started.elapsed() > Duration::from_millis(200);
    let ran = run_cancelled(&APP, &fixture.output(), QUERY, &cancel);
    let elapsed = started.elapsed();
    assert_eq!((ran.code, ran.stdout.as_str()), (130, ""));
    assert_eq!(
        ran.stderr_lines(),
        [
            warning(&fixture.output(), 30),
            "Consulta cancelada.".to_owned()
        ]
    );
    assert!(elapsed < Duration::from_millis(1_200), "{elapsed:?}");
}

#[test]
fn a_batch_waits_before_its_first_header() {
    let fixture = CanopyFixture::with_rows(&fixture_rows());
    let _held = hold(&fixture);
    let batch = fixture.dir.path().join("batch.txt");
    std::fs::write(&batch, "--where \"Primary Type=THEFT\" --count\n").expect("written");
    let ran = run(
        &ONE_SECOND,
        &fixture.output(),
        &["--batch", &batch.to_string_lossy()],
    );
    assert_eq!((ran.code, ran.stdout.as_str()), (1, ""));
    let lines = ran.stderr_lines();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], warning(&fixture.output(), 1));
    assert!(
        lines[1].starts_with("El conjunto de datos en "),
        "{lines:?}"
    );
}
