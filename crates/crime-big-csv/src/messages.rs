//! Every user-facing text of the big CSV tool, in Spanish. Record counts use Spanish thousands
//! separators (`8.656.904`), as the generator's messages do.

use std::path::Path;

pub fn written(records: u64, output: &Path) -> String {
    format!(
        "Se han escrito {} registros en {}.",
        thousands(records),
        output.display()
    )
}

pub fn output_exists(path: &Path) -> String {
    format!(
        "El archivo {} ya existe; no se sobrescribe.",
        path.display()
    )
}

/// The same wording as the generator's unreadable input.
pub fn input_unreadable(path: &Path) -> String {
    format!("No se puede leer el archivo {}.", path.display())
}

pub fn header_mismatch(path: &Path) -> String {
    format!(
        "El archivo {} no tiene la cabecera esperada del CSV de delitos de Chicago.",
        path.display()
    )
}

pub fn invalid_record(path: &Path, line: u64) -> String {
    format!(
        "El archivo {} no es un CSV válido en la línea {line}.",
        path.display()
    )
}

pub fn ids_exhausted() -> String {
    "No quedan ID libres por encima del mayor ID del archivo; no se ha escrito el CSV grande."
        .to_owned()
}

pub fn case_numbers_exhausted(prefix: &str) -> String {
    format!(
        "No quedan números de caso libres que empiecen por {prefix}; no se ha escrito el CSV grande."
    )
}

pub fn output_unwritable(path: &Path) -> String {
    format!("No se puede escribir el archivo {}.", path.display())
}

/// Formats a count with Spanish thousands separators: `8656904` becomes `8.656.904`. A copy of the
/// generator's own (`crime_generator::messages::thousands`), so this tool does not depend on the
/// generator.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_uses_dots() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1.000");
        assert_eq!(thousands(25_970_712), "25.970.712");
    }
}
