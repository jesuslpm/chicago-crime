//! The built binary end to end: exit codes, console output and the run log.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{csv_bytes, csv_with_header, header_row, row_with_id, sample_row, with_field};

fn generator(args: &[&Path]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-generator"));
    let mut flags = ["--input", "--output", "--temp-dir"].iter();
    for arg in args {
        command
            .arg(flags.next().expect("at most three paths"))
            .arg(arg);
    }
    command.output().expect("the generator can be started")
}

/// The single file in `dir` named `<output name>-YYYYMMDD-HHMMSS.log`.
fn default_log(dir: &Path, output_name: &str) -> PathBuf {
    let logs: Vec<PathBuf> = fs::read_dir(dir)
        .expect("the folder can be listed")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let Some(stamp) = name
                .strip_prefix(&format!("{output_name}-"))
                .and_then(|rest| rest.strip_suffix(".log"))
            else {
                return false;
            };
            stamp.len() == 15
                && stamp.char_indices().all(|(index, character)| {
                    (index == 8 && character == '-') || (index != 8 && character.is_ascii_digit())
                })
        })
        .collect();
    assert_eq!(logs.len(), 1, "exactly one default log: {logs:?}");
    logs.into_iter().next().expect("one log")
}

#[test]
fn reads_the_csv_and_logs_rejections_next_to_the_output_folder() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    fs::write(
        &input,
        csv_with_header(&[
            row_with_id(1),
            with_field(sample_row(), "Date", b"tomorrow"),
            row_with_id(2),
        ]),
    )
    .expect("the fixture can be written");
    let output = dir.path().join("dataset");

    let result = generator(&[&input, &output, dir.path()]);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        result.status.success(),
        "exit {:?}: {stderr}",
        result.status
    );
    let rejection = "Fila 3 omitida, columna \"Date\": \"tomorrow\" no es una fecha válida \
                     (formato MM/DD/AAAA hh:mm:ss AM/PM).";
    let summary = "Lectura completada: 2 registros válidos, 1 rechazados.";
    let log = fs::read_to_string(default_log(dir.path(), "dataset")).expect("log");
    for expected in [rejection, summary] {
        assert!(
            log.contains(expected),
            "log should contain {expected:?}: {log}"
        );
        assert!(
            stderr.contains(expected),
            "console should echo {expected:?}: {stderr}"
        );
    }
    assert!(log.contains("SHA-256"), "{log}");
    assert!(!output.exists(), "no dataset is written yet");
}

#[test]
fn header_mismatch_exits_with_failure_and_explains_it_in_the_log() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    let mut header = header_row();
    header[4] = b"IUCR Code".to_vec();
    fs::write(&input, csv_bytes(&[header, sample_row()])).expect("the fixture can be written");
    let output = dir.path().join("dataset");

    let result = generator(&[&input, &output, dir.path()]);

    let message = "La cabecera no coincide con el esquema: columna 5 esperaba \"IUCR\" y \
                   encontró \"IUCR Code\".";
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(stderr.contains(message), "{stderr}");
    let log = fs::read_to_string(default_log(dir.path(), "dataset")).expect("log");
    assert!(log.contains(message), "{log}");
}

#[test]
fn missing_input_exits_with_failure() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("missing.csv");
    let output = dir.path().join("dataset");

    let result = generator(&[&input, &output, dir.path()]);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(
        stderr.contains(&format!("No se puede leer el archivo {}.", input.display())),
        "{stderr}"
    );
}

#[test]
fn unwritable_log_exits_with_failure() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    fs::write(&input, csv_with_header(&[row_with_id(1)])).expect("the fixture can be written");
    let log = dir.path().join("missing").join("run.log");

    let result = Command::new(env!("CARGO_BIN_EXE_crime-generator"))
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(dir.path().join("dataset"))
        .arg("--log")
        .arg(&log)
        .output()
        .expect("the generator can be started");

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success());
    assert!(
        stderr.contains(&format!(
            "No se puede escribir el log en {}.",
            log.display()
        )),
        "{stderr}"
    );
}

#[test]
fn missing_options_exit_with_a_usage_error() {
    let result = Command::new(env!("CARGO_BIN_EXE_crime-generator"))
        .output()
        .expect("the generator can be started");

    assert_eq!(result.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("Faltan opciones obligatorias"), "{stderr}");
}
