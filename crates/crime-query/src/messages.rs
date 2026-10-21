//! Every user-facing text of the query app, in Spanish exactly as the PRDs specify.
//!
//! Keeping them together lets later stages reuse the same wording and keeps the tests honest.
//! A folder is shown exactly as the analyst gave it.

use std::fmt::Display;
use std::path::Path;
use std::time::Duration;

use crime_dataset::SCHEMA_VERSION;

use crate::Field;

// Opening and reading the dataset (child PRD, flow step 1 and "Aborta sin salida parcial
// engañosa").

pub fn dataset_not_found(folder: &Path) -> String {
    format!(
        "No se encontró un conjunto de datos válido en {}.",
        folder.display()
    )
}

/// `found` is the schema version the manifest declares; the query app reads [`SCHEMA_VERSION`].
pub fn incompatible_schema(found: u32) -> String {
    format!(
        "El conjunto usa el esquema v{found} y este consultor admite v{SCHEMA_VERSION}. Regenera \
         el conjunto."
    )
}

pub fn dataset_unreadable(folder: &Path) -> String {
    format!(
        "No se pudo leer el conjunto de datos en {}.",
        folder.display()
    )
}

// Query conditions (exit code 1: "datos de la consulta", decision C6). `<campo>` and `<nombre>`
// are the catalog name of a known field and the name as typed for an unknown one.

pub fn no_indexed_condition() -> String {
    "Indica al menos una condición sobre un campo indexado: ID, número de caso, fecha, tipo \
     principal, distrito o área comunitaria."
        .to_owned()
}

pub fn unknown_field(name: &str) -> String {
    format!(
        "El campo \"{name}\" no existe. Campos disponibles: {}",
        available_fields()
    )
}

/// Block, Description and Location (decision C3).
pub fn field_not_conditionable(name: &str) -> String {
    format!(
        "El campo \"{name}\" no admite condiciones. Campos disponibles: {}",
        available_fields()
    )
}

pub fn mandatory_field(name: &str) -> String {
    format!("El campo \"{name}\" es obligatorio; no admite \"sin valor\".")
}

pub fn expected_integer(value: &str, field: &str) -> String {
    format!("\"{value}\" no es válido para \"{field}\": se esperaba un número entero.")
}

/// Every spelling a boolean condition accepts, case-insensitive (`si` is accepted too).
pub fn expected_boolean(value: &str, field: &str) -> String {
    format!("\"{value}\" no es válido para \"{field}\": se esperaba true, false, sí o no.")
}

pub fn expected_decimal(value: &str, field: &str) -> String {
    format!("\"{value}\" no es válido para \"{field}\": se esperaba un número decimal.")
}

pub fn range_not_on_date() -> String {
    "Solo se admiten rangos sobre \"Date\".".to_owned()
}

pub fn invalid_date(value: &str) -> String {
    format!("\"{value}\" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].")
}

pub fn inverted_range() -> String {
    "La fecha inicial es posterior a la final.".to_owned()
}

/// `--startswith CAMPO=` with nothing after the `=` (startswith PRD, decision P6).
pub fn empty_prefix(field: &str) -> String {
    format!("El prefijo de \"{field}\" no puede estar vacío.")
}

/// `--startswith` on a field that is not text: ID, dates, numbers, Arrest, Domestic,
/// coordinates.
pub fn prefix_not_on_text() -> String {
    "\"Empieza por\" solo se admite en campos de texto: Case Number, Primary Type, District, \
     IUCR, Location Description, Beat y FBI Code."
        .to_owned()
}

/// The conditionable fields by CSV name, in CSV order, ending with a period.
fn available_fields() -> String {
    let names: Vec<&str> = Field::conditionable().map(Field::name).collect();
    format!("{}.", names.join(", "))
}

// Running the query: warnings, the stats line (stderr) and cancellation.

/// A requested ID with no record; not an error (the query completes).
pub fn missing_id(id: i64) -> String {
    format!("No existe ningún delito con ID {id}.")
}

/// A requested case number with no record, as the analyst typed it; not an error.
pub fn missing_case_number(typed: &str) -> String {
    format!("No existe ningún delito con número de caso {typed}.")
}

/// The line after a completed query (decision C5, flow step 7): the number of results with
/// Spanish thousands separators, the time in seconds with three decimals and a decimal comma,
/// the CSV names of the indexed conditions and of the fields filtered without an index.
pub fn stats(count: u64, elapsed: Duration, indexes: &[&str], filters: &[&str]) -> String {
    let results = if count == 1 {
        "resultado"
    } else {
        "resultados"
    };
    let filters = if filters.is_empty() {
        "ninguno".to_owned()
    } else {
        filters.join(", ")
    };
    format!(
        "{} {results} en {} s. Índices usados: {}. Filtros sin índice: {filters}.",
        thousands(u128::from(count)),
        seconds(elapsed),
        indexes.join(", ")
    )
}

/// `elapsed` in seconds with three decimals, a decimal comma and Spanish thousands separators,
/// rounded to the millisecond: `0,041`, `1.234,500`.
fn seconds(elapsed: Duration) -> String {
    let millis = (elapsed.as_micros() + 500) / 1_000;
    format!("{},{:03}", thousands(millis / 1_000), millis % 1_000)
}

pub fn query_cancelled() -> String {
    "Consulta cancelada.".to_owned()
}

/// Ctrl+C could not be wired: the query runs, but cannot be cancelled cleanly.
pub fn cancel_unavailable() -> String {
    "Aviso: no se pudo activar la cancelación con Ctrl+C.".to_owned()
}

// Writing the results (decisions C5 and C8). The path is shown exactly as the analyst gave it.

/// The output file cannot be created or written, or cannot replace the target.
pub fn output_unwritable(path: &Path) -> String {
    format!(
        "No se puede escribir el archivo de salida {}.",
        path.display()
    )
}

/// Writing the results to stdout failed for a reason other than a closed pipe.
pub fn stdout_unwritable() -> String {
    "No se puede escribir en la salida estándar.".to_owned()
}

// Query batches (`--batch`, canopy-db-lote PRD). The batch file and the output paths are shown
// exactly as the analyst wrote them; line and query numbers go without thousands separators.

/// A query option typed on the command line together with `--batch`.
pub fn option_with_batch(option: &str) -> String {
    format!(
        "Con --batch, la opción {option} va en cada línea del lote, no en la línea de comandos."
    )
}

pub fn batch_unreadable(file: &Path) -> String {
    format!("No se puede leer el lote {}.", file.display())
}

pub fn batch_not_utf8(file: &Path) -> String {
    format!("El lote {} no es un texto UTF-8 válido.", file.display())
}

/// The file has only empty lines and comments, or nothing at all.
pub fn batch_empty(file: &Path) -> String {
    format!("El lote {} no contiene ninguna consulta.", file.display())
}

/// A message about line `number` of the batch file: a batch rule or a single query's message,
/// unchanged.
pub fn on_line(number: usize, message: impl Display) -> String {
    format!("Línea {number}: {message}")
}

pub fn unclosed_quote() -> String {
    "falta cerrar unas comillas.".to_owned()
}

/// `--dataset`, `--batch`, `--help` or `-h` inside a batch line.
pub fn option_not_in_batch(option: &str) -> String {
    format!("la opción {option} no se admite dentro del lote.")
}

/// An output file that an earlier line of the batch already writes.
pub fn output_reused(path: &Path, earlier_line: usize) -> String {
    format!(
        "el archivo de salida {} ya lo usa la línea {earlier_line}.",
        path.display()
    )
}

pub fn output_is_batch(path: &Path) -> String {
    format!("el archivo de salida {} es el propio lote.", path.display())
}

/// The header written on stderr before query `index` (from 1) of `total`, on line `line`.
pub fn batch_header(index: usize, total: usize, line: usize) -> String {
    format!("--- Consulta {index} de {total} (línea {line}) ---")
}

/// The last line of a batch whose queries all ran; the time covers the whole run.
pub fn batch_completed(total: usize, elapsed: Duration) -> String {
    let queries = if total == 1 { "consulta" } else { "consultas" };
    format!(
        "Lote completado: {} {queries} en {} s.",
        thousands(total as u128),
        seconds(elapsed)
    )
}

/// `1838502` as `1.838.502`.
fn thousands(value: u128) -> String {
    let digits = value.to_string();
    let mut text = String::with_capacity(digits.len() + digits.len() / 3);
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position).is_multiple_of(3) {
            text.push('.');
        }
        text.push(digit);
    }
    text
}

// Command line (exit code 2).

pub fn malformed_condition(text: &str) -> String {
    format!("La condición \"{text}\" no tiene la forma CAMPO=VALOR.")
}

pub fn malformed_prefix(text: &str) -> String {
    format!("La condición \"{text}\" no tiene la forma CAMPO=PREFIJO.")
}

pub fn malformed_range(text: &str) -> String {
    format!("El rango \"{text}\" no tiene la forma Date=DESDE..HASTA.")
}

pub fn empty_range(text: &str) -> String {
    format!(
        "El rango \"{text}\" necesita al menos una fecha: Date=DESDE.., Date=..HASTA o \
         Date=DESDE..HASTA."
    )
}

pub fn invalid_limit() -> String {
    "El límite debe ser un número entero mayor que 0.".to_owned()
}

pub fn count_with_format() -> String {
    "\"Solo conteo\" no se combina con un formato de salida.".to_owned()
}

pub fn output_file_format() -> String {
    "El archivo de salida solo admite los formatos CSV o JSON.".to_owned()
}

pub fn invalid_format(value: &str) -> String {
    format!("El formato \"{value}\" no es válido. Formatos admitidos: table, csv, json.")
}

pub fn missing_options(options: &[String]) -> String {
    format!(
        "Faltan opciones obligatorias: {}.{}",
        options.join(", "),
        help_hint()
    )
}

pub fn unknown_option(option: impl Display) -> String {
    format!("Opción desconocida: {option}.{}", help_hint())
}

pub fn invalid_arguments() -> String {
    format!("Los argumentos no son válidos.{}", help_hint())
}

fn help_hint() -> &'static str {
    " Usa --help para ver las opciones."
}
