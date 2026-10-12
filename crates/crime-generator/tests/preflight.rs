//! Preflight checks run before any row is read and abort with the exact message.

use std::fs;

use crime_generator::Abort;
use crime_generator::preflight::{check_output, check_temp_dir, create_log, open_input};
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("a temporary directory can be created")
}

#[test]
fn missing_input_file_cannot_be_read() {
    let dir = temp_dir();
    let input = dir.path().join("missing.csv");

    let abort = open_input(&input).expect_err("the file does not exist");

    assert_eq!(
        abort.to_string(),
        format!("No se puede leer el archivo {}.", input.display())
    );
}

#[test]
fn folder_given_as_input_cannot_be_read() {
    let dir = temp_dir();

    let abort = open_input(dir.path()).expect_err("a folder is not a CSV file");

    assert_eq!(
        abort.to_string(),
        format!("No se puede leer el archivo {}.", dir.path().display())
    );
}

#[test]
fn existing_input_file_is_opened() {
    let dir = temp_dir();
    let input = dir.path().join("data.csv");
    fs::write(&input, b"ID\n").expect("the fixture can be written");

    assert!(open_input(&input).is_ok());
}

#[test]
fn missing_temp_dir_cannot_be_used() {
    let dir = temp_dir();
    let missing = dir.path().join("missing");

    let abort = check_temp_dir(&missing).expect_err("the folder does not exist");

    assert_eq!(
        abort.to_string(),
        format!(
            "No se puede usar la carpeta temporal {}.",
            missing.display()
        )
    );
}

#[test]
fn file_given_as_temp_dir_cannot_be_used() {
    let dir = temp_dir();
    let file = dir.path().join("not-a-folder");
    fs::write(&file, b"x").expect("the fixture can be written");

    let abort = check_temp_dir(&file).expect_err("a file is not a folder");

    assert_eq!(
        abort.to_string(),
        format!("No se puede usar la carpeta temporal {}.", file.display())
    );
}

#[test]
fn writable_temp_dir_is_accepted_and_left_clean() {
    let dir = temp_dir();

    check_temp_dir(dir.path()).expect("the folder is writable");

    let leftovers = fs::read_dir(dir.path())
        .expect("the folder can be listed")
        .count();
    assert_eq!(leftovers, 0, "the probe file must be deleted");
}

#[test]
fn log_in_a_missing_folder_cannot_be_written() {
    let dir = temp_dir();
    let log = dir.path().join("missing").join("run.log");

    let abort = create_log(&log, false).expect_err("the folder does not exist");

    assert_eq!(
        abort.to_string(),
        format!("No se puede escribir el log en {}.", log.display())
    );
}

#[test]
fn existing_file_at_the_log_path_is_refused_and_left_untouched() {
    let dir = temp_dir();
    let path = dir.path().join("run.log");
    fs::write(&path, b"previous run\n").expect("the fixture can be written");

    let abort = create_log(&path, false).expect_err("the log must be a new file");

    assert_eq!(
        abort.to_string(),
        format!("No se puede escribir el log en {}.", path.display())
    );
    assert_eq!(
        fs::read(&path).expect("the file can be read"),
        b"previous run\n",
        "the existing file must be unchanged"
    );
}

#[test]
fn missing_output_folder_is_accepted_and_not_created_yet() {
    let dir = temp_dir();
    let output = dir.path().join("dataset");

    check_output(&output, false).expect("a new output folder is fine");

    assert!(!output.exists(), "the folder is created only when writing");
}

#[test]
fn output_folder_with_a_dataset_is_refused_without_overwrite() {
    let dir = temp_dir();
    fs::write(dir.path().join("primary.sst"), b"table").expect("the fixture can be written");

    let abort = check_output(dir.path(), false).expect_err("the dataset must be kept");

    assert_eq!(abort, Abort::DatasetExists);
    assert_eq!(
        abort.to_string(),
        "La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura \
         para reemplazarlo."
    );
}

#[test]
fn output_folder_with_a_dataset_is_accepted_with_overwrite() {
    let dir = temp_dir();
    fs::write(dir.path().join("primary.sst"), b"table").expect("the fixture can be written");

    check_output(dir.path(), true).expect("overwriting was allowed");
}

#[test]
fn file_given_as_output_folder_is_refused() {
    let dir = temp_dir();
    let file = dir.path().join("dataset");
    fs::write(&file, b"x").expect("the fixture can be written");

    let abort = check_output(&file, true).expect_err("a file is not a folder");

    assert_eq!(
        abort.to_string(),
        format!(
            "No se puede escribir en la carpeta de salida {}.",
            file.display()
        )
    );
}

#[test]
fn log_is_created_and_receives_lines() {
    let dir = temp_dir();
    let path = dir.path().join("run.log");

    let mut log = create_log(&path, false).expect("the log can be created");
    log.line("primera línea").expect("the log is writable");
    log.line("segunda línea").expect("the log is writable");

    assert_eq!(log.path(), path);
    let content = fs::read_to_string(&path).expect("the log can be read");
    assert_eq!(
        content.lines().collect::<Vec<_>>(),
        ["primera línea", "segunda línea"]
    );
}
