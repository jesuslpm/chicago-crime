//! R1, R2 and R10: the CanopyDB generator killed or cancelled in each phase.
//!
//! Each variant works on a fresh copy of the reference dataset (or on an empty place for one),
//! starts the generator, waits until it marks the variant's phase, sends the signal there and
//! checks that it landed there, then checks what the PRD row expects: the version a query reads,
//! what the generator left behind and what the next generation says about it, its exit code and
//! the published content.
//!
//! A long phase (reading, primary load, index writing) gets its signal a moment after its marker;
//! a short one (manifest, temporary pointer, switch) is stretched by a pause of the app and gets
//! its signal during the pause.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crime_dataset::probe::phase;

use super::{Landed, Scenario, Systems, failed_to_run, until_landed};
use crate::context::{
    self, Context, GENERATION_TIMEOUT, QUERY_TIMEOUT, current_version, json_version,
};
use crate::process::{Ended, Running, run_to_end};
use crate::report::VariantResult;

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            id: "R1",
            systems: Systems::Both,
            racy: false,
            variants: r1_variants(),
            run: r1,
        },
        Scenario {
            id: "R2",
            systems: Systems::Both,
            racy: false,
            variants: R2_PHASES
                .iter()
                .map(|phase| (phase.name, r2_checks(phase).to_vec()))
                .collect(),
            run: r2,
        },
        Scenario {
            id: "R10",
            systems: Systems::Both,
            racy: false,
            variants: vec![(R10_VARIANT, R10_CHECKS.to_vec())],
            run: r10,
        },
    ]
}

/// The prefix of the sort folders the CanopyDB generator makes in its temporary folder
/// (`crime-canopy-generator-sort-<pid>…`).
const SORT_DIR_PREFIX: &str = "crime-canopy-generator-sort-";

/// The KG6 notice of leftovers of a version (orphan version folders, `CURRENT.tmp`).
const LEFTOVERS_NOTICE: &str = "Se han descartado restos de una generación interrumpida anterior.";

/// The KG6 notice of sort folders left in the temporary folder.
const TEMPORARIES_NOTICE: &str =
    "Se han descartado carpetas temporales de una generación interrumpida anterior.";

/// The start of the refusal of a generation whose output folder another one holds.
const LOCKED_NOTICE: &str = "Otra generación está usando la carpeta de salida";

/// The cancellation message of the generator (exit code 130).
const CANCELLED_NOTICE: &str =
    "Generación cancelada por el operador. No se ha modificado el conjunto existente.";

/// The start of the summary line of a published generation.
const PUBLISHED_NOTICE: &str = "Conjunto de datos publicado en ";

/// The start of the line the generator logs once the final dump of the base has ended.
const DUMP_ENDED_NOTICE: &str = "Índices generados:";

/// How long the app pauses in a short phase so that a signal lands in it.
const SHORT_PHASE_PAUSE_MS: u64 = 3000;

/// How a signal reaches its phase.
#[derive(Debug, Clone, Copy)]
enum Timing {
    /// A long phase: the signal goes this many milliseconds after its marker.
    After(u64),
    /// A short phase: the app pauses this many milliseconds after its marker and the signal goes
    /// during the pause.
    Paused(u64),
}

/// A phase of a generation where a signal is sent.
#[derive(Debug, Clone, Copy)]
struct Phase {
    /// Its name in the variant ids.
    name: &'static str,
    /// R1's variant for it with a previous dataset.
    with_dataset: &'static str,
    /// R1's variant for it without a previous dataset.
    without_dataset: &'static str,
    /// The marker the generator writes when it enters it.
    marker: &'static str,
    timing: Timing,
    /// Whether `CURRENT` already names the new version in it.
    switched: bool,
}

const READ: Phase = Phase {
    name: "lectura",
    with_dataset: "lectura/con-conjunto",
    without_dataset: "lectura/sin-conjunto",
    marker: phase::READ,
    timing: Timing::After(2000),
    switched: false,
};
const PRIMARY_LOAD: Phase = Phase {
    name: "carga",
    with_dataset: "carga/con-conjunto",
    without_dataset: "carga/sin-conjunto",
    marker: phase::PRIMARY_LOAD,
    timing: Timing::After(2000),
    switched: false,
};
const INDEX_WRITE: Phase = Phase {
    name: "indices",
    with_dataset: "indices/con-conjunto",
    without_dataset: "indices/sin-conjunto",
    marker: phase::INDEX_WRITE,
    timing: Timing::After(1000),
    switched: false,
};
/// The final dump lasts too little to hit it reliably after its marker, so it is stretched too:
/// the signal lands after the marker and before the dump ends.
const DUMP: Phase = Phase {
    name: "volcado",
    with_dataset: "volcado/con-conjunto",
    without_dataset: "volcado/sin-conjunto",
    marker: phase::DUMP,
    timing: Timing::Paused(SHORT_PHASE_PAUSE_MS),
    switched: false,
};
const MANIFEST: Phase = Phase {
    name: "manifiesto",
    with_dataset: "manifiesto/con-conjunto",
    without_dataset: "manifiesto/sin-conjunto",
    marker: phase::MANIFEST,
    timing: Timing::Paused(SHORT_PHASE_PAUSE_MS),
    switched: false,
};
const CURRENT_TMP: Phase = Phase {
    name: "puntero",
    with_dataset: "puntero/con-conjunto",
    without_dataset: "puntero/sin-conjunto",
    marker: phase::CURRENT_TMP,
    timing: Timing::Paused(SHORT_PHASE_PAUSE_MS),
    switched: false,
};
const SWITCHED: Phase = Phase {
    name: "cambiado",
    with_dataset: "cambiado/con-conjunto",
    without_dataset: "cambiado/sin-conjunto",
    marker: phase::SWITCHED,
    timing: Timing::Paused(SHORT_PHASE_PAUSE_MS),
    switched: true,
};

/// R1's phases, in the order a generation enters them.
const R1_PHASES: [Phase; 7] = [
    READ,
    PRIMARY_LOAD,
    INDEX_WRITE,
    DUMP,
    MANIFEST,
    CURRENT_TMP,
    SWITCHED,
];

/// R2's phases.
const R2_PHASES: [Phase; 5] = [READ, PRIMARY_LOAD, INDEX_WRITE, DUMP, SWITCHED];

/// The checks after a kill that every R1 variant (and R10) shares, after the one on the
/// published version: the next generation's.
const NEXT_GENERATION_CHECKS: [&str; 5] = [
    "siguiente-sin-bloqueo",
    "restos",
    "temporales",
    "siguiente-codigo",
    "contenido",
];

const R10_VARIANT: &str = "carga/con-conjunto";

const R10_CHECKS: [&str; 9] = [
    "lote-codigo",
    "lote-resultado",
    "lote-version",
    "version",
    "siguiente-sin-bloqueo",
    "restos",
    "temporales",
    "siguiente-codigo",
    "contenido",
];

/// The variant name of R1 for `phase`, with a previous dataset or without it.
fn r1_variant_name(phase: &Phase, with_dataset: bool) -> &'static str {
    if with_dataset {
        phase.with_dataset
    } else {
        phase.without_dataset
    }
}

fn r1_variants() -> Vec<(&'static str, Vec<&'static str>)> {
    R1_PHASES
        .iter()
        .flat_map(|phase| {
            [true, false].into_iter().map(move |with_dataset| {
                (
                    r1_variant_name(phase, with_dataset),
                    r1_checks(phase, with_dataset),
                )
            })
        })
        .collect()
}

/// R1's checks: the published version, or the refusal of the folder when there was no dataset
/// before the switch, then the next generation's.
fn r1_checks(phase: &Phase, with_dataset: bool) -> Vec<&'static str> {
    let first = if with_dataset || phase.switched {
        "version"
    } else {
        "rechazo"
    };
    std::iter::once(first)
        .chain(NEXT_GENERATION_CHECKS)
        .collect()
}

/// R2's checks: a cancellation up to the dump, a normal end after the switch.
fn r2_checks(phase: &Phase) -> &'static [&'static str] {
    if phase.switched {
        &[
            "codigo",
            "resumen",
            "sin-cancelacion",
            "version",
            "contenido",
        ]
    } else if phase.marker == phase::DUMP {
        &[
            "codigo",
            "mensaje",
            "fin-del-volcado",
            "restos",
            "version",
            "contenido",
        ]
    } else {
        &["codigo", "mensaje", "restos", "version", "contenido"]
    }
}

/// What an interrupted generation left behind (KG6).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Leftovers {
    /// Version folders that `CURRENT` does not name.
    versions: Vec<String>,
    /// Whether `CURRENT.tmp` is in the output folder.
    current_tmp: bool,
    /// Sort folders of the CanopyDB generator in the temporary folder.
    sort_dirs: Vec<String>,
}

impl Leftovers {
    /// What is left in `output` and in the temporary folder `temp`.
    fn find(output: &Path, temp: &Path) -> Self {
        let current = current_version(output);
        Self {
            versions: context::version_folders(output)
                .into_iter()
                .filter(|name| Some(name) != current.as_ref())
                .collect(),
            current_tmp: output.join("CURRENT.tmp").exists(),
            sort_dirs: context::entries_starting_with(temp, SORT_DIR_PREFIX),
        }
    }

    /// Whether the next generation must say that it discarded leftovers of a version.
    fn expects_leftovers_notice(&self) -> bool {
        !self.versions.is_empty() || self.current_tmp
    }

    /// Whether the next generation must say that it discarded temporary folders.
    fn expects_temporaries_notice(&self) -> bool {
        !self.sort_dirs.is_empty()
    }

    fn is_empty(&self) -> bool {
        !self.expects_leftovers_notice() && !self.expects_temporaries_notice()
    }

    fn describe(&self) -> String {
        format!(
            "versiones huérfanas: {:?}; CURRENT.tmp: {}; carpetas de ordenación: {:?}",
            self.versions,
            if self.current_tmp { "sí" } else { "no" },
            self.sort_dirs
        )
    }
}

/// Adds the check `name` that a KG6 notice appears in `stderr` exactly when `expected`.
fn check_notice(
    result: &mut VariantResult,
    name: &str,
    notice: &str,
    expected: bool,
    stderr: &str,
) {
    let shown = stderr.lines().any(|line| line == notice);
    let detail = if expected {
        format!("quedó algo y la siguiente generación no dijo `{notice}`")
    } else {
        format!("no quedó nada y la siguiente generación dijo `{notice}`")
    };
    result.check(name, shown == expected, detail);
}

/// A generation interrupted in its phase.
struct Interrupted {
    output: PathBuf,
    /// The version `CURRENT` named before the generation, if any.
    old: Option<String>,
    ended: Ended,
    stdout: String,
    stderr: String,
}

/// The output folder of the variant `id`: a fresh copy of the reference dataset, or an empty
/// place for one.
fn fresh_output(context: &Context, id: &str, with_dataset: bool) -> io::Result<PathBuf> {
    context.fresh_output(&id.replace('/', "-"), with_dataset)
}

/// Starts a generation into `output` (with `--overwrite` when it holds a dataset) and waits
/// until it is in `phase`: `Err` with why it never got there.
fn start_in_phase(
    context: &Context,
    id: &str,
    phase: &Phase,
    output: &Path,
    ctrl_c: bool,
) -> io::Result<Result<Running, String>> {
    let overwrite = current_version(output).is_some();
    let mut launch = context.generation(&context.csv, output, overwrite, id);
    if let Timing::Paused(millis) = phase.timing {
        launch = launch.pause(phase.marker, millis);
    }
    if ctrl_c {
        launch = launch.with_ctrl_c();
    }
    let mut running = launch.start()?;
    if !running.wait_phase(phase.marker, GENERATION_TIMEOUT) {
        let code = running.wait(Duration::ZERO).map(|ended| ended.code);
        return Ok(Err(format!(
            "el generador terminó con {code:?} sin llegar a la fase {} (fases: {:?}; stderr: {:?})",
            phase.marker,
            running.phases(),
            running.stderr().trim()
        )));
    }
    if let Timing::After(millis) = phase.timing {
        thread::sleep(Duration::from_millis(millis));
    }
    Ok(Ok(running))
}

/// Kills `running` and waits for its end: `Err` with why the kill did not land in `phase`.
fn kill_in(running: &mut Running, phase: &Phase) -> io::Result<Result<Ended, String>> {
    if !running.kill() {
        return Ok(Err("el generador ya había terminado".to_owned()));
    }
    let ended = running
        .wait(GENERATION_TIMEOUT)
        .ok_or_else(|| io::Error::other("el generador no terminó tras matarlo"))?;
    // A killed process marks nothing more, so its last phase is the one the kill landed in.
    match running.last_phase() {
        Some(last) if last == phase.marker => Ok(Ok(ended)),
        last => Ok(Err(format!("la señal llegó en la fase {last:?}"))),
    }
}

/// The version a query of F1 reads in `output`, with its exit code and stderr.
fn query_version(
    context: &Context,
    output: &Path,
    label: &str,
) -> io::Result<(Option<i32>, Option<String>, String)> {
    let (code, stdout, stderr) = run_to_end(
        context.query(output, &context.fixed_args(1), label),
        QUERY_TIMEOUT,
    )?;
    Ok((code, json_version(&stdout), stderr))
}

/// R1's (and R10's) check of what a query reads after the kill: the previous version before the
/// switch, the refusal of the folder when there was no previous dataset, the new version after
/// the switch.
fn check_published_after_kill(
    context: &Context,
    result: &mut VariantResult,
    phase: &Phase,
    interrupted: &Interrupted,
) -> io::Result<()> {
    let output = &interrupted.output;
    let (code, read, stderr) = query_version(context, output, &format!("{}-query", result.id))?;
    let now = current_version(output);
    result.observe(format!(
        "una consulta F1 dio el código {code:?} y la versión {read:?} (antes {:?}, CURRENT ahora {now:?})",
        interrupted.old
    ));
    if phase.switched {
        result.check(
            "version",
            code == Some(0) && read.is_some() && read == now && now != interrupted.old,
            format!(
                "se esperaba la versión nueva {now:?}, distinta de {:?}; se leyó {read:?} con el código {code:?}",
                interrupted.old
            ),
        );
    } else if interrupted.old.is_some() {
        result.check(
            "version",
            code == Some(0) && read == interrupted.old,
            format!(
                "se esperaba la versión anterior {:?}; se leyó {read:?} con el código {code:?}",
                interrupted.old
            ),
        );
    } else {
        let refusal = format!(
            "No se encontró un conjunto de datos válido en {}.",
            output.display()
        );
        result.check(
            "rechazo",
            code == Some(1) && stderr.lines().any(|line| line == refusal),
            format!(
                "se esperaba `{refusal}` y el código 1; se obtuvo el código {code:?} y {:?}",
                stderr.trim()
            ),
        );
    }
    Ok(())
}

/// The checks of the generation that follows a kill (R1, R10): it is not refused as another
/// generation in progress, says the KG6 notices of what was left and only those, ends with 0 and
/// publishes the content of a clean generation.
fn check_next_generation(
    context: &Context,
    result: &mut VariantResult,
    output: &Path,
) -> io::Result<()> {
    let left = Leftovers::find(output, &context.temp_dir());
    result.observe(format!("quedó: {}", left.describe()));
    let overwrite = current_version(output).is_some();
    let launch = context.generation(
        &context.csv,
        output,
        overwrite,
        &format!("{}-next", result.id),
    );
    let mut running = launch.start()?;
    let ended = running
        .wait(GENERATION_TIMEOUT)
        .ok_or_else(|| io::Error::other("la siguiente generación no terminó"))?;
    let stderr = running.stderr();
    result.observe(format!(
        "la siguiente generación{} terminó con {:?} a los {:.1} s",
        if overwrite { " (con --overwrite)" } else { "" },
        ended.code,
        ended.after_start.as_secs_f64()
    ));
    result.check(
        "siguiente-sin-bloqueo",
        !stderr.contains(LOCKED_NOTICE),
        format!(
            "la siguiente generación informó de otra en curso: {}",
            stderr.trim()
        ),
    );
    check_notice(
        result,
        "restos",
        LEFTOVERS_NOTICE,
        left.expects_leftovers_notice(),
        &stderr,
    );
    check_notice(
        result,
        "temporales",
        TEMPORARIES_NOTICE,
        left.expects_temporaries_notice(),
        &stderr,
    );
    result.check_eq("siguiente-codigo", Some(0), ended.code);
    check_content(context, result, output)
}

/// The check `contenido`: the published content of `output` is that of a clean generation (KG8).
fn check_content(context: &Context, result: &mut VariantResult, output: &Path) -> io::Result<()> {
    let reference = context.reference_dataset()?;
    match context.compare(output, &reference)? {
        Ok(()) => {
            result.check("contenido", true, "");
        }
        Err(report) => {
            result.check(
                "contenido",
                false,
                format!("el contenido publicado no es el de una generación limpia: {report}"),
            );
        }
    }
    Ok(())
}

/// Deletes the copy of a variant whose checks all held, to save disk; a failed one is kept as
/// evidence.
fn drop_copy_if_met(result: &VariantResult, output: &Path) {
    if result.checks.iter().all(|check| check.ok)
        && let Some(root) = output.parent()
    {
        let _ = fs::remove_dir_all(root);
    }
}

/// R1: kill the generator in each phase, with a previous dataset and without it.
fn r1(context: &Context) -> Vec<VariantResult> {
    R1_PHASES
        .iter()
        .flat_map(|phase| [true, false].map(|with_dataset| (phase, with_dataset)))
        .map(|(phase, with_dataset)| {
            let id = format!("R1/{}", r1_variant_name(phase, with_dataset));
            context::progress(&id);
            r1_variant(context, &id, phase, with_dataset).unwrap_or_else(|result| *result)
        })
        .collect()
}

fn r1_variant(
    context: &Context,
    id: &str,
    phase: &Phase,
    with_dataset: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let (interrupted, misses) = land(id, || {
        let output = fresh_output(context, id, with_dataset)?;
        let old = current_version(&output);
        let mut running = match start_in_phase(context, id, phase, &output, false)? {
            Ok(running) => running,
            Err(why) => return Ok(Landed::Missed(why)),
        };
        let ended = match kill_in(&mut running, phase)? {
            Ok(ended) => ended,
            Err(why) => return Ok(Landed::Missed(why)),
        };
        Ok(Landed::Yes(Interrupted {
            output,
            old,
            ended,
            stdout: running.stdout(),
            stderr: running.stderr(),
        }))
    })?;
    let mut result = VariantResult::new(id);
    misses.into_iter().for_each(|miss| result.observe(miss));
    observe_signal(&mut result, "matado", phase, &interrupted.ended);
    after_kill(context, &mut result, phase, &interrupted)
        .map_err(|error| Box::new(failed_to_run(id, &error)))?;
    drop_copy_if_met(&result, &interrupted.output);
    Ok(result)
}

/// [`until_landed`], keeping why each missed attempt missed, so the variant can note them.
fn land<T>(
    id: &str,
    mut attempt: impl FnMut() -> io::Result<Landed<T>>,
) -> Result<(T, Vec<String>), Box<VariantResult>> {
    let mut misses = Vec::new();
    let landed = until_landed(id, |number| {
        let outcome = attempt();
        if let Ok(Landed::Missed(why)) = &outcome {
            misses.push(format!("intento {number} no contó: {why}"));
        }
        outcome
    });
    landed.map(|value| (value, misses)).map_err(Box::new)
}

/// What R1 and R10 check after the kill.
fn after_kill(
    context: &Context,
    result: &mut VariantResult,
    phase: &Phase,
    interrupted: &Interrupted,
) -> io::Result<()> {
    check_published_after_kill(context, result, phase, interrupted)?;
    check_next_generation(context, result, &interrupted.output)
}

fn observe_signal(result: &mut VariantResult, what: &str, phase: &Phase, ended: &Ended) {
    let to_end = ended.after_signal.unwrap_or_default();
    result.observe(format!(
        "{what} en la fase {} a los {:.1} s de empezar; terminó con {:?} a los {:.2} s de la señal",
        phase.marker,
        (ended.after_start.saturating_sub(to_end)).as_secs_f64(),
        ended.code,
        to_end.as_secs_f64()
    ));
}

/// R2: Ctrl+C to the generator in each phase, with a previous dataset.
fn r2(context: &Context) -> Vec<VariantResult> {
    R2_PHASES
        .iter()
        .map(|phase| {
            let id = format!("R2/{}", phase.name);
            context::progress(&id);
            r2_variant(context, &id, phase).unwrap_or_else(|result| *result)
        })
        .collect()
}

fn r2_variant(
    context: &Context,
    id: &str,
    phase: &Phase,
) -> Result<VariantResult, Box<VariantResult>> {
    let (interrupted, misses) = land(id, || {
        let output = fresh_output(context, id, true)?;
        let old = current_version(&output);
        let mut running = match start_in_phase(context, id, phase, &output, true)? {
            Ok(running) => running,
            Err(why) => return Ok(Landed::Missed(why)),
        };
        if !running.ctrl_c()? {
            return Ok(Landed::Missed("el generador ya había terminado".to_owned()));
        }
        // A cancelled generation may still mark the phases it goes through on its way out (the
        // dump ends and the publication starts), so the landing is read right after the signal.
        let at_signal = running.last_phase();
        let ended = running
            .wait(GENERATION_TIMEOUT)
            .ok_or_else(|| io::Error::other("el generador no terminó tras Ctrl+C"))?;
        if at_signal.as_deref() != Some(phase.marker) {
            return Ok(Landed::Missed(format!(
                "la señal llegó en la fase {at_signal:?}"
            )));
        }
        Ok(Landed::Yes(Interrupted {
            output,
            old,
            ended,
            stdout: running.stdout(),
            stderr: running.stderr(),
        }))
    })?;
    let mut result = VariantResult::new(id);
    misses.into_iter().for_each(|miss| result.observe(miss));
    observe_signal(&mut result, "Ctrl+C", phase, &interrupted.ended);
    r2_checks_of(context, &mut result, phase, &interrupted)
        .map_err(|error| Box::new(failed_to_run(id, &error)))?;
    drop_copy_if_met(&result, &interrupted.output);
    Ok(result)
}

fn r2_checks_of(
    context: &Context,
    result: &mut VariantResult,
    phase: &Phase,
    interrupted: &Interrupted,
) -> io::Result<()> {
    let output = &interrupted.output;
    let said = format!("{}{}", interrupted.stdout, interrupted.stderr);
    let cancelled = said.lines().any(|line| line == CANCELLED_NOTICE);
    let (code, read, _) = query_version(context, output, &format!("{}-query", result.id))?;
    let now = current_version(output);
    result.observe(format!(
        "una consulta F1 dio el código {code:?} y la versión {read:?} (antes {:?}, CURRENT ahora {now:?})",
        interrupted.old
    ));
    if phase.switched {
        result.check_eq("codigo", Some(0), interrupted.ended.code);
        result.check(
            "resumen",
            said.lines().any(|line| line.starts_with(PUBLISHED_NOTICE)),
            format!("falta el resumen final (`{PUBLISHED_NOTICE}…`)"),
        );
        result.check(
            "sin-cancelacion",
            !cancelled,
            format!("dijo `{CANCELLED_NOTICE}` tras el cambio de versión"),
        );
        result.check(
            "version",
            code == Some(0) && read.is_some() && read == now && now != interrupted.old,
            format!(
                "se esperaba la versión nueva {now:?}, distinta de {:?}; se leyó {read:?} con el código {code:?}",
                interrupted.old
            ),
        );
    } else {
        result.check_eq("codigo", Some(130), interrupted.ended.code);
        result.check(
            "mensaje",
            cancelled,
            format!(
                "falta `{CANCELLED_NOTICE}` en: {}",
                interrupted.stderr.trim()
            ),
        );
        if phase.marker == phase::DUMP {
            result.check(
                "fin-del-volcado",
                said.lines().any(|line| line.starts_with(DUMP_ENDED_NOTICE)),
                format!("terminó sin acabar el volcado (falta `{DUMP_ENDED_NOTICE}…`)"),
            );
        }
        let left = Leftovers::find(output, &context.temp_dir());
        result.observe(format!("quedó: {}", left.describe()));
        result.check(
            "restos",
            left.is_empty(),
            format!("quedó {}", left.describe()),
        );
        result.check(
            "version",
            code == Some(0) && read == interrupted.old,
            format!(
                "se esperaba la versión anterior {:?}; se leyó {read:?} con el código {code:?}",
                interrupted.old
            ),
        );
    }
    check_content(context, result, output)
}

/// R10: kill the generator during the primary load while a batch with the fixed query set reads
/// the previous dataset: the batch ends with 0 and the result of a clean run over the previous
/// version, and then what R1 expects for that phase with a previous dataset.
fn r10(context: &Context) -> Vec<VariantResult> {
    let id = format!("R10/{R10_VARIANT}");
    context::progress(&id);
    vec![r10_variant(context, &id).unwrap_or_else(|result| *result)]
}

/// What R10's batch did.
struct BatchRun {
    code: Option<i32>,
    /// The query the batch was on when the generator was killed (1 to 11).
    query_at_kill: usize,
    /// How long the batch ran.
    lasted: Duration,
    stdout: String,
    stderr: String,
}

/// How many queries `run` has started.
fn queries_started(run: &Running) -> usize {
    run.phases()
        .iter()
        .filter(|marked| *marked == phase::QUERY)
        .count()
}

fn r10_variant(context: &Context, id: &str) -> Result<VariantResult, Box<VariantResult>> {
    let phase = &PRIMARY_LOAD;
    let failed = |error: io::Error| Box::new(failed_to_run(id, &error));
    // The clean run: the same batch alone over the same copy, before any generation.
    let clean = {
        let output = fresh_output(context, id, true).map_err(failed)?;
        let launch = context
            .batch(&output, &context.fixed_batch_text(), &format!("{id}-clean"))
            .map_err(failed)?;
        let (code, stdout, stderr) = run_to_end(launch, QUERY_TIMEOUT).map_err(failed)?;
        if code != Some(0) {
            return Err(failed(io::Error::other(format!(
                "el lote limpio terminó con {code:?}: {}",
                stderr.trim()
            ))));
        }
        stdout
    };
    let ((interrupted, batch), misses) = land(id, || {
        let output = fresh_output(context, id, true)?;
        let old = current_version(&output);
        let mut generator = match start_in_phase(context, id, phase, &output, false)? {
            Ok(running) => running,
            Err(why) => return Ok(Landed::Missed(why)),
        };
        let mut reader = context
            .batch(&output, &context.fixed_batch_text(), &format!("{id}-batch"))?
            .start()?;
        // The batch has opened the dataset and is reading it once it starts its first query.
        let reading = reader.wait_until(QUERY_TIMEOUT, |run| {
            run.phases().iter().any(|marked| marked == phase::QUERY)
        });
        if !reading || reader.has_ended() {
            return Ok(Landed::Missed(
                "el lote terminó antes de matar el generador".to_owned(),
            ));
        }
        let query_at_kill = queries_started(&reader);
        let ended = match kill_in(&mut generator, phase)? {
            Ok(ended) => ended,
            Err(why) => return Ok(Landed::Missed(why)),
        };
        let still_reading = !reader.has_ended();
        let batch_ended = reader
            .wait(QUERY_TIMEOUT)
            .ok_or_else(|| io::Error::other("el lote no terminó"))?;
        if !still_reading {
            return Ok(Landed::Missed(
                "el lote terminó antes de que muriera el generador".to_owned(),
            ));
        }
        Ok(Landed::Yes((
            Interrupted {
                output,
                old,
                ended,
                stdout: generator.stdout(),
                stderr: generator.stderr(),
            },
            BatchRun {
                code: batch_ended.code,
                query_at_kill,
                lasted: batch_ended.after_start,
                stdout: reader.stdout(),
                stderr: reader.stderr(),
            },
        )))
    })?;
    let mut result = VariantResult::new(id);
    misses.into_iter().for_each(|miss| result.observe(miss));
    observe_signal(&mut result, "matado", phase, &interrupted.ended);
    result.observe(format!(
        "el lote iba por la consulta F{} al matar el generador y terminó con {:?} a los {:.1} s de empezar",
        batch.query_at_kill,
        batch.code,
        batch.lasted.as_secs_f64()
    ));
    result.check_eq("lote-codigo", Some(0), batch.code);
    result.check(
        "lote-resultado",
        batch.stdout == clean,
        format!(
            "la salida del lote ({} bytes) no es la de una ejecución limpia ({} bytes); stderr: {}",
            batch.stdout.len(),
            clean.len(),
            batch.stderr.trim()
        ),
    );
    let old = interrupted.old.clone().unwrap_or_default();
    result.check(
        "lote-version",
        !old.is_empty() && batch.stdout.contains(&format!("\"{old}\"")),
        format!("la salida del lote no nombra la versión anterior {old:?}"),
    );
    after_kill(context, &mut result, phase, &interrupted).map_err(failed)?;
    drop_copy_if_met(&result, &interrupted.output);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftovers_of_a_version_ask_only_for_their_notice() {
        let left = Leftovers {
            versions: vec!["dataset-20261022-101010-7".to_owned()],
            ..Leftovers::default()
        };
        assert!(left.expects_leftovers_notice());
        assert!(!left.expects_temporaries_notice());
        let pointer = Leftovers {
            current_tmp: true,
            ..Leftovers::default()
        };
        assert!(pointer.expects_leftovers_notice());
        assert!(!pointer.expects_temporaries_notice());
    }

    #[test]
    fn sort_folders_ask_only_for_the_temporaries_notice() {
        let left = Leftovers {
            sort_dirs: vec!["crime-canopy-generator-sort-42-1".to_owned()],
            ..Leftovers::default()
        };
        assert!(!left.expects_leftovers_notice());
        assert!(left.expects_temporaries_notice());
        assert!(!left.is_empty());
    }

    #[test]
    fn nothing_left_asks_for_no_notice() {
        let left = Leftovers::default();
        assert!(!left.expects_leftovers_notice());
        assert!(!left.expects_temporaries_notice());
        assert!(left.is_empty());
    }

    #[test]
    fn leftovers_are_the_unnamed_versions_the_pointer_and_the_sort_folders() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("out");
        let temp = dir.path().join("tmp");
        for folder in [
            output.join("dataset-20261022-101010-7"),
            output.join("dataset-20261022-111111-8"),
            temp.join("crime-canopy-generator-sort-8-1"),
            temp.join("crime-generator-sort-9-1"),
        ] {
            fs::create_dir_all(folder).unwrap();
        }
        fs::write(output.join("CURRENT"), "dataset-20261022-101010-7\n").unwrap();
        fs::write(output.join("CURRENT.tmp"), "dataset-20261022-111111-8\n").unwrap();
        assert_eq!(
            Leftovers::find(&output, &temp),
            Leftovers {
                versions: vec!["dataset-20261022-111111-8".to_owned()],
                current_tmp: true,
                sort_dirs: vec!["crime-canopy-generator-sort-8-1".to_owned()],
            }
        );
    }

    #[test]
    fn every_variant_has_its_own_checks() {
        let scenarios = scenarios();
        let r1 = &scenarios[0];
        assert_eq!(r1.variants.len(), 14);
        let checks = |name: &str| {
            r1.variants
                .iter()
                .find(|(variant, _)| *variant == name)
                .map(|(_, checks)| checks.clone())
                .unwrap()
        };
        assert_eq!(checks("lectura/sin-conjunto")[0], "rechazo");
        assert_eq!(checks("lectura/con-conjunto")[0], "version");
        assert_eq!(checks("cambiado/sin-conjunto")[0], "version");
        let r2 = &scenarios[1];
        assert_eq!(
            r2.variants
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            ["lectura", "carga", "indices", "volcado", "cambiado"]
        );
    }
}
