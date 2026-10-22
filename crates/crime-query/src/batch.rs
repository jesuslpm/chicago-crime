//! A batch of queries, one per line of a file, run over a dataset opened once (decision K7 of the
//! canopy-db PRD; canopy-db-lote PRD, decisions B1–B7).
//!
//! The whole file is checked before the dataset is opened, in the PRD's order: reading and
//! encoding, then each line (quotes, options a line does not admit, the single query's form
//! and conditions, an output file already used), then at least one query. The first error
//! rejects the batch and nothing runs. Then each query runs as it would alone, under a header
//! on stderr, and the first one that fails or is cancelled stops the batch; what earlier
//! queries wrote stays. The checked queries are kept (decision B7); no query keeps anything for
//! the next one but the open dataset.

use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use crate::cli::{self, BatchOptions, Options};
use crate::run::{Outcome, open_failed, report, run_query, say};
use crate::{App, ExecError, Query, exit_code, messages};

/// One query of the batch, checked.
struct Line {
    /// Its line in the file, from 1.
    number: usize,
    options: Options,
    query: Query,
}

/// Runs the batch of `options` and returns the exit code. `cancel` is polled before each query
/// and while each one runs; once it answers `true` the batch ends with
/// [`exit_code::CANCELLED`].
pub fn run_batch(
    options: &BatchOptions,
    cancel: &dyn Fn() -> bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    run_batch_with(&crate::APP, options, cancel, stdout, stderr)
}

/// [`run_batch`] for the query app `app`, which opens the dataset with its own engine.
pub fn run_batch_with(
    app: &App,
    options: &BatchOptions,
    cancel: &dyn Fn() -> bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let started = Instant::now();
    let lines = match check(options) {
        Ok(lines) => lines,
        Err((message, code)) => return report(stderr, message, code),
    };
    let dataset = match (app.open)(&options.dataset, cancel, stderr) {
        Ok(dataset) => dataset,
        Err(error) => return open_failed(stderr, error),
    };
    let total = lines.len();
    for (index, line) in lines.iter().enumerate() {
        // A Ctrl+C before a query starts ends the batch without its header.
        if cancel() {
            return report(stderr, ExecError::Cancelled, exit_code::CANCELLED);
        }
        say(
            stderr,
            messages::batch_header(index + 1, total, line.number),
        );
        match run_query(
            &line.options,
            &line.query,
            dataset.as_ref(),
            cancel,
            stdout,
            stderr,
        ) {
            Outcome::Completed => {}
            Outcome::StdoutClosed => return exit_code::COMPLETED,
            Outcome::Failed(code) => return code,
        }
    }
    say(stderr, messages::batch_completed(total, started.elapsed()));
    exit_code::COMPLETED
}

/// Reads and checks the whole batch file. `Err` holds the first error and its exit code.
fn check(options: &BatchOptions) -> Result<Vec<Line>, (String, u8)> {
    let rejected = |message: String| (message, exit_code::REJECTED);
    let bytes = std::fs::read(&options.file)
        .map_err(|_| rejected(messages::batch_unreadable(&options.file)))?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| rejected(messages::batch_not_utf8(&options.file)))?;
    let current = std::env::current_dir().unwrap_or_default();
    let batch_key = path_key(&options.file, &current);
    // The output files of the lines checked so far, by comparison key, with their line.
    let mut outputs: Vec<(String, usize)> = Vec::new();
    let mut lines = Vec::new();
    for (index, text) in text.split('\n').enumerate() {
        let number = index + 1;
        let text = text.strip_suffix('\r').unwrap_or(text);
        if is_ignored(text) {
            continue;
        }
        let on_line = |message: String, code: u8| (messages::on_line(number, message), code);
        let words = split_words(text)
            .ok_or_else(|| on_line(messages::unclosed_quote(), exit_code::USAGE))?;
        let line_options = cli::parse_line(&words, &options.dataset)
            .map_err(|message| on_line(message, exit_code::USAGE))?;
        let query = Query::build(&line_options.conditions)
            .map_err(|error| on_line(error.to_string(), exit_code::REJECTED))?
            .with_limit(line_options.limit);
        if let Some(output) = &line_options.output {
            let key = path_key(output, &current);
            if key == batch_key {
                return Err(on_line(
                    messages::output_is_batch(output),
                    exit_code::REJECTED,
                ));
            }
            if let Some(&(_, earlier)) = outputs.iter().find(|(used, _)| *used == key) {
                return Err(on_line(
                    messages::output_reused(output, earlier),
                    exit_code::REJECTED,
                ));
            }
            outputs.push((key, number));
        }
        lines.push(Line {
            number,
            options: line_options,
            query,
        });
    }
    if lines.is_empty() {
        return Err(rejected(messages::batch_empty(&options.file)));
    }
    Ok(lines)
}

/// An empty line, a line of spaces and tabs only, or a comment: `#` after optional spaces and
/// tabs.
fn is_ignored(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']);
    rest.is_empty() || rest.starts_with('#')
}

/// The words of a batch line: spaces and tabs separate them, double or single quotes group a
/// text with spaces and may open in the middle of a word, the other kind of quote is a plain
/// character inside them, and there are no escapes. `None` when a quote is left open.
pub fn split_words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for c in line.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => word.push(c),
            None if c == ' ' || c == '\t' => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                in_word = true;
            }
            None => {
                word.push(c);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if in_word {
        words.push(word);
    }
    Some(words)
}

/// The key two paths are compared by (decision B5): absolute from `current`, without `.` and
/// `..`, without touching the disk, and on Windows without case.
fn path_key(path: &Path, current: &Path) -> String {
    let mut resolved = PathBuf::new();
    for component in current.join(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    let key = resolved.to_string_lossy().into_owned();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        split_words(line).expect("quotes are closed")
    }

    #[test]
    fn spaces_and_tabs_separate_words_and_quotes_group_them() {
        assert_eq!(
            words("--where \"Primary Type=THEFT\"\t--count"),
            ["--where", "Primary Type=THEFT", "--count"]
        );
        assert_eq!(
            words("--where=\"Primary Type=THEFT\""),
            ["--where=Primary Type=THEFT"]
        );
        assert_eq!(
            words("--where 'Location Description=O\"X' \"B=O'HARE\""),
            ["--where", "Location Description=O\"X", "B=O'HARE"]
        );
        assert_eq!(
            words(r"--output C:\tmp\x.csv"),
            ["--output", r"C:\tmp\x.csv"]
        );
        assert_eq!(words("\"\""), [""]);
        assert_eq!(words("a\u{a0}b\rc\u{feff}"), ["a\u{a0}b\rc\u{feff}"]);
    }

    #[test]
    fn an_open_quote_is_refused() {
        assert_eq!(split_words("--where \"Primary Type=THEFT"), None);
        assert_eq!(split_words("--where O'HARE"), None);
    }

    #[test]
    fn empty_lines_and_comments_are_ignored() {
        assert!(is_ignored(""));
        assert!(is_ignored(" \t "));
        assert!(is_ignored("# note"));
        assert!(is_ignored("  \t# note"));
        assert!(!is_ignored("--count # note"));
        assert!(!is_ignored("\"\""));
    }

    #[test]
    fn paths_are_compared_lexically() {
        let current = std::env::temp_dir().join("work");
        let key = |path: &str| path_key(Path::new(path), &current);
        assert_eq!(key("crim.csv"), key("./crim.csv"));
        assert_eq!(key("crim.csv"), key("sub/../crim.csv"));
        assert_eq!(
            key("crim.csv"),
            key(&current.join("crim.csv").to_string_lossy())
        );
        assert_ne!(key("crim.csv"), key("other.csv"));
        assert_eq!(cfg!(windows), key("crim.csv") == key("CRIM.csv"));
    }
}
