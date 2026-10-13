//! The built binary end to end: the help, exit codes (decision C6) and the order of the checks
//! (command line, then the query conditions, then the dataset).

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use common::{Fixture, fixture_rows};
use crime_query::exit_code;

fn query(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crime-query"))
        .args(args)
        .output()
        .expect("the query app can be started")
}

fn query_on(dataset: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crime-query"));
    command.arg("--dataset").arg(dataset).args(args);
    command.output().expect("the query app can be started")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Asserts the exit code, that stdout is empty and that stderr is exactly `message`.
fn assert_rejected(output: &Output, code: u8, message: &str) {
    assert_eq!(
        output.status.code(),
        Some(i32::from(code)),
        "{}",
        stderr(output)
    );
    assert_eq!(stderr(output).trim_end(), message);
    assert!(output.stdout.is_empty(), "nothing goes to stdout");
}

#[test]
fn exit_codes_match_the_generator() {
    assert_eq!(exit_code::COMPLETED, 0);
    assert_eq!(exit_code::REJECTED, 1);
    assert_eq!(exit_code::USAGE, 2);
    assert_eq!(exit_code::CANCELLED, 130);
}

#[test]
fn the_help_goes_to_stdout_and_exits_with_0() {
    let output = query(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("Uso: crime-query --dataset <CARPETA>"),
        "{text}"
    );
    assert!(text.contains("Códigos de salida:"), "{text}");
    assert!(output.stderr.is_empty(), "{}", stderr(&output));
}

#[test]
fn command_line_errors_exit_with_2() {
    let help = " Usa --help para ver las opciones.";
    let cases: [(&[&str], String); 9] = [
        (
            &[],
            format!("Faltan opciones obligatorias: --dataset <CARPETA>.{help}"),
        ),
        (
            &["--dataset", "out", "--bogus"],
            format!("Opción desconocida: --bogus.{help}"),
        ),
        (
            &["--dataset", "out", "--where", "ID"],
            "La condición \"ID\" no tiene la forma CAMPO=VALOR.".to_owned(),
        ),
        (
            &["--dataset", "out", "--range", "Date=2020-01-01"],
            "El rango \"Date=2020-01-01\" no tiene la forma Date=DESDE..HASTA.".to_owned(),
        ),
        (
            &["--dataset", "out", "--range", "Date=.."],
            "El rango \"Date=..\" necesita al menos una fecha: Date=DESDE.., Date=..HASTA o \
             Date=DESDE..HASTA."
                .to_owned(),
        ),
        (
            &["--dataset", "out", "--limit", "0"],
            "El límite debe ser un número entero mayor que 0.".to_owned(),
        ),
        (
            &["--dataset", "out", "--count", "--format", "csv"],
            "\"Solo conteo\" no se combina con un formato de salida.".to_owned(),
        ),
        (
            &["--dataset", "out", "--output", "r.txt"],
            "El archivo de salida solo admite los formatos CSV o JSON.".to_owned(),
        ),
        (
            &["--dataset", "out", "--format", "xml"],
            "El formato \"xml\" no es válido. Formatos admitidos: table, csv, json.".to_owned(),
        ),
    ];
    for (args, message) in cases {
        assert_rejected(&query(args), exit_code::USAGE, &message);
    }
}

#[test]
fn the_command_line_is_checked_before_the_conditions() {
    let output = query(&["--dataset", "missing", "--where", "Foo=1", "--limit", "-1"]);
    assert_rejected(
        &output,
        exit_code::USAGE,
        "El límite debe ser un número entero mayor que 0.",
    );
}

#[test]
fn query_errors_exit_with_1_before_the_dataset_is_opened() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let missing = dir.path().join("missing");
    assert_rejected(
        &query_on(&missing, &["--where", "Arrest=true"]),
        exit_code::REJECTED,
        "Indica al menos una condición sobre un campo indexado: ID, número de caso, fecha, tipo \
         principal, distrito o área comunitaria.",
    );
    assert_rejected(
        &query_on(&missing, &["--where", "Community Area=x"]),
        exit_code::REJECTED,
        "\"x\" no es válido para \"Community Area\": se esperaba un número entero.",
    );
}

#[test]
fn a_missing_dataset_exits_with_1() {
    let dir = tempfile::tempdir().expect("a temporary directory can be created");
    let missing = dir.path().join("missing");
    assert_rejected(
        &query_on(&missing, &["--where", "ID=1"]),
        exit_code::REJECTED,
        &format!(
            "No se encontró un conjunto de datos válido en {}.",
            missing.display()
        ),
    );
}

#[test]
fn a_schema_1_dataset_exits_with_1() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let manifest_path = fixture.version_dir().join("manifest.json");
    let manifest = fs::read_to_string(&manifest_path).expect("the manifest exists");
    let old = manifest.replace("\"schema_version\": 2", "\"schema_version\": 1");
    assert_ne!(old, manifest, "the manifest should name schema 2");
    fs::write(&manifest_path, old).expect("writable");

    assert_rejected(
        &query_on(&fixture.output(), &["--where", "ID=10"]),
        exit_code::REJECTED,
        "El conjunto usa el esquema v1 y este consultor admite v2. Regenera el conjunto.",
    );
}

#[test]
fn a_valid_query_on_a_valid_dataset_exits_with_0() {
    let fixture = Fixture::with_rows(&fixture_rows());
    let output = query_on(
        &fixture.output(),
        &[
            "--where",
            "Primary Type=theft",
            "--range",
            "Date=2020-01-01..",
            "--where",
            "Arrest=no",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // Interim behavior until the execution task (T4): nothing is printed yet.
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty(), "{}", stderr(&output));
}
