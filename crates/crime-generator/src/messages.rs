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
