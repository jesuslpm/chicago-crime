//! The built binary end to end: exit codes (decision G3), console output, the run log and the
//! final summary.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{
    DATASET_FILES, csv_bytes, csv_with_header, header_row, row_with_id, sample_row, with_field,
};
use crime_generator::exit_code;
use crime_generator::messages::thousands;

const PEAK_MEMORY: &str = "Pico de memoria privada comprometida: ";
const MEMORY_WARNING: &str = "Aviso: el pico de memoria privada comprometida (";

fn generator(args: &[&Path]) -> Output {
    generator_with(args, &[])
}

/// Runs the binary with `--input`, `--output` and `--temp-dir` taken from `paths`, in this order,
/// followed by `extra`.
fn generator_with(paths: &[&Path], extra: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-generator"));
    let mut flags = ["--input", "--output", "--temp-dir"].iter();
    for path in paths {
        command
            .arg(flags.next().expect("at most three paths"))
            .arg(path);
    }
    command
        .args(extra)
        .output()
        .expect("the generator can be started")
}

/// The single line of `log` that starts with `prefix`.
fn line_starting_with<'a>(log: &'a str, prefix: &str) -> &'a str {
    let lines: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with(prefix))
        .collect();
    assert_eq!(lines.len(), 1, "one line starts with {prefix:?}:\n{log}");
    lines[0]
}

/// Whether `text` is a size in MiB with one decimal and a decimal comma, such as `92,7` or
/// `1.024,0`.
fn is_mebibytes(text: &str) -> bool {
    let Some((whole, tenths)) = text.split_once(',') else {
        return false;
    };
    tenths.len() == 1
        && tenths.bytes().all(|byte| byte.is_ascii_digit())
        && !whole.is_empty()
        && whole
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
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
fn writes_the_primary_table_and_logs_rejections_next_to_the_output_folder() {
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
    assert_eq!(
        result.status.code(),
        Some(0),
        "a completed run exits with 0 even with rejections: {stderr}"
    );
    let rejection = "Fila 3 omitida, columna \"Date\": \"tomorrow\" no es una fecha válida \
                     (formato MM/DD/AAAA hh:mm:ss AM/PM).";
    let reading = "Lectura completada: 2 registros válidos, 1 rechazados.";
    let indexes = "Índices generados: date 2, primary_type 2, district 2, community_area 2, \
                   case_number 2.";
    let summary = "Generación completada: 2 registros válidos, 1 rechazados (ver log).";
    let log = fs::read_to_string(default_log(dir.path(), "dataset")).expect("log");
    for expected in [rejection, reading, indexes, summary] {
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
    assert_eq!(
        common::folder_entries(&common::dataset_dir(&output)),
        common::DATASET_FILES,
        "the primary table, the five index tables and the manifest are published"
    );
}

#[test]
fn an_existing_dataset_exits_with_failure_and_is_kept() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    fs::write(&input, csv_with_header(&[row_with_id(1)])).expect("the fixture can be written");
    let output = dir.path().join("dataset");
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).expect("the log folder can be created");
    let first = generator(&[&input, &output, dir.path()]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let published = common::snapshot(&output);

    let result = Command::new(env!("CARGO_BIN_EXE_crime-generator"))
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .arg("--temp-dir")
        .arg(dir.path())
        .arg("--log")
        .arg(logs.join("second.log"))
        .output()
        .expect("the generator can be started");

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(
        result.status.code(),
        Some(i32::from(exit_code::ABORTED)),
        "{stderr}"
    );
    for expected in [
        "La carpeta de salida ya contiene un conjunto de datos.",
        "Generación cancelada. No se ha modificado el conjunto existente.",
    ] {
        assert!(stderr.contains(expected), "{stderr}");
    }
    assert_eq!(common::snapshot(&output), published, "nothing changed");
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
    assert_eq!(
        result.status.code(),
        Some(i32::from(exit_code::ABORTED)),
        "{stderr}"
    );
    assert_eq!(exit_code::ABORTED, 1);
    assert!(stderr.contains(message), "{stderr}");
    let log = fs::read_to_string(default_log(dir.path(), "dataset")).expect("log");
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(
        lines,
        [
            message,
            "Generación cancelada. No se ha modificado el conjunto existente."
        ],
        "the log explains the cause and that nothing changed (AC-G9)"
    );
    assert!(!output.exists(), "a folder this run created is removed");
}

#[test]
fn missing_input_exits_with_failure() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("missing.csv");
    let output = dir.path().join("dataset");

    let result = generator(&[&input, &output, dir.path()]);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(
        result.status.code(),
        Some(i32::from(exit_code::ABORTED)),
        "{stderr}"
    );
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
    assert_eq!(
        result.status.code(),
        Some(i32::from(exit_code::ABORTED)),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "No se puede escribir el log en {}.",
            log.display()
        )),
        "{stderr}"
    );
}

#[test]
fn log_at_the_input_path_is_refused_and_the_input_is_kept() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    let bytes = csv_with_header(&[row_with_id(1), row_with_id(2)]);
    fs::write(&input, &bytes).expect("the fixture can be written");

    let result = Command::new(env!("CARGO_BIN_EXE_crime-generator"))
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(dir.path().join("dataset"))
        .arg("--temp-dir")
        .arg(dir.path())
        .arg("--log")
        .arg(&input)
        .output()
        .expect("the generator can be started");

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        !result.status.success(),
        "exit {:?}: {stderr}",
        result.status
    );
    assert!(
        stderr.contains(&format!(
            "No se puede escribir el log en {}.",
            input.display()
        )),
        "{stderr}"
    );
    assert_eq!(
        fs::read(&input).expect("the input can be read"),
        bytes,
        "the input must be unchanged"
    );
}

#[test]
fn missing_options_exit_with_a_usage_error() {
    let result = Command::new(env!("CARGO_BIN_EXE_crime-generator"))
        .output()
        .expect("the generator can be started");

    assert_eq!(result.status.code(), Some(2));
    assert_eq!(exit_code::USAGE, 2);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("Faltan opciones obligatorias"), "{stderr}");
}

#[test]
fn the_final_summary_lists_counts_duration_peak_memory_every_file_the_folder_and_the_log() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    fs::write(
        &input,
        csv_with_header(&[
            row_with_id(1),
            with_field(sample_row(), "ID", b"x"),
            row_with_id(2),
            row_with_id(3),
        ]),
    )
    .expect("the fixture can be written");
    let output = dir.path().join("dataset");

    let result = generator(&[&input, &output, dir.path()]);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(0), "{stderr}");
    let log_path = default_log(dir.path(), "dataset");
    let log = fs::read_to_string(&log_path).expect("log");
    let dataset = common::dataset_dir(&output);

    let completed = "Generación completada: 3 registros válidos, 1 rechazados (ver log).";
    let folder = format!("Conjunto de datos publicado en {}.", dataset.display());
    let log_line = format!("Log de la ejecución: {}.", log_path.display());
    for expected in [completed, folder.as_str(), log_line.as_str()] {
        assert_eq!(
            log.lines().filter(|line| *line == expected).count(),
            1,
            "{expected:?}:\n{log}"
        );
        assert!(stderr.contains(expected), "echoed on the console: {stderr}");
    }

    let duration = line_starting_with(&log, "Duración: ");
    assert!(duration.ends_with(" s."), "{duration}");

    let peak = line_starting_with(&log, PEAK_MEMORY);
    if cfg!(any(windows, target_os = "linux")) {
        let figure = peak
            .strip_prefix(PEAK_MEMORY)
            .and_then(|rest| rest.strip_suffix(" MiB (límite: 512 MiB)."))
            .unwrap_or_else(|| panic!("the peak and the limit: {peak:?}"));
        assert!(is_mebibytes(figure), "{peak:?}");
    } else {
        assert_eq!(peak, format!("{PEAK_MEMORY}no disponible en este sistema."));
    }
    assert!(!log.contains(MEMORY_WARNING), "{log}");

    let files: Vec<&str> = log
        .lines()
        .filter(|line| line.starts_with("Archivo "))
        .collect();
    assert_eq!(
        files.len(),
        DATASET_FILES.len(),
        "one line per file:\n{log}"
    );
    for file in DATASET_FILES {
        let size = fs::metadata(dataset.join(file)).expect("metadata").len();
        let prefix = format!("Archivo {file}: {} bytes (", thousands(size));
        let line = line_starting_with(&log, &prefix);
        let figure = line
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(" MiB)."))
            .unwrap_or_else(|| panic!("the size in MiB: {line:?}"));
        assert!(is_mebibytes(figure), "{line:?}");
    }

    // The summary closes the log, from the count to the log location.
    let lines: Vec<&str> = log.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == completed)
        .expect("the count");
    assert_eq!(
        lines.len() - start,
        4 + DATASET_FILES.len() + 1,
        "count, duration, peak, folder, files and log:\n{log}"
    );
    assert_eq!(lines.last(), Some(&log_line.as_str()));
}

#[test]
fn a_peak_over_the_memory_limit_is_a_warning_and_the_run_still_exits_with_zero() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let input = dir.path().join("crimes.csv");
    fs::write(&input, csv_with_header(&[row_with_id(1), row_with_id(2)]))
        .expect("the fixture can be written");
    let output = dir.path().join("dataset");

    // Any process commits more than 1 MiB.
    let result = generator_with(&[&input, &output, dir.path()], &["--memory-limit-mib", "1"]);

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(0), "{stderr}");
    let log = fs::read_to_string(default_log(dir.path(), "dataset")).expect("log");
    assert!(log.contains("Generación completada"), "{log}");
    assert_eq!(
        common::folder_entries(&common::dataset_dir(&output)),
        DATASET_FILES
    );
    let peak = line_starting_with(&log, PEAK_MEMORY);
    if cfg!(any(windows, target_os = "linux")) {
        let warning = line_starting_with(&log, MEMORY_WARNING);
        let figure = warning
            .strip_prefix(MEMORY_WARNING)
            .and_then(|rest| rest.strip_suffix(" MiB) supera el límite de 1 MiB."))
            .unwrap_or_else(|| panic!("the peak and the limit: {warning:?}"));
        assert!(
            figure
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.'),
            "{warning:?}"
        );
        assert!(stderr.contains(warning), "echoed on the console: {stderr}");
        assert!(
            peak.ends_with(" MiB (supera el límite de 1 MiB)."),
            "the summary shows it too: {peak:?}"
        );
    } else {
        assert!(!log.contains(MEMORY_WARNING), "{log}");
    }
}
