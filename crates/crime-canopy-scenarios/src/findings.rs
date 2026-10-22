//! The identifiers of the findings log (`## Hn — …` headings of `docs/canopy-db/hallazgos.md`),
//! which a `NOT MET` association must name.

use std::fs;
use std::io;
use std::path::Path;

/// The finding identifiers of the log at `path`, in order.
pub fn read(path: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "no se pudo leer el registro de hallazgos {}: {error}",
                path.display()
            ),
        )
    })?;
    Ok(identifiers(&text))
}

fn identifiers(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("## H"))
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then(|| format!("H{digits}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_findings_are_the_numbered_second_level_headings() {
        let text = "# Registro\n## Resumen\n## H1 — Uno\n### H9 no\n## H12 — Doce\n";
        assert_eq!(identifiers(text), vec!["H1", "H12"]);
    }
}
