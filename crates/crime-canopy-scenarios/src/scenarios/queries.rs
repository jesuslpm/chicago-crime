//! R3–R7: the CanopyDB query app killed or cancelled midway, and the lock it leaves.
//!
//! R5 is the model of the others: launch, wait for the phase, send the signal, check that it
//! landed there, then check the variant's expectations, each one a named check. Every variant
//! that kills a process works on its own copy of the reference dataset.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crime_dataset::probe::phase;

use super::{
    Landed, Scenario, Systems, failed_to_run, next_query_opens, output_temporaries, until_landed,
};
use crate::context::{Context, QUERY_TIMEOUT, current_version};
use crate::process::{Ended, Running};
use crate::report::VariantResult;

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            id: "R3",
            systems: Systems::Both,
            racy: false,
            variants: R3_VARIANTS
                .iter()
                .map(|(name, kill)| (*name, kill.checks()))
                .collect(),
            run: r3,
        },
        Scenario {
            id: "R4",
            systems: Systems::Both,
            racy: false,
            variants: vec![
                ("tras-f10", R4_CHECKS.to_vec()),
                ("durante-f11", R4_CHECKS.to_vec()),
            ],
            run: r4,
        },
        Scenario {
            id: "R5",
            systems: Systems::Both,
            racy: false,
            variants: vec![
                ("consultor", R5_CHECKS.to_vec()),
                ("lote", R5_CHECKS.to_vec()),
            ],
            run: r5,
        },
        Scenario {
            id: "R6",
            systems: Systems::Both,
            racy: false,
            variants: vec![
                ("ctrl-c", R6_CTRL_C_CHECKS.to_vec()),
                ("matar", R6_KILL_CHECKS.to_vec()),
            ],
            run: r6,
        },
        Scenario {
            id: "R7",
            systems: Systems::Both,
            racy: false,
            variants: vec![("matar-lote", R7_CHECKS.to_vec())],
            run: r7,
        },
    ]
}

const R4_CHECKS: [&str; 5] = [
    "archivo",
    "f10-completo",
    "siguiente-sin-espera",
    "siguiente-resultado",
    "ac-m6",
];

const R5_CHECKS: [&str; 6] = [
    "codigo",
    "mensaje",
    "tiempo",
    "salida",
    "siguiente-sin-espera",
    "siguiente-resultado",
];

const R6_CTRL_C_CHECKS: [&str; 8] = [
    "codigo",
    "mensaje",
    "tiempo",
    "lote",
    "tercero-aviso",
    "tercero-resultado",
    "siguiente-sin-espera",
    "siguiente-resultado",
];

const R6_KILL_CHECKS: [&str; 5] = [
    "lote",
    "tercero-aviso",
    "tercero-resultado",
    "siguiente-sin-espera",
    "siguiente-resultado",
];

const R7_CHECKS: [&str; 5] = [
    "apertura-rapida",
    "un-aviso",
    "resultado",
    "siguiente-sin-espera",
    "siguiente-resultado",
];

/// How long after F11 starts the Ctrl+C is sent: F11 lists every `THEFT` and lasts seconds.
const INTO_F11: Duration = Duration::from_millis(400);

/// How long the query app pauses with the base open, so that a kill lands in the opening (R3).
const OPENING_PAUSE_MS: u64 = 3_000;

/// How long each query of the batch of R4 pauses as it starts, so that a kill lands after F10
/// and before F11 writes anything.
const AFTER_F10_PAUSE_MS: u64 = 1_500;

/// How long the batch of R6 keeps the dataset open: it ends between 10 and 20 s after the second
/// query app's warning, which comes a moment after the batch opens.
const R6_HOLD_MS: u64 = 15_000;

/// The batch of R6 ends between these times after the second query app's warning (PRD, R6).
const R6_HOLD_AFTER_WARNING: (Duration, Duration) =
    (Duration::from_secs(10), Duration::from_secs(20));

/// How long the batch of R7 keeps the dataset open: far longer than the scenario, it is killed.
const R7_HOLD_MS: u64 = 60_000;

/// The longest a query app may take to show the wait warning after it starts.
const WARNING_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a query app waits for the dataset before it is signalled (R6) or the batch is killed
/// (R7): long enough to be well inside its wait.
const WAITING_BEFORE_SIGNAL: Duration = Duration::from_secs(1);

/// The longest a signalled query app may take to end before the runner gives up on it.
const SIGNAL_END_TIMEOUT: Duration = Duration::from_secs(10);

/// The start and the end of the wait warning of KC2; the folder goes between them.
const WAIT_WARNING_START: &str = "Aviso: otro proceso tiene abierto el conjunto de datos en ";
const WAIT_WARNING_END: &str = "; se esperará hasta 30 s.";

/// The message of a cancelled query (KC2).
const CANCELLED: &str = "Consulta cancelada.";

/// The name of the output file of F11 in R3.
const OUTPUT_NAME: &str = "f11.csv";

/// What the previous file with the output's name holds before the kill (R3).
const PREVIOUS_CONTENT: &[u8] = b"archivo previo: no debe cambiar\n";

// ---------------------------------------------------------------------------------------------
// What the scenarios share
// ---------------------------------------------------------------------------------------------

/// Runs each variant of `scenario` over the reference dataset; a variant whose runner fails, or
/// every variant when the reference dataset cannot be generated, does not run.
fn each_variant(
    context: &Context,
    scenario: &str,
    variants: &[&str],
    run: impl Fn(&Path, &str, &str) -> Result<VariantResult, Box<VariantResult>>,
) -> Vec<VariantResult> {
    let dataset = match context.reference_dataset() {
        Ok(dataset) => dataset,
        Err(error) => {
            return variants
                .iter()
                .map(|variant| failed_to_run(&format!("{scenario}/{variant}"), &error))
                .collect();
        }
    };
    variants
        .iter()
        .map(|variant| {
            let id = format!("{scenario}/{variant}");
            run(&dataset, &id, variant).unwrap_or_else(|result| *result)
        })
        .collect()
}

/// A fresh copy of the reference dataset for the variant `id`.
fn fresh_copy(context: &Context, id: &str) -> io::Result<PathBuf> {
    context.fresh_output(&id.replace('/', "-"), true)
}

/// Removes the copy of a variant whose checks all passed; a failed one keeps it as evidence.
fn discard_copy_if_met(result: VariantResult, dataset: &Path) -> VariantResult {
    if result.checks.iter().all(|check| check.ok)
        && let Some(root) = dataset.parent()
    {
        let _ = fs::remove_dir_all(root);
    }
    result
}

/// What AC-M6 compares in a dataset folder: the names of its files and folders, and the bytes of
/// the manifest of the published version.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderState {
    names: Vec<String>,
    manifest: Option<Vec<u8>>,
}

fn folder_state(output: &Path) -> FolderState {
    let mut names = Vec::new();
    collect_names(output, "", &mut names);
    names.sort();
    let manifest = current_version(output).and_then(|version| {
        fs::read(output.join(version).join(crime_dataset::MANIFEST_FILE_NAME)).ok()
    });
    FolderState { names, manifest }
}

fn collect_names(dir: &Path, prefix: &str, names: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.path().is_dir() {
            collect_names(&entry.path(), &format!("{name}/"), names);
        }
        names.push(name);
    }
}

/// How `after` differs from `before` in what AC-M6 compares; empty when it does not.
fn folder_differences(before: &FolderState, after: &FolderState) -> Vec<String> {
    let mut differences = Vec::new();
    let new: Vec<&String> = after
        .names
        .iter()
        .filter(|name| !before.names.contains(name))
        .collect();
    let gone: Vec<&String> = before
        .names
        .iter()
        .filter(|name| !after.names.contains(name))
        .collect();
    if !new.is_empty() {
        differences.push(format!("archivos nuevos {new:?}"));
    }
    if !gone.is_empty() {
        differences.push(format!("archivos que faltan {gone:?}"));
    }
    if before.manifest.is_none() {
        differences.push("el conjunto no tenía manifiesto".to_owned());
    } else if after.manifest != before.manifest {
        differences.push("el manifiesto cambió".to_owned());
    }
    differences
}

/// AC-M6 after the signal and after the next query app: the same names, none new, and the same
/// manifest as before the run.
fn check_ac_m6(
    result: &mut VariantResult,
    before: &FolderState,
    after_signal: &FolderState,
    after_next: &FolderState,
) {
    let mut problems = Vec::new();
    for (when, state) in [
        ("tras la señal", after_signal),
        ("tras el siguiente consultor", after_next),
    ] {
        let differences = folder_differences(before, state);
        if !differences.is_empty() {
            problems.push(format!("{when}: {}", differences.join(", ")));
        }
    }
    result.check("ac-m6", problems.is_empty(), problems.join("; "));
}

/// The lines of `stderr` that are a warning (`Aviso: …`).
fn warnings(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("Aviso:"))
        .collect()
}

/// Whether `stderr` holds the wait warning of KC2.
fn has_waited(stderr: &str) -> bool {
    warnings(stderr)
        .iter()
        .any(|line| line.starts_with(WAIT_WARNING_START) && line.ends_with(WAIT_WARNING_END))
}

/// Whether `stderr` holds exactly one warning, and it is the wait warning of KC2.
fn one_wait_warning(stderr: &str) -> bool {
    let warnings = warnings(stderr);
    warnings.len() == 1 && has_waited(stderr)
}

/// "El mismo resultado" of a listing: the same lines, in any order.
fn same_records(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let mut a: Vec<&str> = a.lines().collect();
    let mut b: Vec<&str> = b.lines().collect();
    a.sort_unstable();
    b.sort_unstable();
    a == b
}

/// Whether `run` has marked `wanted`.
fn marked(run: &Running, wanted: &str) -> bool {
    run.phases().iter().any(|phase| phase == wanted)
}

/// How many times `run` has marked `wanted`.
fn times_marked(run: &Running, wanted: &str) -> usize {
    run.phases().iter().filter(|phase| *phase == wanted).count()
}

/// Waits for the end of every run, polling them all so that each end time is exact; `false` if
/// `timeout` passes first.
fn wait_all(runs: &mut [&mut Running], timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let mut all = true;
        for run in runs.iter_mut() {
            if !run.has_ended() {
                all = false;
            }
        }
        if all {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// When a run that has ended ended.
fn end_instant(run: &Running, ended: &Ended) -> Instant {
    run.started() + ended.after_start
}

/// The batch file of one line, F3, that keeps the dataset open in R6 and R7, at a fixed path so
/// that its clean run is run only once.
fn holder_batch(context: &Context) -> io::Result<PathBuf> {
    let dir = context.work.join("batches");
    fs::create_dir_all(&dir)?;
    let file = dir.join("f3.txt");
    fs::write(&file, format!("{}\n", context.fixed_line(3)))?;
    Ok(file)
}

fn holder_args(batch: &Path) -> Vec<String> {
    vec!["--batch".to_owned(), batch.display().to_string()]
}

fn timeout_error(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, what.to_owned())
}

// ---------------------------------------------------------------------------------------------
// R3
// ---------------------------------------------------------------------------------------------

/// Where R3 kills the query app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum R3Kill {
    /// During a pause right after the base is opened (L4), so the lock is held.
    Opening,
    /// During the count F3.
    Count,
    /// During the listing F11 to the standard output.
    Listing,
    /// During the listing F11 to an output file that did not exist.
    ListingToNewFile,
    /// During the listing F11 to an output file over a previous one with that name.
    ListingOverPreviousFile,
}

const R3_VARIANTS: [(&str, R3Kill); 5] = [
    ("apertura", R3Kill::Opening),
    ("f3", R3Kill::Count),
    ("f11-salida-estandar", R3Kill::Listing),
    ("f11-archivo-nuevo", R3Kill::ListingToNewFile),
    ("f11-archivo-previo", R3Kill::ListingOverPreviousFile),
];

impl R3Kill {
    fn writes_file(self) -> bool {
        matches!(
            self,
            R3Kill::ListingToNewFile | R3Kill::ListingOverPreviousFile
        )
    }

    fn checks(self) -> Vec<&'static str> {
        let mut checks = Vec::new();
        if self.writes_file() {
            checks.push("archivo");
        }
        checks.extend(["siguiente-sin-espera", "siguiente-resultado", "ac-m6"]);
        checks
    }
}

/// One run of R3 whose kill landed in its phase.
struct R3Run {
    dataset: PathBuf,
    before: FolderState,
    ended: Ended,
    stdout_len: u64,
    run_dir: PathBuf,
}

/// R3: kill a query app while it opens the base, during F3, and during F11 to the standard output
/// or to an output file, new or over a previous one. The next query app gives the clean result,
/// the dataset folder meets AC-M6, and the output file is the previous one or none.
fn r3(context: &Context) -> Vec<VariantResult> {
    let names: Vec<&str> = R3_VARIANTS.iter().map(|(name, _)| *name).collect();
    each_variant(context, "R3", &names, |_, id, variant| {
        let kill = R3_VARIANTS
            .iter()
            .find(|(name, _)| *name == variant)
            .map(|(_, kill)| *kill)
            .expect("a declared variant");
        r3_variant(context, id, kill)
    })
}

fn r3_variant(
    context: &Context,
    id: &str,
    kill: R3Kill,
) -> Result<VariantResult, Box<VariantResult>> {
    let landed = until_landed(id, |_| r3_attempt(context, id, kill)).map_err(Box::new)?;
    let mut result = VariantResult::new(id);
    result.observe(format!(
        "matado con {} bytes en la salida estándar; código {:?}",
        landed.stdout_len, landed.ended.code
    ));
    if kill.writes_file() {
        let leftovers = output_temporaries(&landed.run_dir, OUTPUT_NAME);
        if !leftovers.is_empty() {
            result.observe(format!(
                "temporal junto al archivo de salida (un proceso matado no puede borrarlo): {leftovers:?}"
            ));
        }
        let previous = (kill == R3Kill::ListingOverPreviousFile).then_some(PREVIOUS_CONTENT);
        check_output_file(&mut result, &landed.run_dir.join(OUTPUT_NAME), previous);
    }
    let after_signal = folder_state(&landed.dataset);
    if let Err(error) = next_query_opens(context, &mut result, &landed.dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    let after_next = folder_state(&landed.dataset);
    check_ac_m6(&mut result, &landed.before, &after_signal, &after_next);
    Ok(discard_copy_if_met(result, &landed.dataset))
}

fn r3_attempt(context: &Context, id: &str, kill: R3Kill) -> io::Result<Landed<R3Run>> {
    let dataset = fresh_copy(context, id)?;
    let before = folder_state(&dataset);
    let number = match kill {
        R3Kill::Opening | R3Kill::Count => 3,
        _ => 11,
    };
    let mut launch = context.query(&dataset, &context.fixed_args(number), id);
    let run_dir = launch.run_dir.clone();
    if kill.writes_file() {
        fs::create_dir_all(&run_dir)?;
        let output = run_dir.join(OUTPUT_NAME);
        if kill == R3Kill::ListingOverPreviousFile {
            fs::write(&output, PREVIOUS_CONTENT)?;
        }
        launch = launch.arg("--output").arg(output);
    }
    if kill == R3Kill::Opening {
        launch = launch.pause(phase::OPENED, OPENING_PAUSE_MS);
    }
    let mut running = launch.start()?;
    let awaited = if kill == R3Kill::Opening {
        phase::OPENED
    } else {
        phase::QUERY
    };
    if !running.wait_phase(awaited, QUERY_TIMEOUT) {
        return Ok(Landed::Missed(format!(
            "el consultor terminó sin llegar a la fase {awaited}"
        )));
    }
    if matches!(
        kill,
        R3Kill::Listing | R3Kill::ListingToNewFile | R3Kill::ListingOverPreviousFile
    ) {
        thread::sleep(INTO_F11);
    }
    if !running.kill() {
        return Ok(Landed::Missed("el consultor ya había terminado".to_owned()));
    }
    let ended = running
        .wait(QUERY_TIMEOUT)
        .ok_or_else(|| timeout_error("el consultor no terminó tras matarlo"))?;
    // Where the kill landed, read once the app is dead and nothing changes any more.
    let stdout_len = running.stdout_len();
    let missed = match kill {
        R3Kill::Opening if marked(&running, phase::QUERY) => {
            Some("la consulta ya había empezado: la base ya estaba abierta")
        }
        R3Kill::Count if stdout_len > 0 => Some("el conteo ya había terminado"),
        R3Kill::Listing if stdout_len == 0 => Some("el listado aún no escribía"),
        R3Kill::ListingToNewFile | R3Kill::ListingOverPreviousFile
            if output_temporaries(&run_dir, OUTPUT_NAME).is_empty() =>
        {
            Some("el listado no estaba escribiendo su temporal")
        }
        _ => None,
    };
    if let Some(why) = missed {
        return Ok(Landed::Missed(why.to_owned()));
    }
    Ok(Landed::Yes(R3Run {
        dataset,
        before,
        ended,
        stdout_len,
        run_dir,
    }))
}

/// C8: the file with the requested name is the previous one, intact, or does not exist if there
/// was none; never a half-written one.
fn check_output_file(result: &mut VariantResult, path: &Path, previous: Option<&[u8]>) {
    let now = fs::read(path).ok();
    let seen = match &now {
        Some(bytes) => format!("un archivo de {} bytes", bytes.len()),
        None => "ningún archivo".to_owned(),
    };
    match previous {
        Some(previous) => result.check(
            "archivo",
            now.as_deref() == Some(previous),
            format!(
                "se esperaba el archivo previo intacto ({} bytes), hay {seen}",
                previous.len()
            ),
        ),
        None => result.check(
            "archivo",
            now.is_none(),
            format!("no había archivo previo y hay {seen}"),
        ),
    };
}

// ---------------------------------------------------------------------------------------------
// R4
// ---------------------------------------------------------------------------------------------

/// One run of R4 whose kill landed in its phase.
struct R4Run {
    dataset: PathBuf,
    before: FolderState,
    ended: Ended,
    files: PathBuf,
}

/// R4: kill a batch of the fixed set whose F10 and F11 write to output files, after F10 ends and
/// during F11. As R3, and the file of F10 is complete and the one of a clean run.
fn r4(context: &Context) -> Vec<VariantResult> {
    each_variant(
        context,
        "R4",
        &["tras-f10", "durante-f11"],
        |_, id, variant| r4_variant(context, id, variant == "tras-f10"),
    )
}

fn r4_variant(
    context: &Context,
    id: &str,
    after_f10: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let landed = until_landed(id, |_| r4_attempt(context, id, after_f10)).map_err(Box::new)?;
    let mut result = VariantResult::new(id);
    result.observe(format!("lote matado; código {:?}", landed.ended.code));
    let leftovers = output_temporaries(&landed.files, "f11.csv");
    if !leftovers.is_empty() {
        result.observe(format!(
            "temporal junto al archivo de F11 (un proceso matado no puede borrarlo): {leftovers:?}"
        ));
    }
    check_output_file(&mut result, &landed.files.join("f11.csv"), None);
    let clean = match context.clean_run(&context.fixed_args(10)) {
        Ok(clean) => clean,
        Err(error) => return Err(Box::new(failed_to_run(id, &error))),
    };
    let f10 = fs::read_to_string(landed.files.join("f10.csv"));
    let f10_ok = clean.code == Some(0)
        && f10
            .as_deref()
            .is_ok_and(|text| same_records(text, &clean.stdout));
    result.check(
        "f10-completo",
        f10_ok,
        match &f10 {
            Ok(text) => format!(
                "el archivo de F10 tiene {} líneas y la ejecución limpia {} (código {:?})",
                text.lines().count(),
                clean.stdout.lines().count(),
                clean.code
            ),
            Err(error) => format!("no se pudo leer el archivo de F10: {error}"),
        },
    );
    let after_signal = folder_state(&landed.dataset);
    if let Err(error) = next_query_opens(context, &mut result, &landed.dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    let after_next = folder_state(&landed.dataset);
    check_ac_m6(&mut result, &landed.before, &after_signal, &after_next);
    Ok(discard_copy_if_met(result, &landed.dataset))
}

fn r4_attempt(context: &Context, id: &str, after_f10: bool) -> io::Result<Landed<R4Run>> {
    let dataset = fresh_copy(context, id)?;
    let before = folder_state(&dataset);
    let files = context.run_dir(&format!("{id}-salidas"));
    fs::create_dir_all(&files)?;
    let mut text = String::new();
    for number in 1..=11 {
        text.push_str(context.fixed_line(number));
        if number >= 10 {
            let output = files.join(format!("f{number}.csv"));
            text.push_str(&format!(" --output \"{}\"", output.display()));
        }
        text.push('\n');
    }
    let mut launch = context.batch(&dataset, &text, id)?;
    if after_f10 {
        // Every query pauses as it starts: the kill lands in the pause of F11, after F10 has
        // written its file and before F11 creates its temporary.
        launch = launch.pause(phase::QUERY, AFTER_F10_PAUSE_MS);
    }
    let mut running = launch.start()?;
    if !running.wait_until(QUERY_TIMEOUT, |run| times_marked(run, phase::QUERY) >= 11) {
        return Ok(Landed::Missed("la consulta F11 no empezó".to_owned()));
    }
    if !after_f10 {
        thread::sleep(INTO_F11);
    }
    if !running.kill() {
        return Ok(Landed::Missed("el lote ya había terminado".to_owned()));
    }
    let ended = running
        .wait(QUERY_TIMEOUT)
        .ok_or_else(|| timeout_error("el lote no terminó tras matarlo"))?;
    let f11_writing = !output_temporaries(&files, "f11.csv").is_empty();
    let missed = if after_f10 {
        if !files.join("f10.csv").exists() {
            Some("F10 aún no había escrito su archivo")
        } else if f11_writing {
            Some("F11 ya había empezado a escribir")
        } else {
            None
        }
    } else if !f11_writing {
        Some("F11 no estaba escribiendo su temporal")
    } else {
        None
    };
    if let Some(why) = missed {
        return Ok(Landed::Missed(why.to_owned()));
    }
    Ok(Landed::Yes(R4Run {
        dataset,
        before,
        ended,
        files,
    }))
}

// ---------------------------------------------------------------------------------------------
// R5
// ---------------------------------------------------------------------------------------------

/// R5: Ctrl+C midway through F11, outside the wait, to a single query and to a batch: 130 and
/// `Consulta cancelada.` in less than 1 s, no output file nor temporary left, and the next query
/// app opens without the wait warning.
fn r5(context: &Context) -> Vec<VariantResult> {
    each_variant(
        context,
        "R5",
        &["consultor", "lote"],
        |dataset, id, variant| r5_variant(context, dataset, id, variant == "lote"),
    )
}

fn r5_variant(
    context: &Context,
    dataset: &std::path::Path,
    id: &str,
    batch: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let mut result = VariantResult::new(id);
    let landed = super::until_landed(id, |_| {
        let run_dir = context.run_dir(id);
        fs::create_dir_all(&run_dir)?;
        let output_name = "f11.csv";
        let output = run_dir.join(output_name);
        let f11 = format!(
            "{} --output \"{}\"",
            context.fixed_line(11),
            output.display()
        );
        // The batch runs F1 to F10 first, then F11 with its output file.
        let launch = if batch {
            let mut lines: Vec<String> = (1..=10)
                .map(|number| context.fixed_line(number).to_owned())
                .collect();
            lines.push(f11);
            context.batch(dataset, &(lines.join("\n") + "\n"), id)?
        } else {
            let mut args = context.fixed_args(11);
            args.push("--output".to_owned());
            args.push(output.display().to_string());
            context.query(dataset, &args, id)
        };
        let mut running = launch.with_ctrl_c().start()?;
        let queries_before_f11 = if batch { 11 } else { 1 };
        let reached = running.wait_until(QUERY_TIMEOUT, |run| {
            run.phases()
                .iter()
                .filter(|phase| *phase == "query")
                .count()
                >= queries_before_f11
        });
        if !reached {
            return Ok(Landed::Missed("la consulta F11 no empezó".to_owned()));
        }
        std::thread::sleep(INTO_F11);
        if !running.ctrl_c()? {
            return Ok(Landed::Missed("F11 ya había terminado".to_owned()));
        }
        let ended = running
            .wait(QUERY_TIMEOUT)
            .ok_or_else(|| std::io::Error::other("el consultor no terminó tras Ctrl+C"))?;
        Ok(Landed::Yes((
            ended,
            running.stderr(),
            run_dir,
            output_name.to_owned(),
        )))
    });
    let (ended, stderr, run_dir, output_name) = landed.map_err(Box::new)?;
    let after = ended.after_signal.unwrap_or_default();
    result.observe(format!(
        "código {:?} a los {:.2} s del Ctrl+C",
        ended.code,
        after.as_secs_f64()
    ));
    result.check_eq("codigo", Some(130), ended.code);
    result.check(
        "mensaje",
        stderr.lines().any(|line| line == "Consulta cancelada."),
        format!("falta `Consulta cancelada.` en: {}", stderr.trim()),
    );
    result.check(
        "tiempo",
        after < Duration::from_secs(1),
        format!("tardó {:.2} s", after.as_secs_f64()),
    );
    let leftovers = output_temporaries(&run_dir, &output_name);
    let file_left = run_dir.join(&output_name).exists();
    result.check(
        "salida",
        !file_left && leftovers.is_empty(),
        format!("quedó el archivo de salida: {file_left}; temporales: {leftovers:?}"),
    );
    if let Err(error) = next_query_opens(context, &mut result, dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    Ok(result)
}

// ---------------------------------------------------------------------------------------------
// R6
// ---------------------------------------------------------------------------------------------

/// One run of R6 whose signal landed during the second query app's wait.
struct R6Run {
    dataset: PathBuf,
    second: Ended,
    second_stderr: String,
    holder: Ended,
    holder_stdout: String,
    holder_after_warning: Duration,
    third: Ended,
    third_stdout: String,
    third_stderr: String,
    third_after_holder: bool,
}

/// R6: a batch holds the dataset; a second query app waits and is cancelled with Ctrl+C or
/// killed; a third one arrives. The cancelled one ends as KC2 says, the batch ends with 0 and the
/// clean result, and the third shows one warning and answers with the clean result once the
/// batch ends.
fn r6(context: &Context) -> Vec<VariantResult> {
    each_variant(context, "R6", &["ctrl-c", "matar"], |_, id, variant| {
        r6_variant(context, id, variant == "ctrl-c")
    })
}

fn r6_variant(
    context: &Context,
    id: &str,
    ctrl_c: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let prepared = holder_batch(context).and_then(|batch| {
        let holder_clean = context.clean_run(&holder_args(&batch))?;
        let f3_clean = context.clean_run(&context.fixed_args(3))?;
        Ok((batch, holder_clean, f3_clean))
    });
    let (batch, holder_clean, f3_clean) =
        prepared.map_err(|error| Box::new(failed_to_run(id, &error)))?;
    let run = until_landed(id, |_| r6_attempt(context, id, &batch, ctrl_c)).map_err(Box::new)?;
    let mut result = VariantResult::new(id);
    let after = run.second.after_signal.unwrap_or_default();
    let signal = if ctrl_c { "Ctrl+C" } else { "la muerte" };
    result.observe(format!(
        "segundo: código {:?} a los {:.2} s de {signal}; lote: código {:?}, terminó {:.1} s después del aviso del segundo; tercero: código {:?}",
        run.second.code,
        after.as_secs_f64(),
        run.holder.code,
        run.holder_after_warning.as_secs_f64(),
        run.third.code
    ));
    if ctrl_c {
        result.check_eq("codigo", Some(130), run.second.code);
        result.check(
            "mensaje",
            run.second_stderr.lines().any(|line| line == CANCELLED),
            format!("falta `{CANCELLED}` en: {}", run.second_stderr.trim()),
        );
        result.check(
            "tiempo",
            after < Duration::from_secs(1),
            format!("tardó {:.2} s", after.as_secs_f64()),
        );
    }
    result.check(
        "lote",
        run.holder.code == Some(0) && run.holder_stdout == holder_clean.stdout,
        format!(
            "el lote dio el código {:?} y {:?}, no {:?}",
            run.holder.code,
            run.holder_stdout.trim(),
            holder_clean.stdout.trim()
        ),
    );
    result.check(
        "tercero-aviso",
        one_wait_warning(&run.third_stderr),
        format!(
            "se esperaba un solo aviso de espera, el tercero mostró: {:?}",
            warnings(&run.third_stderr)
        ),
    );
    result.check(
        "tercero-resultado",
        run.third.code == Some(0) && run.third_stdout == f3_clean.stdout && run.third_after_holder,
        format!(
            "el tercero dio el código {:?} y {:?}, no {:?}; terminó después del lote: {}",
            run.third.code,
            run.third_stdout.trim(),
            f3_clean.stdout.trim(),
            run.third_after_holder
        ),
    );
    if let Err(error) = next_query_opens(context, &mut result, &run.dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    Ok(discard_copy_if_met(result, &run.dataset))
}

fn r6_attempt(
    context: &Context,
    id: &str,
    batch: &Path,
    ctrl_c: bool,
) -> io::Result<Landed<R6Run>> {
    let dataset = fresh_copy(context, id)?;
    let mut holder = context
        .query(&dataset, &holder_args(batch), &format!("{id}-lote"))
        .pause(phase::QUERY, R6_HOLD_MS)
        .start()?;
    if !holder.wait_phase(phase::QUERY, QUERY_TIMEOUT) {
        return Ok(Landed::Missed(
            "el lote no llegó a abrir el conjunto".to_owned(),
        ));
    }
    let mut launch = context.query(&dataset, &context.fixed_args(3), &format!("{id}-segundo"));
    if ctrl_c {
        launch = launch.with_ctrl_c();
    }
    let mut second = launch.start()?;
    if !second.wait_until(WARNING_TIMEOUT, |run| has_waited(&run.stderr())) {
        return Ok(Landed::Missed(
            "el segundo consultor no llegó a esperar".to_owned(),
        ));
    }
    let warned = Instant::now();
    thread::sleep(WAITING_BEFORE_SIGNAL);
    if holder.has_ended() || marked(&second, phase::OPENED) {
        return Ok(Landed::Missed(
            "el lote ya había liberado el conjunto".to_owned(),
        ));
    }
    let sent = if ctrl_c {
        second.ctrl_c()?
    } else {
        second.kill()
    };
    if !sent {
        return Ok(Landed::Missed(
            "el segundo consultor ya había terminado".to_owned(),
        ));
    }
    let second_ended = second
        .wait(SIGNAL_END_TIMEOUT)
        .ok_or_else(|| timeout_error("el segundo consultor no terminó tras la señal"))?;
    if marked(&second, phase::OPENED) {
        return Ok(Landed::Missed(
            "la señal llegó cuando el segundo ya había abierto el conjunto".to_owned(),
        ));
    }
    let mut third = context
        .query(&dataset, &context.fixed_args(3), &format!("{id}-tercero"))
        .start()?;
    if !wait_all(&mut [&mut holder, &mut third], QUERY_TIMEOUT) {
        return Err(timeout_error("el lote o el tercer consultor no terminaron"));
    }
    let holder_ended = holder.wait(Duration::ZERO).expect("it has ended");
    let third_ended = third.wait(Duration::ZERO).expect("it has ended");
    let holder_end = end_instant(&holder, &holder_ended);
    let holder_after_warning = holder_end.saturating_duration_since(warned);
    let (low, high) = R6_HOLD_AFTER_WARNING;
    if holder_after_warning < low || holder_after_warning > high {
        return Ok(Landed::Missed(format!(
            "el lote terminó {:.1} s después del aviso, fuera de 10 a 20 s",
            holder_after_warning.as_secs_f64()
        )));
    }
    Ok(Landed::Yes(R6Run {
        dataset,
        second: second_ended,
        second_stderr: second.stderr(),
        holder: holder_ended,
        holder_stdout: holder.stdout(),
        holder_after_warning,
        third: third_ended,
        third_stdout: third.stdout(),
        third_stderr: third.stderr(),
        third_after_holder: end_instant(&third, &third_ended) >= holder_end,
    }))
}

// ---------------------------------------------------------------------------------------------
// R7
// ---------------------------------------------------------------------------------------------

/// One run of R7 whose kill landed while the second query app waited.
struct R7Run {
    dataset: PathBuf,
    /// From the kill of the batch to the second query app's `opened` mark.
    opened_after_kill: Option<Duration>,
    second: Ended,
    second_stdout: String,
    second_stderr: String,
}

/// R7: a batch holds the dataset, a second query app waits, and the batch is killed. The second
/// opens in less than 1 s from the kill, with no second warning, and answers with 0 and the clean
/// result.
fn r7(context: &Context) -> Vec<VariantResult> {
    each_variant(context, "R7", &["matar-lote"], |_, id, _| {
        r7_variant(context, id)
    })
}

fn r7_variant(context: &Context, id: &str) -> Result<VariantResult, Box<VariantResult>> {
    let prepared = holder_batch(context)
        .and_then(|batch| Ok((batch, context.clean_run(&context.fixed_args(3))?)));
    let (batch, f3_clean) = prepared.map_err(|error| Box::new(failed_to_run(id, &error)))?;
    let run = until_landed(id, |_| r7_attempt(context, id, &batch)).map_err(Box::new)?;
    let mut result = VariantResult::new(id);
    let opened = match run.opened_after_kill {
        Some(after) => format!("{:.2} s", after.as_secs_f64()),
        None => "nunca".to_owned(),
    };
    result.observe(format!(
        "el segundo abrió a los {opened} de matar el lote; código {:?}",
        run.second.code
    ));
    result.check(
        "apertura-rapida",
        run.opened_after_kill
            .is_some_and(|after| after < Duration::from_secs(1)),
        format!("abrió a los {opened} de matar el lote"),
    );
    result.check(
        "un-aviso",
        one_wait_warning(&run.second_stderr),
        format!(
            "se esperaba un solo aviso de espera, el segundo mostró: {:?}",
            warnings(&run.second_stderr)
        ),
    );
    result.check(
        "resultado",
        run.second.code == Some(0) && run.second_stdout == f3_clean.stdout,
        format!(
            "el segundo dio el código {:?} y {:?}, no {:?}",
            run.second.code,
            run.second_stdout.trim(),
            f3_clean.stdout.trim()
        ),
    );
    if let Err(error) = next_query_opens(context, &mut result, &run.dataset) {
        return Err(Box::new(failed_to_run(id, &error)));
    }
    Ok(discard_copy_if_met(result, &run.dataset))
}

fn r7_attempt(context: &Context, id: &str, batch: &Path) -> io::Result<Landed<R7Run>> {
    let dataset = fresh_copy(context, id)?;
    let mut holder = context
        .query(&dataset, &holder_args(batch), &format!("{id}-lote"))
        .pause(phase::QUERY, R7_HOLD_MS)
        .start()?;
    if !holder.wait_phase(phase::QUERY, QUERY_TIMEOUT) {
        return Ok(Landed::Missed(
            "el lote no llegó a abrir el conjunto".to_owned(),
        ));
    }
    let mut second = context
        .query(&dataset, &context.fixed_args(3), &format!("{id}-segundo"))
        .start()?;
    if !second.wait_until(WARNING_TIMEOUT, |run| has_waited(&run.stderr())) {
        return Ok(Landed::Missed(
            "el segundo consultor no llegó a esperar".to_owned(),
        ));
    }
    thread::sleep(WAITING_BEFORE_SIGNAL);
    if second.has_ended() || marked(&second, phase::OPENED) {
        return Ok(Landed::Missed(
            "el segundo consultor ya no esperaba".to_owned(),
        ));
    }
    let killed_at = Instant::now();
    if !holder.kill() {
        return Ok(Landed::Missed("el lote ya había terminado".to_owned()));
    }
    let opened_after_kill = second
        .wait_until(SIGNAL_END_TIMEOUT, |run| marked(run, phase::OPENED))
        .then(|| killed_at.elapsed());
    let second_ended = second
        .wait(QUERY_TIMEOUT)
        .ok_or_else(|| timeout_error("el segundo consultor no terminó"))?;
    Ok(Landed::Yes(R7Run {
        dataset,
        opened_after_kill,
        second: second_ended,
        second_stdout: second.stdout(),
        second_stderr: second.stderr(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warning(folder: &str) -> String {
        format!("{WAIT_WARNING_START}{folder}{WAIT_WARNING_END}")
    }

    #[test]
    fn one_wait_warning_means_exactly_one_warning_and_that_it_is_the_wait_one() {
        let one = format!("{}\n", warning(r"C:\datos\salida"));
        assert!(has_waited(&one));
        assert!(one_wait_warning(&one));
        let two = format!("{one}{}\n", warning(r"C:\datos\salida"));
        assert!(!one_wait_warning(&two));
        let other = "Aviso: otra cosa\n";
        assert!(!has_waited(other));
        assert!(!one_wait_warning(other));
        assert!(!one_wait_warning(""));
    }

    #[test]
    fn the_same_records_are_the_same_lines_in_any_order() {
        assert!(same_records("a\nb\n", "a\nb\n"));
        assert!(same_records("a\nb\n", "b\na\n"));
        assert!(!same_records("a\nb\n", "a\n"));
        assert!(!same_records("a\nb\n", "a\nc\n"));
    }

    #[test]
    fn ac_m6_sees_a_new_file_a_missing_one_and_a_changed_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path();
        let version = "dataset-20261022-101010-1";
        fs::create_dir_all(output.join(version).join("canopydb")).unwrap();
        fs::write(output.join("CURRENT"), version).unwrap();
        fs::write(output.join(version).join("manifest.json"), "{}").unwrap();
        fs::write(output.join(version).join("canopydb").join("LOCK"), "").unwrap();
        let before = folder_state(output);
        assert!(before.names.contains(&format!("{version}/canopydb/LOCK")));
        assert!(folder_differences(&before, &folder_state(output)).is_empty());

        fs::write(output.join(version).join("canopydb").join("extra"), "").unwrap();
        let differences = folder_differences(&before, &folder_state(output));
        assert_eq!(differences.len(), 1);
        assert!(differences[0].starts_with("archivos nuevos"));
        fs::remove_file(output.join(version).join("canopydb").join("extra")).unwrap();

        fs::remove_file(output.join(version).join("canopydb").join("LOCK")).unwrap();
        let differences = folder_differences(&before, &folder_state(output));
        assert_eq!(differences.len(), 1);
        assert!(differences[0].starts_with("archivos que faltan"));
        fs::write(output.join(version).join("canopydb").join("LOCK"), "").unwrap();

        fs::write(output.join(version).join("manifest.json"), "{\"x\":1}").unwrap();
        assert_eq!(
            folder_differences(&before, &folder_state(output)),
            vec!["el manifiesto cambió".to_owned()]
        );
    }

    #[test]
    fn an_output_file_is_the_previous_one_intact_or_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f11.csv");
        let mut result = VariantResult::new("R3/x");
        check_output_file(&mut result, &path, None);
        fs::write(&path, PREVIOUS_CONTENT).unwrap();
        check_output_file(&mut result, &path, Some(PREVIOUS_CONTENT));
        assert!(result.checks.iter().all(|check| check.ok));

        let mut result = VariantResult::new("R3/x");
        check_output_file(&mut result, &path, None);
        fs::write(&path, b"a medias").unwrap();
        check_output_file(&mut result, &path, Some(PREVIOUS_CONTENT));
        assert!(result.checks.iter().all(|check| !check.ok));
    }

    #[test]
    fn every_variant_of_r3_to_r7_declares_its_checks() {
        let declared: Vec<String> = scenarios()
            .iter()
            .flat_map(|scenario| {
                scenario
                    .variants
                    .iter()
                    .map(|(variant, _)| scenario.variant_id(variant))
            })
            .collect();
        assert_eq!(
            declared,
            [
                "R3/apertura",
                "R3/f3",
                "R3/f11-salida-estandar",
                "R3/f11-archivo-nuevo",
                "R3/f11-archivo-previo",
                "R4/tras-f10",
                "R4/durante-f11",
                "R5/consultor",
                "R5/lote",
                "R6/ctrl-c",
                "R6/matar",
                "R7/matar-lote",
            ]
        );
    }
}
