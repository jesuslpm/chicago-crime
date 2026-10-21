//! Command line options, their defaults and every command-line error (exit code 2), in Spanish.

use std::num::NonZeroU64;
use std::path::PathBuf;

use crime_query::RawCondition;
use crime_query::cli::{Format, Mode, Options, Parsed, parse};

fn parsed(args: &[&str]) -> Parsed {
    parse(std::iter::once("crime-query").chain(args.iter().copied()))
}

fn options(args: &[&str]) -> Options {
    match parsed(args) {
        Parsed::Run(options) => options,
        other => panic!("{args:?} should be accepted: {other:?}"),
    }
}

fn invalid(args: &[&str]) -> String {
    match parsed(args) {
        Parsed::Invalid(message) => message,
        other => panic!("{args:?} should be rejected: {other:?}"),
    }
}

fn help(args: &[&str]) -> String {
    match parsed(args) {
        Parsed::Help(text) => text,
        other => panic!("{args:?} should show the help: {other:?}"),
    }
}

/// `--dataset out` followed by `extra`.
fn with_dataset<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["--dataset", "out"];
    args.extend_from_slice(extra);
    args
}

#[test]
fn defaults_are_a_table_on_standard_output_without_limit() {
    let options = options(&with_dataset(&["--where", "ID=1"]));
    assert_eq!(options.dataset, PathBuf::from("out"));
    assert_eq!(options.conditions, [RawCondition::equal("ID", "1")]);
    assert_eq!(options.limit, None);
    assert_eq!(options.mode, Mode::Records(Format::Table));
    assert_eq!(options.output, None);
}

#[test]
fn every_option_is_honoured() {
    let options = options(&with_dataset(&[
        "--where",
        "ID=1",
        "--limit",
        "25",
        "--format",
        "csv",
        "--output",
        "result.csv",
    ]));
    assert_eq!(options.limit, NonZeroU64::new(25));
    assert_eq!(options.mode, Mode::Records(Format::Csv));
    assert_eq!(options.output, Some(PathBuf::from("result.csv")));
}

#[test]
fn formats_are_read_case_insensitively() {
    for (typed, format) in [
        ("table", Format::Table),
        ("csv", Format::Csv),
        ("json", Format::Json),
        ("JSON", Format::Json),
        ("Csv", Format::Csv),
    ] {
        let options = options(&with_dataset(&["--format", typed]));
        assert_eq!(options.mode, Mode::Records(format), "{typed}");
    }
    let json_file = options(&with_dataset(&["--format", "json", "--output", "r.json"]));
    assert_eq!(json_file.output, Some(PathBuf::from("r.json")));
}

#[test]
fn count_only_is_a_mode_of_its_own() {
    let options = options(&with_dataset(&["--count", "--limit", "3"]));
    assert_eq!(options.mode, Mode::Count);
    assert_eq!(options.limit, NonZeroU64::new(3));
}

#[test]
fn conditions_keep_the_command_line_order_and_split_at_the_first_separator() {
    let options = options(&with_dataset(&[
        "--range",
        "Date=2020-01-01..2020-01-31 10:00",
        "--where",
        "Primary Type=THEFT",
        "--where",
        "District=",
        "--range",
        "date=..2020-02-01",
        "--where",
        " IUCR =a=b",
        "--range",
        "Date=2020-03-01..",
    ]));
    assert_eq!(
        options.conditions,
        [
            RawCondition::range("Date", "2020-01-01", "2020-01-31 10:00"),
            RawCondition::equal("Primary Type", "THEFT"),
            RawCondition::equal("District", ""),
            RawCondition::range("date", "", "2020-02-01"),
            RawCondition::equal(" IUCR ", "a=b"),
            RawCondition::range("Date", "2020-03-01", ""),
        ]
    );
}

#[test]
fn a_query_without_conditions_is_a_valid_command_line() {
    // Rejecting it is a query rule (exit code 1), not a command-line one.
    assert!(options(&with_dataset(&[])).conditions.is_empty());
}

#[test]
fn a_condition_without_equals_sign_is_rejected() {
    assert_eq!(
        invalid(&with_dataset(&["--where", "Primary Type"])),
        "La condición \"Primary Type\" no tiene la forma CAMPO=VALOR."
    );
}

#[test]
fn a_range_without_equals_sign_or_dots_is_rejected() {
    for range in [
        "Date",
        "Date=2020-01-01",
        "Date 2020-01-01..2020-01-02",
        "..=x",
    ] {
        assert_eq!(
            invalid(&with_dataset(&["--range", range])),
            format!("El rango \"{range}\" no tiene la forma Date=DESDE..HASTA."),
        );
    }
}

#[test]
fn a_range_with_both_ends_empty_is_rejected() {
    assert_eq!(
        invalid(&with_dataset(&["--range", "Date=.."])),
        "El rango \"Date=..\" necesita al menos una fecha: Date=DESDE.., Date=..HASTA o \
         Date=DESDE..HASTA."
    );
}

#[test]
fn the_limit_must_be_a_positive_integer() {
    for limit in ["0", "-3", "abc", "1.5", "", "+"] {
        assert_eq!(
            invalid(&with_dataset(&["--limit", limit])),
            "El límite debe ser un número entero mayor que 0.",
            "{limit:?}"
        );
    }
}

#[test]
fn count_only_does_not_combine_with_a_format() {
    for format in ["table", "csv", "json"] {
        assert_eq!(
            invalid(&with_dataset(&["--count", "--format", format])),
            "\"Solo conteo\" no se combina con un formato de salida."
        );
    }
}

#[test]
fn the_output_file_only_admits_csv_or_json() {
    let message = "El archivo de salida solo admite los formatos CSV o JSON.";
    assert_eq!(invalid(&with_dataset(&["--output", "r.txt"])), message);
    assert_eq!(
        invalid(&with_dataset(&["--format", "table", "--output", "r.txt"])),
        message
    );
    assert_eq!(
        invalid(&with_dataset(&["--count", "--output", "r.txt"])),
        message
    );
}

#[test]
fn an_unknown_format_is_rejected() {
    assert_eq!(
        invalid(&with_dataset(&["--format", "xml"])),
        "El formato \"xml\" no es válido. Formatos admitidos: table, csv, json."
    );
}

#[test]
fn clap_errors_are_translated() {
    assert_eq!(
        invalid(&["--where", "ID=1"]),
        "Faltan opciones obligatorias: --dataset <CARPETA>. Usa --help para ver las opciones."
    );
    assert_eq!(
        invalid(&with_dataset(&["--sort", "ID"])),
        "Opción desconocida: --sort. Usa --help para ver las opciones."
    );
    assert_eq!(
        invalid(&with_dataset(&["--where"])),
        "Los argumentos no son válidos. Usa --help para ver las opciones."
    );
}

#[test]
fn the_help_is_in_spanish_and_ends_with_the_exit_codes() {
    let text = help(&["--help"]);
    assert_eq!(help(&["-h"]), text);
    assert_eq!(help(&["--dataset", "out", "--help"]), text);
    for expected in [
        "Uso: crime-query --dataset <CARPETA> [opciones]",
        "--where <CAMPO=VALOR>",
        "--range <Date=DESDE..HASTA>",
        "--limit <N>",
        "--count",
        "--format <FORMATO>",
        "--output <ARCHIVO>",
        "Muestra esta ayuda.",
        "Códigos de salida:",
        "El texto se compara sin distinguir mayúsculas",
        "CAMPO= (nada tras el =) pide \"sin valor\"",
        "Arrest y Domestic admiten true, false, sí, si o no.",
        "los avisos y las estadísticas, a la salida de errores",
        "se sobrescribe sin preguntar",
        "el archivo no se crea ni se modifica",
        ".<ARCHIVO>.XXXXXX.crime-query.tmp",
        "puede quedar ese archivo temporal; se puede borrar",
        "cancelado la consulta con Ctrl+C; no queda ningún archivo de salida a",
    ] {
        assert!(text.contains(expected), "{expected:?} in:\n{text}");
    }
    let codes = text
        .split("Códigos de salida:")
        .nth(1)
        .expect("the exit codes section");
    for code in ["  0 ", "  1 ", "  2 ", "  130 "] {
        assert!(codes.contains(code), "{code:?} in:\n{codes}");
    }
    assert!(text.ends_with('\n'));
}

// Prefix conditions (startswith PRD).

#[test]
fn startswith_conditions_keep_their_place_among_the_others() {
    let options = options(&with_dataset(&[
        "--startswith",
        "Primary Type=CRIM",
        "--where",
        "Primary Type=THEFT",
        "--range",
        "Date=2020-01-01..",
        "--startswith",
        " district =0 1=",
        "--startswith",
        "Case Number=",
    ]));
    assert_eq!(
        options.conditions,
        [
            RawCondition::starts_with("Primary Type", "CRIM"),
            RawCondition::equal("Primary Type", "THEFT"),
            RawCondition::range("Date", "2020-01-01", ""),
            RawCondition::starts_with(" district ", "0 1="),
            // An empty prefix is a query rule (exit code 1), not a command-line one.
            RawCondition::starts_with("Case Number", ""),
        ]
    );
}

#[test]
fn a_startswith_without_equals_sign_is_rejected() {
    assert_eq!(
        invalid(&with_dataset(&["--startswith", "Primary Type"])),
        "La condición \"Primary Type\" no tiene la forma CAMPO=PREFIJO."
    );
}

#[test]
fn the_help_describes_startswith_with_the_other_conditions() {
    let text = help(&["--help"]);
    for expected in [
        "--startswith <CAMPO=PREFIJO>",
        "Condición \"empieza por\"",
        "Case Number, Primary Type y District",
        "IUCR, Location Description, Beat y FBI Code",
    ] {
        assert!(text.contains(expected), "{expected:?} in:\n{text}");
    }
    let where_at = text.find("--where").expect("--where");
    let startswith_at = text.find("--startswith").expect("--startswith");
    let limit_at = text.find("--limit").expect("--limit");
    assert!(
        where_at < startswith_at && startswith_at < limit_at,
        "{text}"
    );
}
