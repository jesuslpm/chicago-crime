//! What every scenario shares: the binaries, the reference CSV, the work folder, the reference
//! dataset (generated once and copied for each variant), the fixed query set (KC7) and the
//! results of clean runs (canopy-db-robustez PRD, "Reglas comunes").

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::process::{Launch, run_to_end};

/// The longest a generation of the reference CSV may take before a scenario gives up on it.
pub const GENERATION_TIMEOUT: Duration = Duration::from_secs(20 * 60);

/// The longest a query may take before a scenario gives up on it.
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// The minimum number of repetitions of a racy scenario (PRD, "Reglas comunes").
pub const MIN_REPETITIONS: usize = 5;

/// The binaries the scenarios run, all built next to this one.
#[derive(Debug, Clone)]
pub struct Binaries {
    pub generator: PathBuf,
    pub query: PathBuf,
    pub original_generator: PathBuf,
    pub original_query: PathBuf,
    /// `crime-canopy-acceptance`, for `--compare`.
    pub canopy_acceptance: PathBuf,
    /// `crime-acceptance`, for `select`: the direct filter of a CSV.
    pub acceptance: PathBuf,
    pub big_csv: PathBuf,
}

impl Binaries {
    /// The binaries in `dir`.
    pub fn in_dir(dir: &Path) -> Self {
        let bin = |name: &str| dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        Self {
            generator: bin("crime-canopy-generator"),
            query: bin("crime-canopy-query"),
            original_generator: bin("crime-generator"),
            original_query: bin("crime-query"),
            canopy_acceptance: bin("crime-canopy-acceptance"),
            acceptance: bin("crime-acceptance"),
            big_csv: bin("crime-big-csv"),
        }
    }

    /// The binaries that are missing; the big CSV tool only matters when `big` (R13 runs).
    pub fn missing(&self, big: bool) -> Vec<PathBuf> {
        [
            &self.generator,
            &self.query,
            &self.original_generator,
            &self.original_query,
            &self.canopy_acceptance,
            &self.acceptance,
        ]
        .into_iter()
        .chain(big.then_some(&self.big_csv))
        .filter(|path| !path.is_file())
        .cloned()
        .collect()
    }
}

/// The context of a run of the scenarios.
#[derive(Debug)]
pub struct Context {
    pub bins: Binaries,
    /// The reference CSV.
    pub csv: PathBuf,
    /// The work folder: the reference dataset, the copies and the runs live here.
    pub work: PathBuf,
    /// Repetitions of each racy scenario (at least [`MIN_REPETITIONS`]).
    pub repetitions: usize,
    /// The fixed query set, F1 to F11, with `<número>` already replaced.
    fixed: Vec<String>,
    runs: AtomicUsize,
    clean: Mutex<HashMap<Vec<String>, CleanRun>>,
}

/// A clean run of a query: alone, with no interruption and no other process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanRun {
    pub code: Option<i32>,
    pub stdout: String,
}

impl Context {
    /// A context over `csv` and `work`, with the fixed query set read from `fixed_queries`.
    pub fn new(
        bins: Binaries,
        csv: &Path,
        work: &Path,
        fixed_queries: &Path,
        repetitions: usize,
    ) -> io::Result<Self> {
        fs::create_dir_all(work)?;
        fs::create_dir_all(work.join("tmp"))?;
        let mut context = Self {
            bins,
            csv: csv.to_path_buf(),
            work: work.to_path_buf(),
            repetitions: repetitions.max(MIN_REPETITIONS),
            fixed: Vec::new(),
            runs: AtomicUsize::new(0),
            clean: Mutex::new(HashMap::new()),
        };
        let case = context.shared_case_number()?;
        context.fixed = read_fixed_queries(fixed_queries)?
            .into_iter()
            .map(|line| line.replace("<número>", &case))
            .collect();
        Ok(context)
    }

    /// The fixed query set: F1 is `fixed(1)`, as one batch line.
    pub fn fixed_line(&self, number: usize) -> &str {
        &self.fixed[number - 1]
    }

    /// The arguments of the fixed query `number` (1 to 11) for a single query.
    pub fn fixed_args(&self, number: usize) -> Vec<String> {
        split_line(self.fixed_line(number))
    }

    /// The whole fixed query set, as a batch file's text.
    pub fn fixed_batch_text(&self) -> String {
        let mut text = self.fixed.join("\n");
        text.push('\n');
        text
    }

    /// A new folder for one run of an app: its phase file, stdout and stderr.
    pub fn run_dir(&self, label: &str) -> PathBuf {
        let number = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        let safe: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.work.join("runs").join(format!("{number:04}-{safe}"))
    }

    /// The temporary folder every generator of the scenarios uses.
    pub fn temp_dir(&self) -> PathBuf {
        self.work.join("tmp")
    }

    /// A generation of `input` into `output` with the CanopyDB generator.
    pub fn generation(&self, input: &Path, output: &Path, overwrite: bool, label: &str) -> Launch {
        let run_dir = self.run_dir(label);
        let mut launch = Launch::new(&self.bins.generator, &run_dir)
            .arg("--input")
            .arg(input)
            .arg("--output")
            .arg(output)
            .arg("--temp-dir")
            .arg(self.temp_dir())
            .arg("--log")
            .arg(run_dir.join("generator.log"));
        if overwrite {
            launch = launch.arg("--overwrite");
        }
        launch
    }

    /// A query of the CanopyDB query app over `dataset` with `args`.
    pub fn query<S: AsRef<str>>(&self, dataset: &Path, args: &[S], label: &str) -> Launch {
        Launch::new(&self.bins.query, &self.run_dir(label))
            .arg("--dataset")
            .arg(dataset)
            .args(args.iter().map(|arg| arg.as_ref().to_owned()))
    }

    /// A batch of the CanopyDB query app over `dataset` with the lines `text`.
    pub fn batch(&self, dataset: &Path, text: &str, label: &str) -> io::Result<Launch> {
        let run_dir = self.run_dir(label);
        fs::create_dir_all(&run_dir)?;
        let file = run_dir.join("batch.txt");
        fs::write(&file, text)?;
        Ok(Launch::new(&self.bins.query, &run_dir)
            .arg("--dataset")
            .arg(dataset)
            .arg("--batch")
            .arg(file))
    }

    /// The output folder of the reference dataset, generated once from the reference CSV.
    pub fn reference_dataset(&self) -> io::Result<PathBuf> {
        let output = self.work.join("reference").join("out");
        if output.join("CURRENT").is_file() {
            return Ok(output);
        }
        if output.exists() {
            fs::remove_dir_all(&output)?;
        }
        progress("generando el conjunto de referencia (una vez)");
        let launch = self.generation(&self.csv, &output, false, "reference");
        let (code, _, stderr) = run_to_end(launch, GENERATION_TIMEOUT)?;
        if code != Some(0) {
            return Err(io::Error::other(format!(
                "el generador CanopyDB no generó el conjunto de referencia (código {code:?}): {stderr}"
            )));
        }
        Ok(output)
    }

    /// A fresh copy of the reference dataset in `<work>/copies/<name>/out`, or an empty place
    /// for one when `with_dataset` is false. Returns the output folder.
    pub fn fresh_output(&self, name: &str, with_dataset: bool) -> io::Result<PathBuf> {
        let root = self.work.join("copies").join(name);
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        let output = root.join("out");
        if with_dataset {
            copy_dir(&self.reference_dataset()?, &output)?;
        }
        Ok(output)
    }

    /// The result of a clean run of the query `args` over the reference dataset, run once and
    /// remembered.
    pub fn clean_run(&self, args: &[String]) -> io::Result<CleanRun> {
        if let Some(known) = self.clean.lock().expect("not poisoned").get(args) {
            return Ok(known.clone());
        }
        let reference = self.reference_dataset()?;
        let (code, stdout, _) = run_to_end(self.query(&reference, args, "clean"), QUERY_TIMEOUT)?;
        let run = CleanRun { code, stdout };
        self.clean
            .lock()
            .expect("not poisoned")
            .insert(args.to_vec(), run.clone());
        Ok(run)
    }

    /// Compares the entries of two output folders with the CanopyDB verifier (`--compare`,
    /// decision KG8): `Ok(())` when they are the same, `Err` with its report otherwise.
    pub fn compare(&self, a: &Path, b: &Path) -> io::Result<Result<(), String>> {
        let launch = Launch::new(&self.bins.canopy_acceptance, &self.run_dir("compare"))
            .arg("--compare")
            .arg(a)
            .arg(b);
        let (code, stdout, stderr) = run_to_end(launch, GENERATION_TIMEOUT)?;
        Ok(if code == Some(0) {
            Ok(())
        } else {
            Err(format!("{stdout}{stderr}").trim().to_owned())
        })
    }

    /// The first case number that a later row of the reference CSV repeats (`<número>` of F2),
    /// found once and kept in the work folder.
    pub fn shared_case_number(&self) -> io::Result<String> {
        let kept = self.work.join("shared-case-number.txt");
        if let Ok(text) = fs::read_to_string(&kept)
            && !text.trim().is_empty()
        {
            return Ok(text.trim().to_owned());
        }
        progress("buscando el primer número de caso repetido del CSV (una vez)");
        let case = first_repeated_case_number(&self.csv)?;
        fs::write(&kept, &case)?;
        Ok(case)
    }
}

/// The version that a JSON output of a query names (`dataset.version`).
pub fn json_version(stdout: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(stdout).ok()?;
    value
        .get("dataset")?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

/// The version folder `CURRENT` names in `output`, if any.
pub fn current_version(output: &Path) -> Option<String> {
    fs::read_to_string(output.join("CURRENT"))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|name| !name.is_empty())
}

/// The version folders in `output` (named `dataset-…`), sorted.
pub fn version_folders(output: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(output)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(crime_dataset::VERSION_DIR_PREFIX))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// The entries of `dir` whose name starts with `prefix`, sorted.
pub fn entries_starting_with(dir: &Path, prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(prefix))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// The number of results a count prints (its stdout), if it is a number.
pub fn count_of(stdout: &str) -> Option<u64> {
    stdout.trim().replace('.', "").parse().ok()
}

/// Copies `from` into `to`, recursively.
pub fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let path = entry?.path();
        let target = to.join(path.file_name().expect("an entry has a name"));
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// A progress line on stderr.
pub fn progress(text: &str) {
    eprintln!("· {text}");
}

/// Splits a batch line into its arguments: words separated by spaces, double quotes group.
pub fn split_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ' ' | '\t' if !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(current);
    }
    args
}

fn read_fixed_queries(path: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "no se pudo leer el conjunto fijo de consultas {}: {error}",
                path.display()
            ),
        )
    })?;
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
    if lines.len() != 11 {
        return Err(io::Error::other(format!(
            "el conjunto fijo de consultas {} tiene {} consultas y no 11",
            path.display(),
            lines.len()
        )));
    }
    Ok(lines)
}

fn first_repeated_case_number(csv_path: &Path) -> io::Result<String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)
        .map_err(io::Error::other)?;
    // Hashes, not the texts: a set of 8.6 million strings would weigh several hundred MiB.
    let mut seen: HashSet<u64> = HashSet::new();
    let mut record = csv::StringRecord::new();
    while reader.read_record(&mut record).map_err(io::Error::other)? {
        let Some(case) = record.get(1) else { continue };
        let mut hasher = DefaultHasher::new();
        case.hash(&mut hasher);
        if !seen.insert(hasher.finish()) {
            return Ok(case.to_owned());
        }
    }
    Err(io::Error::other("el CSV no repite ningún número de caso"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_line_splits_into_its_arguments() {
        assert_eq!(
            split_line(
                r#"--where "Primary Type=THEFT" --range Date=2019-08-01..2019-08-31 --count"#
            ),
            vec![
                "--where",
                "Primary Type=THEFT",
                "--range",
                "Date=2019-08-01..2019-08-31",
                "--count"
            ]
        );
    }

    #[test]
    fn the_version_of_a_json_output_is_its_dataset_version() {
        let json = r#"{"dataset":{"version":"dataset-20261022-101010-7"},"results":[]}"#;
        assert_eq!(
            json_version(json).as_deref(),
            Some("dataset-20261022-101010-7")
        );
        assert_eq!(json_version("12"), None);
    }

    #[test]
    fn a_count_reads_with_or_without_thousands_separators() {
        assert_eq!(count_of("1.234.567\n"), Some(1_234_567));
        assert_eq!(count_of("12\n"), Some(12));
        assert_eq!(count_of("no"), None);
    }

    #[test]
    fn the_first_repeated_case_number_is_the_first_one_a_later_row_repeats() {
        let dir = tempfile::tempdir().unwrap();
        let csv = dir.path().join("crimes.csv");
        fs::write(
            &csv,
            "\"ID\",\"Case Number\"\n\"1\",\"JA1\"\n\"2\",\"JB2\"\n\"3\",\"JB2\"\n\"4\",\"JA1\"\n",
        )
        .unwrap();
        assert_eq!(first_repeated_case_number(&csv).unwrap(), "JB2");
    }
}
