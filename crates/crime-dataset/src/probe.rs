//! Phase markers for the robustness scenarios (canopy-db-robustez PRD).
//!
//! The scenario runner kills an app, or presses Ctrl+C, in a given phase of its run, so it needs to
//! know which phase the app is in. Two environment variables ask for that; they appear in no help,
//! and with both unset [`mark`] does nothing, so a normal run does not change:
//!
//! - [`PHASE_FILE_ENV`] names a file where the app appends each phase it enters, one per line;
//! - [`PHASE_PAUSE_ENV`] holds `<phase>:<milliseconds>`: the app pauses that long right after it
//!   marks that phase, so that a short phase lasts long enough for a signal to land in it.
//!
//! The phases are the constants of [`phase`].

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

/// The environment variable that names the phase file.
pub const PHASE_FILE_ENV: &str = "CRIME_PHASE_FILE";

/// The environment variable that asks for a pause: `<phase>:<milliseconds>`.
pub const PHASE_PAUSE_ENV: &str = "CRIME_PHASE_PAUSE";

/// The phases the apps mark, in the order a run enters them.
pub mod phase {
    /// Generator: reading and sorting the CSV.
    pub const READ: &str = "read";
    /// Generator: the version folder exists and the primary table is being written.
    pub const PRIMARY_LOAD: &str = "primary-load";
    /// Generator: the indexes are being written.
    pub const INDEX_WRITE: &str = "index-write";
    /// Generator: the storage is being finished; for CanopyDB, the final dump of the base.
    pub const DUMP: &str = "dump";
    /// Generator: the manifest is being written.
    pub const MANIFEST: &str = "manifest";
    /// Generator: the temporary pointer is written and `CURRENT` is not replaced yet.
    pub const CURRENT_TMP: &str = "current-tmp";
    /// Generator: `CURRENT` names the new version and the previous one is not deleted yet.
    pub const SWITCHED: &str = "switched";
    /// CanopyDB query app: the base is open (its lock is held) and its tables are being opened.
    pub const OPENED: &str = "opened";
    /// Query apps: a query starts; a batch marks it once per query.
    pub const QUERY: &str = "query";
}

/// What the environment asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Probe {
    file: Option<PathBuf>,
    pause: Option<(String, Duration)>,
}

impl Probe {
    fn from_env() -> Probe {
        Probe {
            file: std::env::var_os(PHASE_FILE_ENV).map(PathBuf::from),
            pause: std::env::var(PHASE_PAUSE_ENV)
                .ok()
                .and_then(|text| parse_pause(&text)),
        }
    }

    fn mark(&self, phase: &str) {
        if let Some(path) = &self.file {
            // Best effort: a phase file that cannot be written never fails the run.
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(file, "{phase}");
            }
        }
        if let Some((name, pause)) = &self.pause
            && name == phase
        {
            thread::sleep(*pause);
        }
    }
}

/// Marks that the app enters `phase` (see the module documentation).
pub fn mark(phase: &str) {
    static PROBE: OnceLock<Probe> = OnceLock::new();
    PROBE.get_or_init(Probe::from_env).mark(phase);
}

/// Reads `<phase>:<milliseconds>`; anything else asks for no pause.
fn parse_pause(text: &str) -> Option<(String, Duration)> {
    let (phase, millis) = text.rsplit_once(':')?;
    let millis = millis.trim().parse::<u64>().ok()?;
    let phase = phase.trim();
    (!phase.is_empty()).then(|| (phase.to_owned(), Duration::from_millis(millis)))
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn a_pause_names_a_phase_and_its_milliseconds() {
        assert_eq!(
            parse_pause("dump:1500"),
            Some(("dump".to_owned(), Duration::from_millis(1500)))
        );
        assert_eq!(parse_pause("dump"), None);
        assert_eq!(parse_pause("dump:soon"), None);
        assert_eq!(parse_pause(":10"), None);
    }

    #[test]
    fn marking_appends_each_phase_and_pauses_only_at_its_phase() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("phases");
        let probe = Probe {
            file: Some(file.clone()),
            pause: Some((phase::DUMP.to_owned(), Duration::from_millis(300))),
        };
        let started = Instant::now();
        probe.mark(phase::READ);
        assert!(started.elapsed() < Duration::from_millis(300));
        probe.mark(phase::DUMP);
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "read\ndump\n");
    }

    #[test]
    fn without_the_environment_marking_does_nothing() {
        let probe = Probe::default();
        let started = Instant::now();
        probe.mark(phase::DUMP);
        assert!(started.elapsed() < Duration::from_millis(50));
    }
}
