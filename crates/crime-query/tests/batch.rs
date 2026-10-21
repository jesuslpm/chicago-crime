//! Query batches (`--batch`, canopy-db-lote PRD, AC-B1…AC-B8): the file form, the upfront
//! checks and their codes, the output of each query and of the batch, failures, cancellation and
//! the version every query reads. The exact moments (a cancellation or a publication between two
//! queries, a failing stdout) are driven through the library with writers that act when a header
//! is written.

mod common;

use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::rc::Rc;

use common::{
    Fixture, damage_middle, fixture_rows, many_rows, query_on, row, stderr_lines, stdout,
};
use crime_query::cli::BatchOptions;
use crime_query::{exit_code, run_batch};
use tempfile::TempDir;

/// A folder for the batch file and the output files, which is also the current folder of the
/// runs, so relative paths resolve there.
fn work() -> TempDir {
    tempfile::tempdir().expect("a temporary directory can be created")
}

/// Writes `content` as the batch file `name` in `dir`.
fn batch_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).expect("the batch file can be written");
    path
}

/// Runs the binary with `--dataset <dataset> --batch <file>` and `extra`, in `cwd`.
fn batch_on(dataset: &Path, file: &Path, cwd: &Path, extra: &[&str]) -> Output {
    let binary = env!("CARGO_BIN_EXE_crime-query");
    Command::new(binary)
        .current_dir(cwd)
        .arg("--dataset")
        .arg(dataset)
        .arg("--batch")
        .arg(file)
        .args(extra)
        .output()
        .expect("the query app can be started")
}

/// Runs the binary with `args` only, in `cwd`.
fn raw(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crime-query"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("the query app can be started")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("the query app exited")
}

/// The single query `args` run alone in `cwd`.
fn alone(dataset: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crime-query"))
        .current_dir(cwd)
        .arg("--dataset")
        .arg(dataset)
        .args(args)
        .output()
        .expect("the query app can be started")
}

/// A stats line or the batch's last line with its time replaced by `<t>`.
fn without_time(line: &str) -> String {
    match line.split_once(" en ") {
        Some((head, tail)) => match tail.split_once(" s.") {
            Some((seconds, rest)) => {
                assert!(
                    seconds
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == ','),
                    "{line}"
                );
                format!("{head} en <t> s.{rest}")
            }
            None => line.to_owned(),
        },
        None => line.to_owned(),
    }
}

fn lines_without_time(output: &Output) -> Vec<String> {
    stderr_lines(output)
        .iter()
        .map(|line| without_time(line))
        .collect()
}

/// Asserts a rejected batch: the code, the only stderr line, no stdout.
fn assert_rejected(output: &Output, expected_code: u8, message: &str) {
    assert_eq!(
        code(output),
        i32::from(expected_code),
        "{:?}",
        stderr_lines(output)
    );
    assert_eq!(stderr_lines(output), [message]);
    assert_eq!(stdout(output), "");
}

#[test]
fn each_query_runs_as_it_would_alone_and_the_batch_reports_every_one() {
    // AC-B1, AC-B6.
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let queries: [&[&str]; 4] = [
        &["--where", "Primary Type=theft", "--count"],
        &["--where", "Primary Type=BATTERY"],
        &[
            "--where",
            "District=008",
            "--format",
            "csv",
            "--output",
            "district.csv",
        ],
        &[
            "--where", "ID=10", "--where", "ID=999", "--format", "json", "--limit", "1",
        ],
    ];
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where \"Primary Type=theft\" --count\n\
          --where \"Primary Type=BATTERY\"\n\
          --where District=008 --format csv --output district.csv\n\
          --where ID=10 --where ID=999 --format json --limit 1\n",
    );
    let batch = batch_on(&fixture.output(), &file, dir.path(), &[]);
    assert_eq!(code(&batch), 0, "{:?}", stderr_lines(&batch));
    let batch_csv = fs::read(dir.path().join("district.csv")).expect("the batch wrote the file");

    let mut expected_stdout = String::new();
    let mut expected_stderr = Vec::new();
    for (index, query) in queries.iter().enumerate() {
        let single = alone(&fixture.output(), dir.path(), query);
        assert_eq!(code(&single), 0);
        expected_stdout.push_str(&stdout(&single));
        expected_stderr.push(format!(
            "--- Consulta {} de 4 (línea {}) ---",
            index + 1,
            index + 1
        ));
        expected_stderr.extend(lines_without_time(&single));
    }
    expected_stderr.push("Lote completado: 4 consultas en <t> s.".to_owned());
    assert_eq!(stdout(&batch), expected_stdout);
    assert_eq!(lines_without_time(&batch), expected_stderr);
    // The single run above rewrote district.csv with what the query writes alone.
    assert_eq!(
        batch_csv,
        fs::read(dir.path().join("district.csv")).unwrap()
    );
}

#[test]
fn a_batch_of_one_query_ends_with_one_query_in_the_last_line() {
    // AC-B6.
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(dir.path(), "batch.txt", b"--where ID=10 --count");
    let output = batch_on(&fixture.output(), &file, dir.path(), &[]);
    assert_eq!(code(&output), 0);
    assert_eq!(stdout(&output), "1\n");
    assert_eq!(
        lines_without_time(&output),
        [
            "--- Consulta 1 de 1 (línea 1) ---",
            "1 resultado en <t> s. Índices usados: ID. Filtros sin índice: ninguno.",
            "Lote completado: 1 consulta en <t> s.",
        ]
    );
}

#[test]
fn the_file_form_accepts_bom_crlf_comments_and_shell_like_quotes() {
    // AC-B3.
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"\xEF\xBB\xBF# a comment\r\n\
          \r\n\
          \t \n\
          --where 'Primary Type=theft' --count\r\n\
          \t# an indented comment\n\
          --where=\"Primary Type=BATTERY\" --count\n\
          --where \"Case Number=#1\" --count\r\n\
          --where Primary\" \"Type=ASSAULT --format csv",
    );
    let output = batch_on(&fixture.output(), &file, dir.path(), &[]);
    assert_eq!(code(&output), 0, "{:?}", stderr_lines(&output));
    let assault = alone(
        &fixture.output(),
        dir.path(),
        &["--where", "Primary Type=ASSAULT", "--format", "csv"],
    );
    assert_eq!(stdout(&output), format!("3\n2\n0\n{}", stdout(&assault)));
    let headers: Vec<String> = stderr_lines(&output)
        .into_iter()
        .filter(|line| line.starts_with("--- "))
        .collect();
    assert_eq!(
        headers,
        [
            "--- Consulta 1 de 4 (línea 4) ---",
            "--- Consulta 2 de 4 (línea 6) ---",
            "--- Consulta 3 de 4 (línea 7) ---",
            "--- Consulta 4 de 4 (línea 8) ---",
        ]
    );
    assert!(
        stderr_lines(&output)
            .contains(&"No existe ningún delito con número de caso #1.".to_owned())
    );
}

#[test]
fn a_wrong_line_rejects_the_batch_before_anything_runs() {
    // AC-B4: the line number is the line in the file, the message the single query's.
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let head = "--where ID=10 --format csv --output first.csv\n--where ID=20 --count\n\n# note\n";
    let form = batch_file(
        dir.path(),
        "form.txt",
        format!("{head}--where \"Primary Type\" --count\n--limit 0\n").as_bytes(),
    );
    let output = batch_on(&fixture.output(), &form, dir.path(), &[]);
    assert_rejected(
        &output,
        exit_code::USAGE,
        "Línea 5: La condición \"Primary Type\" no tiene la forma CAMPO=VALOR.",
    );
    let conditions = batch_file(
        dir.path(),
        "conditions.txt",
        format!("{head}--where Ward=abc\n--where \"Primary Type\"\n").as_bytes(),
    );
    let output = batch_on(&fixture.output(), &conditions, dir.path(), &[]);
    assert_rejected(
        &output,
        exit_code::REJECTED,
        "Línea 5: \"abc\" no es válido para \"Ward\": se esperaba un número entero.",
    );
    let unindexed = batch_file(
        dir.path(),
        "unindexed.txt",
        b"--where Arrest=true --count\n",
    );
    let output = batch_on(&fixture.output(), &unindexed, dir.path(), &[]);
    assert_rejected(
        &output,
        exit_code::REJECTED,
        "Línea 1: Indica al menos una condición sobre un campo indexado: ID, número de caso, \
         fecha, tipo principal, distrito o área comunitaria.",
    );
    let unknown = batch_file(dir.path(), "unknown.txt", b"--where ID=10 --lmit 3\n");
    let output = batch_on(&fixture.output(), &unknown, dir.path(), &[]);
    assert_rejected(
        &output,
        exit_code::USAGE,
        "Línea 1: Opción desconocida: --lmit. Usa --help para ver las opciones.",
    );
    assert!(!dir.path().join("first.csv").exists());
}

#[test]
fn every_batch_rule_has_its_message_and_code() {
    // AC-B5.
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let ok = batch_file(dir.path(), "ok.txt", b"--where ID=10 --count\n");
    let dataset = fixture.output();
    let dataset = dataset.to_str().unwrap();
    let ok_path = ok.to_str().unwrap();
    let cases: Vec<(Output, u8, String)> = vec![
        (
            raw(dir.path(), &["--dataset", dataset, "--batch", ok_path, "--count", "--where", "ID=1"]),
            exit_code::USAGE,
            "Con --batch, la opción --count va en cada línea del lote, no en la línea de comandos."
                .to_owned(),
        ),
        (
            raw(dir.path(), &["--dataset", dataset, "--batch", ok_path, "--output=x.csv", "--format", "csv"]),
            exit_code::USAGE,
            "Con --batch, la opción --output va en cada línea del lote, no en la línea de comandos."
                .to_owned(),
        ),
        (
            raw(dir.path(), &["--dataset", dataset, "--batch", ok_path, "--limit", "abc"]),
            exit_code::USAGE,
            "El límite debe ser un número entero mayor que 0.".to_owned(),
        ),
        (
            raw(dir.path(), &["--batch", ok_path]),
            exit_code::USAGE,
            "Faltan opciones obligatorias: --dataset <CARPETA>. Usa --help para ver las opciones."
                .to_owned(),
        ),
        (
            raw(dir.path(), &["--dataset", dataset, "--batch", ok_path, "--batch", ok_path]),
            exit_code::USAGE,
            "Los argumentos no son válidos. Usa --help para ver las opciones.".to_owned(),
        ),
    ];
    for (output, expected_code, message) in cases {
        assert_rejected(&output, expected_code, &message);
    }

    for missing in ["missing.txt", "", "-", "."] {
        let output = batch_on(&fixture.output(), Path::new(missing), dir.path(), &[]);
        assert_rejected(
            &output,
            exit_code::REJECTED,
            &format!("No se puede leer el lote {missing}."),
        );
    }
    let latin1 = batch_file(
        dir.path(),
        "latin1.txt",
        b"--where \"Location Description=CAF\xC9\"\n",
    );
    assert_rejected(
        &batch_on(&fixture.output(), &latin1, dir.path(), &[]),
        exit_code::REJECTED,
        &format!("El lote {} no es un texto UTF-8 válido.", latin1.display()),
    );
    for content in [&b""[..], b"\xEF\xBB\xBF", b"# only\n\n  \t\r\n"] {
        let empty = batch_file(dir.path(), "empty.txt", content);
        assert_rejected(
            &batch_on(&fixture.output(), &empty, dir.path(), &[]),
            exit_code::REJECTED,
            &format!("El lote {} no contiene ninguna consulta.", empty.display()),
        );
    }

    let lines: Vec<(&[u8], u8, &str)> = vec![
        (b"--where ID=10\n--where \"Primary Type=THEFT --count\n", exit_code::USAGE, "Línea 2: falta cerrar unas comillas."),
        (b"--where ID=10 --dataset other\n", exit_code::USAGE, "Línea 1: la opción --dataset no se admite dentro del lote."),
        (b"--where ID=10 --batch=other.txt\n", exit_code::USAGE, "Línea 1: la opción --batch no se admite dentro del lote."),
        (b"--help\n", exit_code::USAGE, "Línea 1: la opción --help no se admite dentro del lote."),
        (b"--where ID=10 -h\n", exit_code::USAGE, "Línea 1: la opción -h no se admite dentro del lote."),
        (
            b"--where ID=10 --format csv --output out.csv\n--where ID=20 --format json --output ./sub/../out.csv\n",
            exit_code::REJECTED,
            "Línea 2: el archivo de salida ./sub/../out.csv ya lo usa la línea 1.",
        ),
        (
            b"--where ID=10 --format csv --output rules.txt\n",
            exit_code::REJECTED,
            "Línea 1: el archivo de salida rules.txt es el propio lote.",
        ),
    ];
    for (content, expected_code, message) in lines {
        let file = batch_file(dir.path(), "rules.txt", content);
        let output = batch_on(&fixture.output(), &file, dir.path(), &[]);
        assert_rejected(&output, expected_code, message);
    }
    assert!(!dir.path().join("out.csv").exists());

    let missing_dataset = batch_on(&dir.path().join("nothing"), &ok, dir.path(), &[]);
    assert_rejected(
        &missing_dataset,
        exit_code::REJECTED,
        &format!(
            "No se encontró un conjunto de datos válido en {}.",
            dir.path().join("nothing").display()
        ),
    );
}

#[cfg(windows)]
#[test]
fn output_paths_are_compared_without_case_on_windows() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where ID=10 --format csv --output out.csv\n--where ID=20 --format csv --output OUT.CSV\n",
    );
    assert_rejected(
        &batch_on(&fixture.output(), &file, dir.path(), &[]),
        exit_code::REJECTED,
        "Línea 2: el archivo de salida OUT.CSV ya lo usa la línea 1.",
    );
}

#[test]
fn a_query_that_fails_stops_the_batch_and_earlier_results_stay() {
    // AC-B7: a folder that does not exist and a path that is a folder.
    let fixture = Fixture::with_rows(&fixture_rows());
    for target in ["missing/second.csv", "folder"] {
        let dir = work();
        fs::create_dir(dir.path().join("folder")).unwrap();
        let file = batch_file(
            dir.path(),
            "batch.txt",
            format!(
                "--where ID=10 --format csv --output first.csv\n\
                 --where ID=20 --format csv --output {target}\n\
                 --where ID=30 --format csv --output third.csv\n"
            )
            .as_bytes(),
        );
        let output = batch_on(&fixture.output(), &file, dir.path(), &[]);
        assert_eq!(code(&output), i32::from(exit_code::REJECTED), "{target}");
        assert_eq!(stdout(&output), "");
        assert_eq!(
            lines_without_time(&output),
            [
                "--- Consulta 1 de 3 (línea 1) ---".to_owned(),
                "1 resultado en <t> s. Índices usados: ID. Filtros sin índice: ninguno.".to_owned(),
                "--- Consulta 2 de 3 (línea 2) ---".to_owned(),
                format!("No se puede escribir el archivo de salida {target}."),
            ]
        );
        assert!(dir.path().join("first.csv").exists());
        assert!(!dir.path().join("third.csv").exists());
    }
}

#[test]
fn a_dataset_that_can_no_longer_be_read_stops_the_batch() {
    let mut fixture = Fixture::new();
    fixture.publish(&many_rows(2000));
    damage_middle(&fixture.version_dir().join("primary.sst"));
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where ID=1 --count\n--where \"Primary Type=THEFT\" --format csv\n--where ID=2 --count\n",
    );
    let output = batch_on(&fixture.output(), &file, dir.path(), &[]);
    assert_eq!(code(&output), i32::from(exit_code::REJECTED));
    let lines = stderr_lines(&output);
    assert_eq!(lines[2], "--- Consulta 2 de 3 (línea 2) ---", "{lines:?}");
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(
        lines[3].starts_with(&format!(
            "No se pudo leer el conjunto de datos en {}.",
            fixture.output().display()
        )),
        "{lines:?}"
    );
}

// Through the library: the exact moments.

fn options(dataset: &Path, file: &Path) -> BatchOptions {
    BatchOptions {
        dataset: dataset.to_path_buf(),
        file: file.to_path_buf(),
    }
}

/// A stderr that keeps what it gets and calls `on_write` with each piece written: a message, or
/// the line break after it.
struct Watched<F: FnMut(&str)> {
    text: Rc<RefCell<String>>,
    on_write: F,
}

impl<F: FnMut(&str)> Write for Watched<F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let piece = String::from_utf8_lossy(bytes).into_owned();
        (self.on_write)(&piece);
        self.text.borrow_mut().push_str(&piece);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_cancellation_during_a_query_stops_the_batch_and_earlier_files_stay() {
    // AC-B8: cancelled from the second query on.
    let mut fixture = Fixture::new();
    fixture.publish(&many_rows(500));
    let dir = work();
    let first = dir.path().join("first.csv");
    let second = dir.path().join("second.csv");
    let third = dir.path().join("third.csv");
    let file = batch_file(
        dir.path(),
        "batch.txt",
        format!(
            "--where ID=1 --format csv --output \"{}\"\n\
             --where \"Primary Type=THEFT\" --format csv --output \"{}\"\n\
             --where ID=3 --format csv --output \"{}\"\n",
            first.display(),
            second.display(),
            third.display()
        )
        .as_bytes(),
    );
    let cancelled = Cell::new(false);
    let text = Rc::new(RefCell::new(String::new()));
    let mut stderr = Watched {
        text: Rc::clone(&text),
        on_write: |piece: &str| {
            if piece.starts_with("--- Consulta 2 ") {
                cancelled.set(true);
            }
        },
    };
    let mut out = Vec::new();
    let code = run_batch(
        &options(&fixture.output(), &file),
        &|| cancelled.get(),
        &mut out,
        &mut stderr,
    );
    assert_eq!(code, exit_code::CANCELLED);
    let text = text.borrow();
    assert!(
        text.ends_with("--- Consulta 2 de 3 (línea 2) ---\nConsulta cancelada.\n"),
        "{text}"
    );
    assert!(!text.contains("Lote completado"));
    assert!(out.is_empty());
    let single = alone(
        &fixture.output(),
        dir.path(),
        &["--where", "ID=1", "--format", "csv"],
    );
    assert_eq!(fs::read(&first).unwrap(), single.stdout);
    assert!(!second.exists());
    assert!(!third.exists());
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".crime-query.tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_cancellation_before_the_first_query_runs_nothing() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(dir.path(), "batch.txt", b"--where ID=10 --count\n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_batch(
        &options(&fixture.output(), &file),
        &|| true,
        &mut out,
        &mut err,
    );
    assert_eq!(code, exit_code::CANCELLED);
    assert_eq!(String::from_utf8(err).unwrap(), "Consulta cancelada.\n");
    assert!(out.is_empty());
}

#[test]
fn a_cancellation_between_two_queries_writes_no_header_for_the_next() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where ID=10 --count\n--where ID=20 --count\n",
    );
    let mut out = Vec::new();
    // Raised once the first query has written its stats line.
    let cancelled = Cell::new(false);
    let text = Rc::new(RefCell::new(String::new()));
    let mut stderr = Watched {
        text: Rc::clone(&text),
        on_write: |piece: &str| {
            if piece.starts_with("1 resultado en ") {
                cancelled.set(true);
            }
        },
    };
    let code = run_batch(
        &options(&fixture.output(), &file),
        &|| cancelled.get(),
        &mut out,
        &mut stderr,
    );
    assert_eq!(code, exit_code::CANCELLED);
    let lines: Vec<String> = text.borrow().lines().map(without_time).collect();
    assert_eq!(
        lines,
        [
            "--- Consulta 1 de 2 (línea 1) ---",
            "1 resultado en <t> s. Índices usados: ID. Filtros sin índice: ninguno.",
            "Consulta cancelada.",
        ]
    );
    assert_eq!(String::from_utf8(out).unwrap(), "1\n");
}

#[test]
fn every_query_reads_the_version_the_batch_opened() {
    // AC-B2: a new version is published once the first query has run.
    let fixture = RefCell::new(Fixture::with_rows(&fixture_rows()));
    let dataset = fixture.borrow().output();
    let opened = fixture.borrow().version_dir();
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where \"Primary Type=THEFT\" --format json\n\
          --where \"Primary Type=THEFT\" --format json\n\
          --where \"Primary Type=THEFT\" --count\n",
    );
    let text = Rc::new(RefCell::new(String::new()));
    let mut stderr = Watched {
        text: Rc::clone(&text),
        on_write: |piece: &str| {
            if piece.starts_with("--- Consulta 2 ") {
                fixture
                    .borrow_mut()
                    .publish(&[row(&[("ID", "1"), ("Primary Type", "THEFT")])]);
            }
        },
    };
    let mut out = Vec::new();
    let code = run_batch(&options(&dataset, &file), &|| false, &mut out, &mut stderr);
    assert_eq!(code, exit_code::COMPLETED, "{}", text.borrow());
    assert_ne!(
        fixture.borrow().version_dir(),
        opened,
        "a new version was published"
    );
    let out = String::from_utf8(out).unwrap();
    let (json, count) = out
        .rsplit_once("]}\n")
        .expect("two JSON documents, then the count");
    assert_eq!(count, "3\n");
    let version = opened.file_name().unwrap().to_string_lossy().into_owned();
    let documents: Vec<serde_json::Value> =
        serde_json::Deserializer::from_str(&format!("{json}]}}"))
            .into_iter()
            .map(|document| document.expect("valid JSON"))
            .collect();
    assert_eq!(documents.len(), 2);
    for document in documents {
        assert_eq!(document["dataset"]["version"], version.as_str());
        assert_eq!(document["results"].as_array().unwrap().len(), 3);
    }
    // After the batch, a single query reads the new version.
    let after = query_on(&dataset, &["--where", "Primary Type=THEFT", "--count"]);
    assert_eq!(stdout(&after), "1\n");
}

/// A stdout that refuses every write with `kind`.
struct Refusing(io::ErrorKind);

impl Write for Refusing {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(self.0))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(self.0))
    }
}

#[test]
fn a_closed_stdout_stops_the_batch_quietly_and_another_failure_with_1() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let dir = work();
    let file = batch_file(
        dir.path(),
        "batch.txt",
        b"--where ID=10\n--where ID=20 --count\n",
    );
    let mut err = Vec::new();
    let code = run_batch(
        &options(&fixture.output(), &file),
        &|| false,
        &mut Refusing(io::ErrorKind::BrokenPipe),
        &mut err,
    );
    assert_eq!(code, exit_code::COMPLETED);
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "--- Consulta 1 de 2 (línea 1) ---\n"
    );

    let mut err = Vec::new();
    let code = run_batch(
        &options(&fixture.output(), &file),
        &|| false,
        &mut Refusing(io::ErrorKind::Other),
        &mut err,
    );
    assert_eq!(code, exit_code::REJECTED);
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "--- Consulta 1 de 2 (línea 1) ---\nNo se puede escribir en la salida estándar.\n"
    );
}
