//! The messages only the CanopyDB query app has (canopy-db-consultor PRD, "Validaciones y
//! mensajes"). Every other message is the original query app's (decision K1).

use std::path::Path;
use std::time::Duration;

/// Another process has the base open: the query waits up to `wait` (decision KC2).
pub fn waiting(folder: &Path, wait: Duration) -> String {
    format!(
        "Aviso: otro proceso tiene abierto el conjunto de datos en {}; se esperará hasta {} s.",
        folder.display(),
        wait.as_secs()
    )
}

/// The base is still open by another process after the wait (decision KC2).
pub fn still_open(folder: &Path) -> String {
    format!(
        "El conjunto de datos en {} sigue abierto por otro proceso. Inténtalo de nuevo cuando \
         termine.",
        folder.display()
    )
}

/// The base was written with another version of CanopyDB (decision KC4). Each version is
/// `engine.version`, followed by ` (fork <commit>)` when there is a fork commit.
pub fn other_version(dataset: &str, own: &str) -> String {
    format!(
        "El conjunto se escribió con CanopyDB {dataset} y este consultor usa CanopyDB {own}. \
         Regenera el conjunto."
    )
}

/// A CanopyDB version as the messages show it.
pub fn version_text(version: &str, fork_commit: Option<&str>) -> String {
    match fork_commit {
        Some(commit) => format!("{version} (fork {commit})"),
        None => version.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_the_prd_texts() {
        let folder = Path::new(r"C:\datos\salida");
        assert_eq!(
            waiting(folder, Duration::from_secs(30)),
            r"Aviso: otro proceso tiene abierto el conjunto de datos en C:\datos\salida; se esperará hasta 30 s."
        );
        assert_eq!(
            still_open(folder),
            r"El conjunto de datos en C:\datos\salida sigue abierto por otro proceso. Inténtalo de nuevo cuando termine."
        );
        assert_eq!(
            other_version("0.2.4", "0.2.5"),
            "El conjunto se escribió con CanopyDB 0.2.4 y este consultor usa CanopyDB 0.2.5. \
             Regenera el conjunto."
        );
        assert_eq!(version_text("0.2.5", None), "0.2.5");
        assert_eq!(
            version_text("0.2.5", Some("0123456789abcdef")),
            "0.2.5 (fork 0123456789abcdef)"
        );
    }
}
