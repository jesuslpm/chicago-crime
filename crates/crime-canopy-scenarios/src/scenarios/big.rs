//! R13: the big CSV (decision KR3), only on Windows.
//!
//! The big CSV is the triple of the reference CSV, built once by `crime-big-csv` in the work
//! folder and kept for later runs. The CanopyDB generator must accept it with its memory limit,
//! each query of the fixed set must count what a direct filter of the big CSV counts, and the
//! CanopyDB query app must keep the memory ratio of AC-M8.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use super::{Scenario, Systems, failed_to_run};
use crate::context::{Context, QUERY_TIMEOUT, count_of};
use crate::process::{Launch, run_to_end};
use crate::report::VariantResult;

const ID: &str = "R13/triple";

/// The free space R13 asks for, in multiples of the reference CSV's size: the big CSV (3×), the
/// dataset (about 3.3×) and the generator's sort folders, with margin (about 50 GB, KR3).
const SPACE_FACTOR: u64 = 20;

/// The longest the big generation may take.
const BIG_GENERATION_TIMEOUT: Duration = Duration::from_secs(90 * 60);

/// The longest a direct filter of the big CSV may take.
const SELECT_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The highest ratio between the peaks of AC-M8's three runs.
const MAX_MEMORY_RATIO: f64 = 3.0;

pub fn scenarios() -> Vec<Scenario> {
    vec![Scenario {
        id: "R13",
        systems: Systems::WindowsOnly,
        racy: false,
        variants: vec![(
            "triple",
            vec![
                "csv",
                "generador-codigo",
                "generador-registros",
                "memoria-generador",
                "conteos",
                "memoria-consultor",
            ],
        )],
        run: r13,
    }]
}

fn r13(context: &Context) -> Vec<VariantResult> {
    vec![run(context).unwrap_or_else(|error| failed_to_run(ID, &error))]
}

fn run(context: &Context) -> io::Result<VariantResult> {
    let folder = context.work.join("big");
    fs::create_dir_all(&folder)?;
    let csv_size = fs::metadata(&context.csv)?.len();
    let needed = csv_size * SPACE_FACTOR;
    let big_csv = folder.join("crimes-x3.csv");
    let output = folder.join("out");
    let reused = big_csv.is_file();
    // What the big CSV and an old dataset already take does not need to be free again.
    let already = if reused {
        fs::metadata(&big_csv)?.len()
    } else {
        0
    };
    if let Some(free) = free_bytes(&folder)
        && free + already < needed
    {
        return Ok(VariantResult::not_run(
            ID,
            format!(
                "falta espacio en disco: hacen falta {} GB libres y hay {} GB",
                gigabytes(needed - already),
                gigabytes(free)
            ),
        ));
    }
    let mut result = VariantResult::new(ID);

    // The big CSV, built once.
    let records = if reused {
        result.observe(format!("CSV grande reutilizado: {}", big_csv.display()));
        result.check("csv", true, "");
        None
    } else {
        let launch = Launch::new(&context.bins.big_csv, &context.run_dir("big-csv"))
            .arg("--input")
            .arg(&context.csv)
            .arg("--output")
            .arg(&big_csv);
        let (code, stdout, stderr) = run_to_end(launch, BIG_GENERATION_TIMEOUT)?;
        let records = written_records(&stdout, &big_csv);
        result.check(
            "csv",
            code == Some(0) && records.is_some(),
            format!(
                "crime-big-csv dio {code:?}: {}{}",
                stdout.trim(),
                stderr.trim()
            ),
        );
        if code != Some(0) {
            return Ok(result);
        }
        result.observe(stdout.trim().to_owned());
        records
    };

    // The generation.
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    let mut running = context
        .generation(&big_csv, &output, false, "big-generation")
        .start()?;
    let ended = running
        .wait(BIG_GENERATION_TIMEOUT)
        .ok_or_else(|| io::Error::other("la generación del CSV grande no terminó"))?;
    let stderr = running.stderr();
    result.check_eq("generador-codigo", Some(0), ended.code);
    result.observe(format!("generación en {} s", ended.after_start.as_secs()));
    let completed = stderr
        .lines()
        .find(|line| line.starts_with("Generación completada:"))
        .unwrap_or_default()
        .to_owned();
    let expected_line = records
        .as_deref()
        .map(|records| format!("Generación completada: {records} registros válidos, 0 rechazados"));
    let records_ok = match &expected_line {
        Some(expected) => completed.starts_with(expected),
        None => completed.contains(" registros válidos, 0 rechazados"),
    };
    result.check(
        "generador-registros",
        records_ok,
        format!("se esperaba {expected_line:?}, se obtuvo {completed:?}"),
    );
    let peak = stderr
        .lines()
        .find(|line| line.starts_with("Pico de memoria privada comprometida"))
        .unwrap_or_default()
        .to_owned();
    result.observe(peak.clone());
    result.check(
        "memoria-generador",
        peak.contains("(límite:"),
        format!("el resumen dice {peak:?}"),
    );
    if ended.code != Some(0) {
        return Ok(result);
    }

    // The fixed set against the direct filter of the big CSV.
    let mut wrong = Vec::new();
    for number in 1..=11 {
        let args = context.fixed_args(number);
        let (_, _, query_stderr) = run_to_end(
            context.query(&output, &args, &format!("big-f{number}")),
            QUERY_TIMEOUT,
        )?;
        let got = results_of(&query_stderr);
        let expected = direct_count(context, &big_csv, &args)?.map(|count| {
            if args.iter().any(|arg| arg == "--limit") {
                count.min(limit_of(&args))
            } else {
                count
            }
        });
        result.observe(format!("F{number}: {got:?} (filtro directo {expected:?})"));
        if got.is_none() || got != expected {
            wrong.push(format!("F{number}"));
        }
    }
    result.check(
        "conteos",
        wrong.is_empty(),
        format!("no coinciden con el filtro directo: {}", wrong.join(", ")),
    );

    // AC-M8: the counts F3 and F4 and a batch with the whole fixed set.
    let mut peaks = Vec::new();
    for (label, launch) in [
        (
            "F3",
            context.query(&output, &context.fixed_args(3), "big-m8-f3"),
        ),
        (
            "F4",
            context.query(&output, &context.fixed_args(4), "big-m8-f4"),
        ),
        (
            "lote",
            context.batch(&output, &context.fixed_batch_text(), "big-m8-batch")?,
        ),
    ] {
        let mut running = launch.start()?;
        running
            .wait(QUERY_TIMEOUT * 3)
            .ok_or_else(|| io::Error::other(format!("{label} no terminó")))?;
        peaks.push((label, running.peak_private_bytes()));
    }
    let known: Vec<u64> = peaks.iter().filter_map(|(_, peak)| *peak).collect();
    let text: Vec<String> = peaks
        .iter()
        .map(|(label, peak)| match peak {
            Some(bytes) => format!("{label} {:.1} MiB", *bytes as f64 / 1_048_576.0),
            None => format!("{label} sin medida"),
        })
        .collect();
    let ratio = match (known.iter().max(), known.iter().min()) {
        (Some(&max), Some(&min)) if known.len() == 3 && min > 0 => Some(max as f64 / min as f64),
        _ => None,
    };
    result.observe(format!(
        "picos AC-M8: {}; cociente {ratio:.2?}",
        text.join(", ")
    ));
    result.check(
        "memoria-consultor",
        ratio.is_some_and(|ratio| ratio <= MAX_MEMORY_RATIO),
        format!("cociente {ratio:.2?}, como mucho {MAX_MEMORY_RATIO}"),
    );
    Ok(result)
}

/// The record count of `Se han escrito <N> registros en <salida>.`, as written.
fn written_records(stdout: &str, big_csv: &Path) -> Option<String> {
    let line = stdout.trim();
    let rest = line.strip_prefix("Se han escrito ")?;
    let (records, place) = rest.split_once(" registros en ")?;
    (place.trim_end_matches('.') == big_csv.display().to_string()).then(|| records.to_owned())
}

/// The number of results of the stats line `<n> resultado(s) en …` of a query's stderr.
fn results_of(stderr: &str) -> Option<u64> {
    stderr.lines().find_map(|line| {
        let (count, rest) = line.split_once(' ')?;
        (rest.starts_with("resultado")).then(|| count_of(count))?
    })
}

/// What the direct filter of `csv` counts for the conditions of `args`.
fn direct_count(context: &Context, csv: &Path, args: &[String]) -> io::Result<Option<u64>> {
    let mut select: Vec<String> = vec!["select".into(), "--csv".into(), path_text(csv)];
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--where" | "--startswith" | "--range" => {
                select.push(word.clone());
                select.extend(words.next().cloned());
            }
            "--format" | "--limit" | "--output" => {
                words.next();
            }
            _ => {}
        }
    }
    let launch = Launch::new(&context.bins.acceptance, &context.run_dir("big-select")).args(select);
    let (code, stdout, stderr) = run_to_end(launch, SELECT_TIMEOUT)?;
    if code != Some(0) {
        return Err(io::Error::other(format!(
            "el filtro directo falló ({code:?}): {}",
            stderr.trim()
        )));
    }
    Ok(count_of(&stdout))
}

fn limit_of(args: &[String]) -> u64 {
    args.iter()
        .position(|arg| arg == "--limit")
        .and_then(|at| args.get(at + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(u64::MAX)
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

fn gigabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1_000_000_000)
}

/// The free bytes of the disk that holds `folder`, if the system tells.
#[cfg(windows)]
fn free_bytes(folder: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = folder
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free = 0u64;
    // SAFETY: a NUL-terminated path and a valid out pointer; the two others may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
fn free_bytes(_folder: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tool_message_gives_the_records_as_written() {
        let csv = Path::new("big.csv");
        assert_eq!(
            written_records("Se han escrito 25.970.712 registros en big.csv.\n", csv).as_deref(),
            Some("25.970.712")
        );
        assert_eq!(written_records("otra cosa", csv), None);
    }

    #[test]
    fn the_stats_line_gives_the_number_of_results() {
        let stderr = "--- Consulta 1 de 1 ---\n1.838.502 resultados en 1,821 s. Índices usados: Primary Type.\n";
        assert_eq!(results_of(stderr), Some(1_838_502));
        assert_eq!(results_of("1 resultado en 0,001 s.\n"), Some(1));
    }

    #[test]
    fn a_limit_is_read_from_the_arguments() {
        let args: Vec<String> = ["--where", "Primary Type=THEFT", "--limit", "1000"]
            .map(String::from)
            .to_vec();
        assert_eq!(limit_of(&args), 1000);
    }
}
