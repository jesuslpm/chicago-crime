//! Every user-facing text of the generator, in Spanish exactly as the PRDs specify.
//!
//! Keeping them together lets later stages reuse the same wording and keeps the tests honest.
//! Row numbers are written plainly (`Fila 1234`) and record counts with Spanish thousands
//! separators (`8.656.904`), as in the PRD examples.

use std::fmt::Display;
use std::path::Path;

use crime_dataset::{RecordCounts, SourceInfo};

// Run aborts.

pub fn input_unreadable(path: &Path) -> String {
    format!("No se puede leer el archivo {}.", path.display())
}

pub fn temp_dir_unusable(dir: &Path) -> String {
    format!("No se puede usar la carpeta temporal {}.", dir.display())
}

pub fn log_unwritable(path: &Path) -> String {
    format!("No se puede escribir el log en {}.", path.display())
}

/// `column` is 1-based.
pub fn header_mismatch(column: usize, expected: &str, found: &str) -> String {
    format!(
        "La cabecera no coincide con el esquema: columna {column} esperaba \"{expected}\" y \
         encontró \"{found}\"."
    )
}

pub fn dataset_exists() -> String {
    "La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para \
     reemplazarlo."
        .to_owned()
}

pub fn output_unwritable(dir: &Path) -> String {
    format!(
        "No se puede escribir en la carpeta de salida {}.",
        dir.display()
    )
}

pub fn insufficient_space(dir: &Path) -> String {
    format!("Espacio insuficiente en {}.", dir.display())
}

/// A table received a key that is not greater than the previous one: a defect of the generator.
pub fn unordered_keys(file: &Path) -> String {
    format!(
        "Error interno: las claves de {} no llegan en orden estrictamente creciente.",
        file.display()
    )
}

/// A value of the primary table could not be decoded to build its index entries: a defect of
/// the generator.
pub fn undecodable_record(id: i64) -> String {
    format!(
        "Error interno: el registro con ID {id} de la tabla principal no se puede decodificar \
         para generar los índices."
    )
}

/// An index does not hold one entry per valid record: a defect of the generator.
pub fn index_count_mismatch(index: &str, entries: u64, valid: u64) -> String {
    format!(
        "Error interno: el índice {index} tiene {} entradas, pero la tabla principal tiene {} \
         registros válidos.",
        thousands(entries),
        thousands(valid)
    )
}

/// Another run holds the lock of the output folder.
pub fn output_locked(dir: &Path) -> String {
    format!(
        "Otra generación está usando la carpeta de salida {}.",
        dir.display()
    )
}

/// The output folder holds `entry`, which no generation wrote: it is never replaced or deleted.
pub fn foreign_content(dir: &Path, entry: &str) -> String {
    format!(
        "La carpeta de salida {} contiene \"{entry}\", que no forma parte de un conjunto de \
         datos. Usa una carpeta vacía o una que solo contenga un conjunto generado.",
        dir.display()
    )
}

/// Logged after the message of any abort: the previous dataset, if any, is intact.
pub fn generation_cancelled() -> String {
    "Generación cancelada. No se ha modificado el conjunto existente.".to_owned()
}

// Publication.

pub fn leftovers_discarded() -> String {
    "Se han descartado restos de una generación interrumpida anterior.".to_owned()
}

/// A leftover of an interrupted run could not be deleted; the next run tries again.
pub fn leftover_kept(path: &Path) -> String {
    format!(
        "Aviso: no se ha podido eliminar el resto {}; se reintentará en la próxima generación.",
        path.display()
    )
}

/// The CSV has the header and no rows at all: an empty dataset is published.
pub fn empty_csv() -> String {
    "Aviso: el CSV no contiene registros; el conjunto generado está vacío.".to_owned()
}

/// The new dataset is published, but the folder of the replaced version could not be deleted
/// (a reader may still hold its files); the next run discards it as a leftover.
pub fn previous_version_kept(dir: &Path) -> String {
    format!(
        "Aviso: no se ha podido eliminar la versión anterior {}; se descartará en la próxima \
         generación.",
        dir.display()
    )
}

// Rejected rows. `line` is the line of the file where the row starts.

pub fn wrong_column_count(line: u64, expected: usize, found: usize) -> String {
    format!("Fila {line} omitida: se esperaban {expected} columnas y hay {found}.")
}

pub fn invalid_id(line: u64, value: &str) -> String {
    format!("Fila {line} omitida: el ID \"{value}\" no es un número entero.")
}

pub fn missing_required(line: u64, column: &str) -> String {
    format!("Fila {line} omitida: la columna \"{column}\" es obligatoria y está vacía.")
}

pub fn invalid_date(line: u64, column: &str, value: &str) -> String {
    format!(
        "Fila {line} omitida, columna \"{column}\": \"{value}\" no es una fecha válida \
         (formato MM/DD/AAAA hh:mm:ss AM/PM)."
    )
}

pub fn invalid_flag(line: u64, column: &str, value: &str) -> String {
    format!(
        "Fila {line} omitida, columna \"{column}\": \"{value}\" no es un valor Sí/No válido \
         (true/false)."
    )
}

pub fn invalid_integer(line: u64, column: &str, value: &str) -> String {
    format!("Fila {line} omitida, columna \"{column}\": \"{value}\" no es un número entero.")
}

pub fn invalid_decimal(line: u64, column: &str, value: &str) -> String {
    format!(
        "Fila {line} omitida, columna \"{column}\": \"{value}\" no es un número decimal válido."
    )
}

pub fn invalid_utf8(line: u64, column: &str) -> String {
    format!("Fila {line} omitida, columna \"{column}\": el texto no es UTF-8 válido.")
}

/// A repeated ID: the row at `first_line` is kept (decision D5). IDs are written plainly, like
/// row numbers.
pub fn duplicate_id(line: u64, id: i64, first_line: u64) -> String {
    format!("Fila {line} omitida: el ID {id} ya apareció en la fila {first_line}.")
}

/// Log line of a rejected row: the reason followed by the original row as a CSV line.
pub fn rejected_row(reason: &str, raw: &str) -> String {
    format!("{reason} Contenido original: {raw}")
}

// Progress and results.

pub fn reading_completed(counts: RecordCounts) -> String {
    format!(
        "Lectura completada: {} registros válidos, {} rechazados.",
        thousands(counts.valid),
        thousands(counts.rejected)
    )
}

pub fn source_fingerprint(source: &SourceInfo) -> String {
    format!(
        "Huella SHA-256 de {}: {} ({} bytes).",
        source.file_name,
        source.sha256,
        thousands(source.size_bytes)
    )
}

pub fn sort_completed(segments: usize) -> String {
    format!("Ordenación completada. Segmentos temporales en disco: {segments}.")
}

pub fn index_sort_completed(segments: usize) -> String {
    format!("Ordenación de los índices completada. Segmentos temporales en disco: {segments}.")
}

/// Entries per index, as `(index name, entries)` in index order.
pub fn indexes_generated(counts: &[(&str, u64)]) -> String {
    let counts: Vec<String> = counts
        .iter()
        .map(|(index, entries)| format!("{index} {}", thousands(*entries)))
        .collect();
    format!("Índices generados: {}.", counts.join(", "))
}

/// Final count: duplicates are included in the rejected records.
pub fn generation_completed(counts: RecordCounts) -> String {
    format!(
        "Generación completada: {} registros válidos, {} rechazados (ver log).",
        thousands(counts.valid),
        thousands(counts.rejected)
    )
}

// Command line.

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

/// Formats a count with Spanish thousands separators: `8656904` becomes `8.656.904`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            formatted.push('.');
        }
        formatted.push(digit);
    }
    formatted
}
