//! The output file (decision C8, AC-C14, AC-C15), cancellation (decision C6) and failed writes
//! on stdout: the binary end to end, and the library's [`crime_query::run`] where a test must
//! raise the cancellation or fail stdout.

mod common;

use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{Fixture, fixture_rows, many_rows};
use crime_query::cli::{self, Options, Parsed};
use crime_query::exit_code;

fn query_on(dataset: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-query"));
    command.arg("--dataset").arg(dataset).args(args);
    command.output().expect("the query app can be started")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn assert_code(output: &Output, code: u8) {
    assert_eq!(
        output.status.code(),
        Some(i32::from(code)),
        "{:?}",
        stderr_lines(output)
    );
}

/// The names in `folder`, sorted.
fn entries(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .expect("the folder can be listed")
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

/// An empty folder for output files.
fn results_folder() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory can be created")
}

fn unwritable(path: &Path) -> String {
    format!(
        "No se puede escribir el archivo de salida {}.",
        path.display()
    )
}

#[test]
fn the_file_holds_what_stdout_would_and_is_overwritten_without_asking() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let folder = results_folder();
    for format in ["csv", "json"] {
        let target = folder.path().join(format!("results.{format}"));
        let target_text = target.to_str().expect("UTF-8");
        for condition in ["Primary Type=theft", "ID=20"] {
            let args = ["--where", condition, "--format", format];
            let console = query_on(&fixture.output(), &args);
            assert_code(&console, exit_code::COMPLETED);
            assert!(!console.stdout.is_empty());

            let mut with_file = args.to_vec();
            with_file.extend(["--output", target_text]);
            let saved = query_on(&fixture.output(), &with_file);
            assert_code(&saved, exit_code::COMPLETED);
            assert!(saved.stdout.is_empty(), "{format}: {}", stdout(&saved));
            assert_eq!(
                fs::read(&target).expect("the file was written"),
                console.stdout,
                "{format} {condition}"
            );
            let lines = stderr_lines(&saved);
            assert_eq!(lines.len(), 1, "only the stats line: {lines:?}");
            assert!(lines[0].contains(" en "), "{lines:?}");
        }
    }
    assert_eq!(entries(folder.path()), ["results.csv", "results.json"]);
}

#[test]
fn a_rejected_query_creates_no_file_and_keeps_an_existing_one() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let folder = results_folder();
    let new = folder.path().join("new.csv");
    let old = folder.path().join("old.csv");
    fs::write(&old, "old content").expect("writable");
    let missing_dataset = folder.path().join("no-dataset");
    for target in [&new, &old] {
        let target_text = target.to_str().expect("UTF-8");
        for (dataset, condition) in [
            (fixture.output(), "Foo=1"),
            (missing_dataset.clone(), "ID=10"),
        ] {
            let output = query_on(
                &dataset,
                &[
                    "--where",
                    condition,
                    "--format",
                    "csv",
                    "--output",
                    target_text,
                ],
            );
            assert_code(&output, exit_code::REJECTED);
            assert!(output.stdout.is_empty());
        }
    }
    assert!(!new.exists());
    assert_eq!(fs::read_to_string(&old).expect("kept"), "old content");
    assert_eq!(entries(folder.path()), ["old.csv"]);
}

/// Overwrites bytes in the middle of a table file, past the part opening reads, so the damage
/// is found while the query reads records.
fn damage_middle(path: &Path) {
    let size = fs::metadata(path).expect("the table exists").len();
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the table can be opened for writing");
    file.seek(SeekFrom::Start(size / 3)).expect("seekable");
    file.write_all(&vec![0xa5; (size / 3) as usize])
        .expect("the table can be damaged");
}

#[test]
fn a_read_error_after_writing_started_leaves_no_file_behind() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    damage_middle(&fixture.version_dir().join("primary.sst"));
    let folder = results_folder();
    let new = folder.path().join("new.json");
    let old = folder.path().join("old.csv");
    fs::write(&old, "old content").expect("writable");
    for (target, format) in [(&new, "json"), (&old, "csv")] {
        let output = query_on(
            &fixture.output(),
            &[
                "--where",
                "Primary Type=theft",
                "--format",
                format,
                "--output",
                target.to_str().expect("UTF-8"),
            ],
        );
        assert_code(&output, exit_code::REJECTED);
        assert!(output.stdout.is_empty());
        assert_eq!(
            stderr_lines(&output),
            [format!(
                "No se pudo leer el conjunto de datos en {}.",
                fixture.output().display()
            )]
        );
    }
    assert!(!new.exists());
    assert_eq!(fs::read_to_string(&old).expect("kept"), "old content");
    assert_eq!(
        entries(folder.path()),
        ["old.csv"],
        "no temporary file left"
    );
}

#[test]
fn an_unwritable_target_is_rejected_before_the_query_runs() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let folder = results_folder();
    let a_folder = folder.path().join("a-folder");
    fs::create_dir(&a_folder).expect("a folder");
    let in_missing_folder = folder.path().join("missing").join("results.csv");
    for target in [&a_folder, &in_missing_folder] {
        let output = query_on(
            &fixture.output(),
            &[
                "--where",
                "ID=10",
                "--format",
                "csv",
                "--output",
                target.to_str().expect("UTF-8"),
            ],
        );
        assert_code(&output, exit_code::REJECTED);
        assert!(output.stdout.is_empty());
        assert_eq!(stderr_lines(&output), [unwritable(target)]);
    }
    assert_eq!(entries(folder.path()), ["a-folder"]);
    assert!(entries(&a_folder).is_empty());
}

fn options(dataset: &Path, args: &[&str]) -> Options {
    let mut all = vec!["crime-query".to_owned(), "--dataset".to_owned()];
    all.push(dataset.to_str().expect("UTF-8").to_owned());
    all.extend(args.iter().map(|&arg| arg.to_owned()));
    match cli::parse(all) {
        Parsed::Run(options) => options,
        other => panic!("{args:?} should be a valid command line: {other:?}"),
    }
}

/// Runs the library entry point with `cancel`, returning the exit code, stdout and stderr.
fn run_with(options: &Options, cancel: &dyn Fn() -> bool) -> (u8, Vec<u8>, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = crime_query::run(options, cancel, &mut out, &mut err);
    (code, out, String::from_utf8(err).expect("UTF-8 stderr"))
}

/// A cancellation check that answers `true` from its `n`-th call on.
fn raised_after(n: usize) -> impl Fn() -> bool {
    let calls = Cell::new(0);
    move || {
        calls.set(calls.get() + 1);
        calls.get() > n
    }
}

#[test]
fn the_library_run_matches_the_binary() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let args = ["--where", "Primary Type=theft", "--format", "csv"];
    let (code, out, err) = run_with(&options(&fixture.output(), &args), &|| false);
    assert_eq!(code, exit_code::COMPLETED, "{err}");
    let binary = query_on(&fixture.output(), &args);
    assert_eq!(out, binary.stdout);
    assert_eq!(err.lines().count(), 1, "{err}");
}

#[test]
fn a_cancelled_query_exits_with_130_and_leaves_no_file() {
    let fixture = Fixture::with_rows(&many_rows(3_000));
    let folder = results_folder();
    let new = folder.path().join("new.csv");
    let old = folder.path().join("old.json");
    fs::write(&old, "old content").expect("writable");
    let cases: [(PathBuf, &str, usize); 4] = [
        (new.clone(), "csv", 0),
        (new.clone(), "csv", 2_000),
        (old.clone(), "json", 0),
        (old.clone(), "json", 2_000),
    ];
    for (target, format, after) in cases {
        let options = options(
            &fixture.output(),
            &[
                "--where",
                "Primary Type=theft",
                "--format",
                format,
                "--output",
                target.to_str().expect("UTF-8"),
            ],
        );
        let (code, out, err) = run_with(&options, &raised_after(after));
        assert_eq!(code, exit_code::CANCELLED, "{err}");
        assert!(out.is_empty());
        assert_eq!(err, "Consulta cancelada.\n");
    }
    assert!(!new.exists());
    assert_eq!(fs::read_to_string(&old).expect("kept"), "old content");
    assert_eq!(
        entries(folder.path()),
        ["old.json"],
        "no temporary file left"
    );

    for mode in [&["--format", "table"][..], &["--count"][..]] {
        let mut args = vec!["--where", "Primary Type=theft"];
        args.extend(mode);
        let (code, _, err) = run_with(&options(&fixture.output(), &args), &raised_after(10));
        assert_eq!(code, exit_code::CANCELLED, "{mode:?}");
        assert_eq!(err, "Consulta cancelada.\n", "{mode:?}");
    }
}

/// A stdout that fails every write with `kind`.
struct Failing(io::ErrorKind);

impl Write for Failing {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(self.0))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(self.0))
    }
}

#[test]
fn a_closed_pipe_ends_quietly_and_other_stdout_errors_fail() {
    let fixture = Fixture::with_rows(&many_rows(50));
    for mode in [
        &["--format", "csv"][..],
        &["--format", "json"][..],
        &["--format", "table"][..],
        &["--count"][..],
    ] {
        let mut args = vec!["--where", "Primary Type=theft"];
        args.extend(mode);
        let options = options(&fixture.output(), &args);

        let mut err = Vec::new();
        let mut closed = Failing(io::ErrorKind::BrokenPipe);
        let code = crime_query::run(&options, &|| false, &mut closed, &mut err);
        assert_eq!(code, exit_code::COMPLETED, "{mode:?}");
        assert!(
            err.is_empty(),
            "{mode:?}: {}",
            String::from_utf8_lossy(&err)
        );

        let mut err = Vec::new();
        let mut full = Failing(io::ErrorKind::Other);
        let code = crime_query::run(&options, &|| false, &mut full, &mut err);
        assert_eq!(code, exit_code::REJECTED, "{mode:?}");
        assert_eq!(
            String::from_utf8_lossy(&err),
            "No se puede escribir en la salida estándar.\n",
            "{mode:?}"
        );
    }
}
