//! R8, R9, R11 and R12: several query apps, and query apps during a publication
//! (canopy-db-robustez PRD, "Escenarios"; decisions KR4 and KR7).
//!
//! The four are racy: the runner repeats each one [`Context::repetitions`] times and merges the
//! repetitions, so each `run` here does one repetition. Every variant works on its own copy of
//! the reference dataset ([`Context::fresh_output`]).
//!
//! Timing comes from the hidden phase pauses: a query app paused at `opened` keeps the dataset
//! open for a precise time before it runs its queries, and a generator paused at `current-tmp`
//! waits right before its switch, so that a holder and a waiter can be in place first.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crime_dataset::probe::phase;
use serde_json::Value;

use super::{Landed, Scenario, Systems, failed_to_run, until_landed};
use crate::context::{
    CleanRun, Context, GENERATION_TIMEOUT, QUERY_TIMEOUT, current_version, json_version,
    version_folders,
};
use crate::process::{Ended, Running, run_to_end};
use crate::report::VariantResult;

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            id: "R8",
            systems: Systems::Both,
            racy: true,
            variants: vec![("a", R8_A_CHECKS.to_vec()), ("b", R8_B_CHECKS.to_vec())],
            run: r8,
        },
        Scenario {
            id: "R9",
            systems: Systems::Both,
            racy: true,
            variants: vec![("publicaciones", R9_CHECKS.to_vec())],
            run: r9,
        },
        Scenario {
            id: "R11",
            systems: Systems::LinuxOnly,
            racy: true,
            variants: vec![
                ("consultor-f11", R11_CHECKS.to_vec()),
                ("lote", R11_CHECKS.to_vec()),
            ],
            run: r11,
        },
        Scenario {
            id: "R12",
            systems: Systems::LinuxOnly,
            racy: true,
            variants: vec![("lote-y-espera", R12_CHECKS.to_vec())],
            run: r12,
        },
    ]
}

const R8_A_CHECKS: [&str; 3] = ["un-aviso", "codigos", "resultado"];
const R8_B_CHECKS: [&str; 4] = ["un-aviso", "codigos", "mensaje", "rechazo-a-tiempo"];
const R9_CHECKS: [&str; 5] = [
    "generaciones",
    "consultas",
    "rechazos",
    "sin-restos",
    "contenido",
];
const R11_CHECKS: [&str; 6] = [
    "codigo",
    "resultado",
    "version",
    "generacion",
    "sin-restos",
    "contenido",
];
const R12_CHECKS: [&str; 8] = [
    "lote",
    "un-aviso",
    "abre-sin-esperar",
    "version-nueva",
    "resultado",
    "a-tiempo",
    "generacion",
    "contenido",
];

/// How often a scenario looks at the runs it follows.
const POLL: Duration = Duration::from_millis(10);

/// The start of the wait warning of KC2.
const WARNING_START: &str = "Aviso: otro proceso tiene abierto el conjunto de datos en ";

/// KC2: the rejection comes between 30 and 35 s after the warning.
const REJECTION_WINDOW: (Duration, Duration) = (Duration::from_secs(30), Duration::from_secs(35));

/// KC2 and KR7: the wait never lasts more than 30 s from the warning.
const WAIT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------------------------
// R8: four waiters and a batch that releases the dataset.
// ---------------------------------------------------------------------------------------------

/// How many query apps wait in R8.
const R8_QUERIERS: usize = 4;

/// How long the batch keeps the dataset open in variant A: the waiters start and warn within a
/// second of its opening, so it ends about 6 s after the last warning (the PRD asks 5 to 15 s).
/// Near the low end on purpose: the four counts then open one after another, and under load one
/// F3 took up to 8 s, so the last one must still fit in its 30 s.
const R8_HOLD_A: Duration = Duration::from_secs(7);

/// Variant A: the batch ends between 5 and 15 s after the last warning.
const R8_WINDOW_A: (Duration, Duration) = (Duration::from_secs(5), Duration::from_secs(15));

/// How long the batch keeps the dataset open in variant B: at least 45 s after the last warning.
const R8_HOLD_B: Duration = Duration::from_secs(50);

/// Variant B: the batch lasts at least 45 s more than the last warning.
const R8_MIN_B: Duration = Duration::from_secs(45);

/// R8: four counts F3 launched together wait for a batch that has the dataset open. A: the batch
/// ends 5 to 15 s after the last warning, and each one warns once and answers with 0 and the clean
/// result. B: the batch lasts 45 s more, and each one warns once and is rejected with KC2's
/// message and 1, 30 to 35 s after its warning. The order they ended in is only observed (KR4).
fn r8(context: &Context) -> Vec<VariantResult> {
    let clean = context.clean_run(&context.fixed_args(3));
    [("a", false), ("b", true)]
        .iter()
        .map(|&(variant, long)| {
            let id = format!("R8/{variant}");
            match &clean {
                Ok(clean) => r8_variant(context, clean, &id, long).unwrap_or_else(|result| *result),
                Err(error) => failed_to_run(&id, error),
            }
        })
        .collect()
}

fn r8_variant(
    context: &Context,
    clean: &CleanRun,
    id: &str,
    long: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let hold = if long { R8_HOLD_B } else { R8_HOLD_A };
    let (dataset, runs, holder_after) = until_landed(id, |_| {
        let dataset = context.fresh_output(&copy_name(id), true)?;
        let text = format!("{}\n", context.fixed_line(4));
        let mut holder = context
            .batch(&dataset, &text, &format!("{id}-lote"))?
            .pause(phase::OPENED, millis(hold))
            .start()?;
        if !holder.wait_phase(phase::OPENED, QUERY_TIMEOUT) {
            return Err(did_not_open("el lote", &holder));
        }
        let mut runs = vec![Watched::new(holder)];
        for number in 1..=R8_QUERIERS {
            let launch = context.query(&dataset, &context.fixed_args(3), &format!("{id}-{number}"));
            runs.push(Watched::new(launch.start()?));
        }
        watch(&mut runs, hold + QUERY_TIMEOUT)?;
        let holder_end = runs[0].ended_at().expect("watched to its end");
        let Some(last_warning) = runs[1..].iter().filter_map(|run| run.warned).max() else {
            // Nobody warned: the checks say so.
            return Ok(Landed::Yes((dataset, runs, None)));
        };
        let after = holder_end.saturating_duration_since(last_warning);
        let fits = if long {
            after >= R8_MIN_B
        } else {
            (R8_WINDOW_A.0..=R8_WINDOW_A.1).contains(&after)
        };
        if fits {
            Ok(Landed::Yes((dataset, runs, Some(after))))
        } else {
            Ok(Landed::Missed(format!(
                "el lote terminó {:.1} s después del último aviso",
                after.as_secs_f64()
            )))
        }
    })
    .map_err(Box::new)?;

    let (holder, queriers) = runs.split_first().expect("the holder is first");
    let mut result = VariantResult::new(id);
    let mut order: Vec<(Instant, usize)> = queriers
        .iter()
        .enumerate()
        .map(|(index, run)| (run.ended_at().expect("watched to its end"), index + 1))
        .collect();
    order.sort();
    result.observe(format!(
        "orden en que terminaron: {}; códigos: {}; desde su aviso: {}; el lote terminó con {:?} {} del último aviso",
        join(order.iter().map(|(_, number)| number.to_string())),
        join(queriers.iter().map(|run| format!("{:?}", run.code()))),
        join(queriers.iter().map(Watched::since_warning)),
        holder.code(),
        holder_after.map_or("sin aviso".to_owned(), |after| format!(
            "a los {:.1} s",
            after.as_secs_f64()
        )),
    ));
    let warnings: Vec<usize> = queriers
        .iter()
        .map(|run| warning_count(&run.run.stderr()))
        .collect();
    result.check(
        "un-aviso",
        queriers
            .iter()
            .all(|run| one_warning(&run.run.stderr(), &dataset)),
        format!("avisos de cada consultor: {warnings:?}; se esperaba uno, el de KC2"),
    );
    let expected = if long { 1 } else { 0 };
    let codes: Vec<Option<i32>> = queriers.iter().map(Watched::code).collect();
    result.check(
        "codigos",
        codes.iter().all(|code| *code == Some(expected)),
        format!("códigos {codes:?}; se esperaba {expected} en todos"),
    );
    if long {
        let rejection = rejection_line(&dataset);
        result.check(
            "mensaje",
            queriers
                .iter()
                .all(|run| run.run.stderr().lines().any(|line| line == rejection)),
            format!("falta «{rejection}» en algún consultor"),
        );
        result.check(
            "rechazo-a-tiempo",
            queriers
                .iter()
                .all(|run| run.waited().is_some_and(in_rejection_window)),
            format!(
                "desde su aviso: {}; se esperaba entre 30 y 35 s",
                join(queriers.iter().map(Watched::since_warning))
            ),
        );
    } else {
        result.check(
            "resultado",
            queriers.iter().all(|run| run.run.stdout() == clean.stdout),
            format!(
                "salidas {:?}; la ejecución limpia da {:?}",
                queriers
                    .iter()
                    .map(|run| run.run.stdout().trim().to_owned())
                    .collect::<Vec<_>>(),
                clean.stdout.trim()
            ),
        );
    }
    Ok(result)
}

// ---------------------------------------------------------------------------------------------
// R9: four query apps while the generator publishes version after version.
// ---------------------------------------------------------------------------------------------

/// How many versions the generator publishes while the queries run (the PRD asks ten). Each one
/// takes about 1 min 40 s on the reference CSV: lower it locally to try R9 quickly, never in a
/// commit.
const R9_PUBLICATIONS: usize = 10;

/// How many query apps loop in R9.
const R9_QUERIERS: usize = 4;

/// The longest the dataset may be free, during the wait of a rejected query, for the others to
/// still count as having it open all that time (KR4: "aunque quedara libre por momentos").
const R9_MOMENT: Duration = Duration::from_secs(1);

/// One query of R9.
struct QueryRun {
    querier: usize,
    /// F3 or F2.
    number: usize,
    code: Option<i32>,
    stdout: String,
    stderr: String,
    warned: Option<Instant>,
    /// When the phase `opened` was seen: from then to its end it had the dataset open.
    opened: Option<Instant>,
    ended: Instant,
}

/// One generation of R9.
struct Generation {
    code: Option<i32>,
    /// Standard error and log of the run, or why it did not end.
    text: String,
    before: Option<String>,
    after: Option<String>,
    /// Whether the previous version was still there right after it ended.
    kept_previous: bool,
}

/// R9: four query apps, each repeating F3 and F2 with no pause, while the generator publishes
/// ten versions with `--overwrite`; then one more generation alone. Every generation ends with 0
/// and publishes (on Windows, a previous version it cannot delete is logged and discarded by a
/// later one); every query answers with 0, the clean result and the version it opened, or is
/// rejected with KC2's message and 1, 30 to 35 s after its warning, only if the others had the
/// dataset open all that time (KR4); after the last generation no previous version is left.
fn r9(context: &Context) -> Vec<VariantResult> {
    let id = "R9/publicaciones";
    vec![r9_run(context, id).unwrap_or_else(|error| failed_to_run(id, &error))]
}

fn r9_run(context: &Context, id: &str) -> io::Result<VariantResult> {
    let clean_count = context.clean_run(&context.fixed_args(3))?;
    let clean_case = context.clean_run(&context.fixed_args(2))?;
    let reference = context.reference_dataset()?;
    let output = context.fresh_output(&copy_name(id), true)?;
    let first = current_version(&output).ok_or_else(|| no_current(&output))?;
    let stop = AtomicBool::new(false);
    let runs = Mutex::new(Vec::new());
    let errors = Mutex::new(Vec::new());
    let mut generations = thread::scope(|scope| {
        for querier in 1..=R9_QUERIERS {
            let (output, stop, runs, errors) = (&output, &stop, &runs, &errors);
            scope.spawn(move || {
                if let Err(error) = r9_querier(context, output, querier, stop, runs) {
                    errors
                        .lock()
                        .expect("not poisoned")
                        .push(format!("consultor {querier}: {error}"));
                }
            });
        }
        let generations: Vec<Generation> = (1..=R9_PUBLICATIONS)
            .map(|number| r9_generation(context, &output, &format!("R9-generacion-{number}")))
            .collect();
        stop.store(true, Ordering::SeqCst);
        generations
    });
    generations.push(r9_generation(context, &output, "R9-generacion-final"));
    let runs = runs.into_inner().expect("not poisoned");
    let errors = errors.into_inner().expect("not poisoned");
    if !errors.is_empty() {
        return Err(io::Error::other(errors.join("; ")));
    }

    let mut result = VariantResult::new(id);
    let mut known: Vec<String> = vec![first];
    known.extend(generations.iter().filter_map(|run| run.after.clone()));

    // Generations: each ends with 0 and publishes; a previous version left behind is logged.
    let mut bad_generations = Vec::new();
    let mut kept = 0;
    for (index, run) in generations.iter().enumerate() {
        let published = run.code == Some(0) && run.after.is_some() && run.after != run.before;
        let left = run.before.as_ref().filter(|_| run.kept_previous);
        let logged = left.is_none_or(|before| {
            run.text
                .contains(&previous_version_kept(&output.join(before)))
        });
        if left.is_some() {
            kept += 1;
        }
        // The generation alone has nobody holding the previous version: it must delete it.
        let still_there = left.is_some() && index + 1 == generations.len();
        if !published || !logged || still_there {
            bad_generations.push(format!(
                "generación {}: código {:?}, versión {:?} → {:?}{}",
                index + 1,
                run.code,
                run.before,
                run.after,
                if logged {
                    String::new()
                } else {
                    " (dejó la anterior sin el aviso del flujo A, paso 5)".to_owned()
                }
            ));
        }
    }
    result.check(
        "generaciones",
        bad_generations.is_empty(),
        first_few(&bad_generations),
    );

    // Queries: the clean result and a known version, or a rejection.
    let mut failures = Vec::new();
    let mut rejections = Vec::new();
    for run in &runs {
        let warnings = warning_count(&run.stderr);
        let answered = run.code == Some(0)
            && warnings <= 1
            && if run.number == 3 {
                run.stdout == clean_count.stdout
            } else {
                same_records(&run.stdout, &clean_case.stdout)
                    && json_version(&run.stdout).is_some_and(|version| known.contains(&version))
            };
        if answered {
            continue;
        }
        if run.code == Some(1)
            && warnings == 1
            && run
                .stderr
                .lines()
                .any(|line| line == rejection_line(&output))
        {
            rejections.push(run);
        } else {
            failures.push(format!(
                "consultor {} con F{}: código {:?}, {} avisos, salida {:?}, errores {:?}",
                run.querier,
                run.number,
                run.code,
                warnings,
                shorten(run.stdout.trim()),
                shorten(run.stderr.trim())
            ));
        }
    }
    result.check("consultas", failures.is_empty(), first_few(&failures));

    let mut bad_rejections = Vec::new();
    for run in &rejections {
        let Some(warned) = run.warned else {
            bad_rejections.push(format!("consultor {}: sin aviso visto", run.querier));
            continue;
        };
        let waited = run.ended.saturating_duration_since(warned);
        let held: Vec<(Instant, Instant)> = runs
            .iter()
            .filter(|other| other.querier != run.querier)
            .filter_map(|other| other.opened.map(|opened| (opened, other.ended)))
            .collect();
        let others_held = covered(held, warned, run.ended, R9_MOMENT);
        if !in_rejection_window(waited) || !others_held {
            bad_rejections.push(format!(
                "consultor {} con F{}: rechazado a los {:.1} s de su aviso; los demás lo tuvieron abierto todo ese tiempo: {others_held}",
                run.querier,
                run.number,
                waited.as_secs_f64()
            ));
        }
    }
    result.check(
        "rechazos",
        bad_rejections.is_empty(),
        first_few(&bad_rejections),
    );

    let current = current_version(&output);
    let folders = version_folders(&output);
    result.check(
        "sin-restos",
        current
            .as_ref()
            .is_some_and(|current| folders == [current.clone()]),
        format!("tras la generación final quedan {folders:?}; CURRENT nombra {current:?}"),
    );
    check_content(context, &mut result, &reference, &output);

    let count = |number: usize| runs.iter().filter(|run| run.number == number).count();
    result.observe(format!(
        "{} generaciones ({} publicadas con consultores); {} consultas F3 y {} F2; {} rechazos; {} generaciones dejaron la versión anterior",
        generations.len(),
        R9_PUBLICATIONS,
        count(3),
        count(2),
        rejections.len(),
        kept
    ));
    Ok(result)
}

/// One query app of R9: F3 then F2, over and over, until `stop`.
fn r9_querier(
    context: &Context,
    output: &Path,
    querier: usize,
    stop: &AtomicBool,
    runs: &Mutex<Vec<QueryRun>>,
) -> io::Result<()> {
    while !stop.load(Ordering::SeqCst) {
        for number in [3, 2] {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let launch = context.query(
                output,
                &context.fixed_args(number),
                &format!("R9-consultor-{querier}-F{number}"),
            );
            let run_dir = launch.run_dir.clone();
            let mut watched = Watched::new(launch.start()?);
            let mut opened = None;
            let deadline = Instant::now() + QUERY_TIMEOUT;
            loop {
                watched.look();
                if opened.is_none() && watched.run.phases().iter().any(|p| p == phase::OPENED) {
                    opened = Some(Instant::now());
                }
                if watched.ended.is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::other(format!(
                        "F{number} no terminó en {} s",
                        QUERY_TIMEOUT.as_secs()
                    )));
                }
                thread::sleep(POLL);
            }
            let run = QueryRun {
                querier,
                number,
                code: watched.code(),
                stdout: watched.run.stdout(),
                stderr: watched.run.stderr(),
                warned: watched.warned,
                opened,
                ended: watched.ended_at().expect("watched to its end"),
            };
            drop(watched);
            // Thousands of queries: keep only the folders of the ones that did not answer.
            if run.code == Some(0) {
                let _ = fs::remove_dir_all(&run_dir);
            }
            runs.lock().expect("not poisoned").push(run);
        }
    }
    Ok(())
}

/// One generation of the reference CSV into `output` with `--overwrite`.
fn r9_generation(context: &Context, output: &Path, label: &str) -> Generation {
    let before = current_version(output);
    let launch = context.generation(&context.csv, output, true, label);
    let log = launch.run_dir.join("generator.log");
    let (code, text) = match run_to_end(launch, GENERATION_TIMEOUT) {
        Ok((code, _, stderr)) => (
            code,
            format!("{stderr}\n{}", fs::read_to_string(&log).unwrap_or_default()),
        ),
        Err(error) => (None, error.to_string()),
    };
    let after = current_version(output);
    let kept_previous = before
        .as_ref()
        .is_some_and(|before| after.as_ref() != Some(before) && output.join(before).exists());
    Generation {
        code,
        text,
        before,
        after,
        kept_previous,
    }
}

// ---------------------------------------------------------------------------------------------
// R11 and R12: a version deleted while a query app has it open (Linux only).
// ---------------------------------------------------------------------------------------------

/// How long the generator pauses right before its switch: time for the holder to open the
/// version it replaces and, in R12, for the waiter to start and warn.
const GENERATOR_PAUSE: Duration = Duration::from_secs(8);

/// How long the holder keeps the version open once it opened it, before it runs its queries: it
/// outlives the rest of the generator's pause, the switch, the deletion and, in R12, the waiter.
const HOLD: Duration = Duration::from_secs(30);

/// A query app that opened a version and keeps it open for [`HOLD`]: a single F11 or a batch
/// with the fixed set, each query writing its own output file.
struct Holder {
    running: Running,
    /// The fixed query number and output file of each of its queries.
    outputs: Vec<(usize, PathBuf)>,
}

/// What one repetition of R11 or R12 saw of the generator and the holder.
struct Publication {
    output: PathBuf,
    /// The version the holder opened, the one the generator deletes.
    old: String,
    generator: Ended,
    /// Whether the old version was gone when the generator ended.
    old_gone: bool,
    holder: Holder,
    holder_ended: Ended,
}

/// R11: the generator publishes and deletes the previous version while a single F11, and in the
/// other variant a batch with the fixed set, has it open. It ends with 0 and the clean result,
/// every query gives the version it opened (KR7), and the deleted version leaves nothing.
fn r11(context: &Context) -> Vec<VariantResult> {
    [("consultor-f11", false), ("lote", true)]
        .iter()
        .map(|&(variant, batch)| {
            let id = format!("R11/{variant}");
            r11_variant(context, &id, batch).unwrap_or_else(|result| *result)
        })
        .collect()
}

fn r11_variant(
    context: &Context,
    id: &str,
    batch: bool,
) -> Result<VariantResult, Box<VariantResult>> {
    let reference = context
        .reference_dataset()
        .map_err(|error| Box::new(failed_to_run(id, &error)))?;
    let (publication, opened_on_old) = until_landed(id, |_| {
        let (output, old, mut generator) = paused_generation(context, id)?;
        let holder = start_holder(context, &output, id, batch)?;
        let opened_on_old = current_version(&output).as_deref() == Some(old.as_str());
        if switched(&generator) {
            return Ok(Landed::Missed(
                "el generador cambió de versión antes de que el consultor abriera".to_owned(),
            ));
        }
        let Some(generator) = generator.wait(GENERATION_TIMEOUT) else {
            return Err(io::Error::other("el generador no terminó"));
        };
        finish_publication(output, old, generator, holder)
            .map(|landed| map_landed(landed, |publication| (publication, opened_on_old)))
    })
    .map_err(Box::new)?;

    let mut result = VariantResult::new(id);
    observe_publication(&mut result, &publication);
    result.check_eq("codigo", Some(0), publication.holder_ended.code);
    let (different, versions) = holder_outputs(context, &publication.holder, &publication.old);
    result.check(
        "resultado",
        different.is_empty(),
        format!("distintas de la ejecución limpia: {}", join(different)),
    );
    result.check(
        "version",
        opened_on_old && versions.is_empty(),
        format!(
            "abrió la versión que CURRENT nombraba ({}): {opened_on_old}; {}",
            publication.old,
            if versions.is_empty() {
                "la salida CSV de F11 no indica versión".to_owned()
            } else {
                join(versions)
            }
        ),
    );
    check_generation(&mut result, "generacion", &publication);
    let remains = remains_of(&publication.output, &publication.old);
    result.check(
        "sin-restos",
        remains.is_empty(),
        format!("la versión borrada dejó {remains:?}"),
    );
    check_content(context, &mut result, &reference, &publication.output);
    Ok(result)
}

/// R12: a batch has a version open and a query F2 waits; the generator publishes another version
/// and deletes the awaited one before the batch ends. The batch, as in R11. The waiter warns once
/// and, as soon as its version is replaced, opens the new one without waiting for the batch and
/// answers with 0, the clean result and the new version, within 30 s of its warning (KR7).
fn r12(context: &Context) -> Vec<VariantResult> {
    let id = "R12/lote-y-espera";
    vec![r12_variant(context, id).unwrap_or_else(|result| *result)]
}

/// What the waiter of R12 did.
struct Waiter {
    watched: Watched,
    /// Whether the batch was still running when the waiter ended.
    batch_running: bool,
}

fn r12_variant(context: &Context, id: &str) -> Result<VariantResult, Box<VariantResult>> {
    let prepared = context.reference_dataset().and_then(|reference| {
        context
            .clean_run(&context.fixed_args(2))
            .map(|clean| (reference, clean))
    });
    let (reference, clean) = prepared.map_err(|error| Box::new(failed_to_run(id, &error)))?;
    let (publication, waiter) = until_landed(id, |_| {
        let (output, old, mut generator) = paused_generation(context, id)?;
        let mut holder = start_holder(context, &output, id, true)?;
        if switched(&generator) {
            return Ok(Landed::Missed(
                "el generador cambió de versión antes de que el lote abriera".to_owned(),
            ));
        }
        let launch = context.query(&output, &context.fixed_args(2), &format!("{id}-espera"));
        let mut watched = Watched::new(launch.start()?);
        let deadline = Instant::now() + GENERATOR_PAUSE;
        let mut switched_first = false;
        while watched.warned.is_none() && watched.ended.is_none() && Instant::now() < deadline {
            switched_first = switched(&generator);
            watched.look();
            thread::sleep(POLL);
        }
        if switched_first {
            return Ok(Landed::Missed(
                "el generador cambió de versión antes de que el que espera avisara".to_owned(),
            ));
        }
        let deadline = Instant::now() + QUERY_TIMEOUT;
        while watched.ended.is_none() {
            if Instant::now() >= deadline {
                return Err(io::Error::other("el que espera no terminó"));
            }
            thread::sleep(POLL);
            watched.look();
        }
        let waiter = Waiter {
            watched,
            batch_running: !holder.running.has_ended(),
        };
        let Some(generator) = generator.wait(GENERATION_TIMEOUT) else {
            return Err(io::Error::other("el generador no terminó"));
        };
        finish_publication(output, old, generator, holder)
            .map(|landed| map_landed(landed, |publication| (publication, waiter)))
    })
    .map_err(Box::new)?;

    let mut result = VariantResult::new(id);
    observe_publication(&mut result, &publication);
    let output = &publication.output;
    let (different, versions) = holder_outputs(context, &publication.holder, &publication.old);
    let remains = remains_of(output, &publication.old);
    let mut batch_problems = different
        .iter()
        .map(|query| format!("{query} distinta de la ejecución limpia"))
        .chain(versions)
        .collect::<Vec<_>>();
    if publication.holder_ended.code != Some(0) {
        batch_problems.push(format!("código {:?}", publication.holder_ended.code));
    }
    if !remains.is_empty() {
        batch_problems.push(format!("la versión borrada dejó {remains:?}"));
    }
    result.check("lote", batch_problems.is_empty(), join(batch_problems));

    let stderr = waiter.watched.run.stderr();
    let stdout = waiter.watched.run.stdout();
    result.observe(format!(
        "el que espera: código {:?} {} de su aviso, con el lote en marcha: {}",
        waiter.watched.code(),
        waiter.watched.since_warning(),
        waiter.batch_running
    ));
    result.check(
        "un-aviso",
        one_warning(&stderr, output),
        format!(
            "{} avisos; se esperaba uno, el de KC2: {}",
            warning_count(&stderr),
            stderr.trim()
        ),
    );
    result.check(
        "abre-sin-esperar",
        waiter.batch_running,
        "el que espera terminó después del lote",
    );
    let new = current_version(output);
    let answered = json_version(&stdout);
    result.check(
        "version-nueva",
        answered.is_some() && answered == new && answered.as_deref() != Some(&publication.old),
        format!(
            "respondió con {answered:?}; la nueva es {new:?} y la borrada {}",
            publication.old
        ),
    );
    result.check(
        "resultado",
        waiter.watched.code() == Some(0) && same_records(&stdout, &clean.stdout),
        format!(
            "código {:?} y {:?}; la ejecución limpia da {:?}",
            waiter.watched.code(),
            shorten(stdout.trim()),
            shorten(clean.stdout.trim())
        ),
    );
    result.check(
        "a-tiempo",
        waiter.watched.waited().is_some_and(|waited| waited < WAIT),
        format!(
            "terminó {} de su aviso; se esperaba antes de 30 s",
            waiter.watched.since_warning()
        ),
    );
    check_generation(&mut result, "generacion", &publication);
    check_content(context, &mut result, &reference, output);
    Ok(result)
}

/// A fresh copy of the reference dataset and a generation with `--overwrite` into it, paused at
/// `current-tmp`: right before its switch, for [`GENERATOR_PAUSE`]. Returns the output folder,
/// the version it will replace and the paused generator.
fn paused_generation(context: &Context, id: &str) -> io::Result<(PathBuf, String, Running)> {
    let output = context.fresh_output(&copy_name(id), true)?;
    let old = current_version(&output).ok_or_else(|| no_current(&output))?;
    let mut generator = context
        .generation(&context.csv, &output, true, &format!("{id}-generador"))
        .pause(phase::CURRENT_TMP, millis(GENERATOR_PAUSE))
        .start()?;
    if !generator.wait_phase(phase::CURRENT_TMP, GENERATION_TIMEOUT) {
        return Err(io::Error::other(format!(
            "el generador no llegó al cambio de versión: {}",
            generator.stderr().trim()
        )));
    }
    Ok((output, old, generator))
}

/// Starts the holder over `dataset`, paused at `opened` for [`HOLD`], and waits until it has
/// the dataset open.
fn start_holder(context: &Context, dataset: &Path, id: &str, batch: bool) -> io::Result<Holder> {
    let dir = context.run_dir(&format!("{id}-salidas"));
    fs::create_dir_all(&dir)?;
    let numbers: Vec<usize> = if batch { (1..=11).collect() } else { vec![11] };
    let outputs: Vec<(usize, PathBuf)> = numbers
        .iter()
        .map(|&number| (number, dir.join(format!("f{number}.out"))))
        .collect();
    let launch = if batch {
        let text: String = outputs
            .iter()
            .map(|(number, file)| {
                format!(
                    "{} --output \"{}\"\n",
                    context.fixed_line(*number),
                    file.display()
                )
            })
            .collect();
        context.batch(dataset, &text, &format!("{id}-lote"))?
    } else {
        let mut args = context.fixed_args(11);
        args.push("--output".to_owned());
        args.push(outputs[0].1.display().to_string());
        context.query(dataset, &args, &format!("{id}-consultor"))
    };
    let mut running = launch.pause(phase::OPENED, millis(HOLD)).start()?;
    if !running.wait_phase(phase::OPENED, QUERY_TIMEOUT) {
        return Err(did_not_open("el consultor que tiene el conjunto", &running));
    }
    Ok(Holder { running, outputs })
}

/// After the generator ended: the deletion must have happened while the holder had the version
/// open, else the repetition does not count. Then waits for the holder.
fn finish_publication(
    output: PathBuf,
    old: String,
    generator: Ended,
    mut holder: Holder,
) -> io::Result<Landed<Publication>> {
    if holder.running.has_ended() {
        return Ok(Landed::Missed(
            "el consultor terminó antes de que el generador publicara y borrara su versión"
                .to_owned(),
        ));
    }
    let old_gone = !output.join(&old).exists();
    let holder_ended = holder
        .running
        .wait(QUERY_TIMEOUT + HOLD)
        .ok_or_else(|| io::Error::other("el consultor que tiene el conjunto no terminó"))?;
    Ok(Landed::Yes(Publication {
        output,
        old,
        generator,
        old_gone,
        holder,
        holder_ended,
    }))
}

fn map_landed<T, U>(landed: Landed<T>, map: impl FnOnce(T) -> U) -> Landed<U> {
    match landed {
        Landed::Yes(value) => Landed::Yes(map(value)),
        Landed::Missed(why) => Landed::Missed(why),
    }
}

fn observe_publication(result: &mut VariantResult, publication: &Publication) {
    result.observe(format!(
        "generador: código {:?}, borró {} con el conjunto abierto: {}; el que lo tenía abierto terminó con {:?}",
        publication.generator.code,
        publication.old,
        publication.old_gone,
        publication.holder_ended.code
    ));
}

/// `name`: the generator ended with 0, published another version and had deleted the one the
/// holder had open when it ended (parent PRD, flow A step 5: on Linux it can).
fn check_generation(result: &mut VariantResult, name: &str, publication: &Publication) {
    let new = current_version(&publication.output);
    result.check(
        name,
        publication.generator.code == Some(0)
            && new.as_ref().is_some_and(|new| *new != publication.old)
            && publication.old_gone,
        format!(
            "código {:?}, CURRENT nombra {new:?}, la anterior {} borrada: {}",
            publication.generator.code, publication.old, publication.old_gone
        ),
    );
}

/// The holder's queries whose output is not the clean run's, and the JSON outputs that do not
/// name `version`.
fn holder_outputs(context: &Context, holder: &Holder, version: &str) -> (Vec<String>, Vec<String>) {
    let mut different = Vec::new();
    let mut versions = Vec::new();
    for (number, file) in &holder.outputs {
        let same = clean_output(context, *number)
            .and_then(|clean| same_file(file, &clean))
            .unwrap_or_else(|error| {
                different.push(format!("F{number} ({error})"));
                true
            });
        if !same {
            different.push(format!("F{number}"));
        }
        if context.fixed_line(*number).contains("--format json") {
            let named = fs::read_to_string(file)
                .ok()
                .and_then(|text| json_version(&text));
            if named.as_deref() != Some(version) {
                versions.push(format!("F{number} dio la versión {named:?}, no {version}"));
            }
        }
    }
    (different, versions)
}

/// The output file of a clean run of the fixed query `number` over the reference dataset, made
/// once.
fn clean_output(context: &Context, number: usize) -> io::Result<PathBuf> {
    let file = context
        .work
        .join("clean-outputs")
        .join(format!("f{number}.out"));
    if file.is_file() {
        return Ok(file);
    }
    fs::create_dir_all(file.parent().expect("a file has a folder"))?;
    let reference = context.reference_dataset()?;
    let mut args = context.fixed_args(number);
    args.push("--output".to_owned());
    args.push(file.display().to_string());
    let (code, _, stderr) = run_to_end(context.query(&reference, &args, "clean"), QUERY_TIMEOUT)?;
    if code != Some(0) {
        return Err(io::Error::other(format!(
            "la ejecución limpia de F{number} terminó con {code:?}: {}",
            stderr.trim()
        )));
    }
    Ok(file)
}

/// Whether `run` has marked the switch of version.
fn switched(run: &Running) -> bool {
    run.phases().iter().any(|marked| marked == phase::SWITCHED)
}

/// The entries of `output` that come from the version `old`.
fn remains_of(output: &Path, old: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(output)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| name.contains(old))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// ---------------------------------------------------------------------------------------------
// Shared helpers.
// ---------------------------------------------------------------------------------------------

/// A run followed by polling: when its wait warning first showed and when it ended.
struct Watched {
    run: Running,
    warned: Option<Instant>,
    ended: Option<Ended>,
}

impl Watched {
    fn new(run: Running) -> Self {
        Self {
            run,
            warned: None,
            ended: None,
        }
    }

    /// Looks at the run once.
    fn look(&mut self) {
        let see_warning = |watched: &mut Self| {
            if watched.warned.is_none() && watched.run.stderr().contains(WARNING_START) {
                watched.warned = Some(Instant::now());
            }
        };
        see_warning(self);
        if self.ended.is_none() {
            self.ended = self.run.wait(Duration::ZERO);
            if self.ended.is_some() {
                see_warning(self);
            }
        }
    }

    fn code(&self) -> Option<i32> {
        self.ended.and_then(|ended| ended.code)
    }

    fn ended_at(&self) -> Option<Instant> {
        self.ended
            .map(|ended| self.run.started() + ended.after_start)
    }

    /// From its warning to its end.
    fn waited(&self) -> Option<Duration> {
        Some(self.ended_at()?.saturating_duration_since(self.warned?))
    }

    fn since_warning(&self) -> String {
        self.waited().map_or("sin aviso".to_owned(), |waited| {
            format!("{:.1} s", waited.as_secs_f64())
        })
    }
}

/// Looks at `runs` until all of them end; an error if one is still running after `timeout`.
fn watch(runs: &mut [Watched], timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        for run in runs.iter_mut() {
            run.look();
        }
        if runs.iter().all(|run| run.ended.is_some()) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(format!(
                "algún proceso no terminó en {} s",
                timeout.as_secs()
            )));
        }
        thread::sleep(POLL);
    }
}

/// The wait warning of KC2 for `dataset`.
fn warning_line(dataset: &Path) -> String {
    format!(
        "{WARNING_START}{}; se esperará hasta 30 s.",
        dataset.display()
    )
}

/// The rejection of KC2 for `dataset`.
fn rejection_line(dataset: &Path) -> String {
    format!(
        "El conjunto de datos en {} sigue abierto por otro proceso. Inténtalo de nuevo cuando termine.",
        dataset.display()
    )
}

/// The warnings a query app printed.
fn warning_count(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|line| line.starts_with("Aviso:"))
        .count()
}

/// Whether `stderr` holds exactly one warning, the wait warning of KC2 for `dataset`.
fn one_warning(stderr: &str, dataset: &Path) -> bool {
    let warnings: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("Aviso:"))
        .collect();
    warnings == [warning_line(dataset).as_str()]
}

/// The generator's warning when it cannot delete the previous version `dir` (parent PRD, flow A
/// step 5).
fn previous_version_kept(dir: &Path) -> String {
    format!(
        "Aviso: no se ha podido eliminar la versión anterior {}; se descartará en la próxima generación.",
        dir.display()
    )
}

fn in_rejection_window(waited: Duration) -> bool {
    (REJECTION_WINDOW.0..=REJECTION_WINDOW.1).contains(&waited)
}

/// Whether two JSON outputs hold the same query and records, whatever version they read: each
/// publication is a new version of the same content.
fn same_records(stdout: &str, clean: &str) -> bool {
    let strip = |text: &str| -> Option<Value> {
        let mut value: Value = serde_json::from_str(text).ok()?;
        let dataset = value.get_mut("dataset")?.as_object_mut()?;
        dataset.remove("version");
        dataset.remove("generated_at");
        Some(value)
    };
    matches!((strip(stdout), strip(clean)), (Some(a), Some(b)) if a == b)
}

/// Whether the intervals cover `from..to` with no gap longer than `moment`.
fn covered(
    mut intervals: Vec<(Instant, Instant)>,
    from: Instant,
    to: Instant,
    moment: Duration,
) -> bool {
    intervals.sort();
    let mut reached = from;
    for (start, end) in intervals {
        if reached >= to {
            break;
        }
        if end <= reached {
            continue;
        }
        if start > reached + moment {
            return false;
        }
        reached = end;
    }
    reached + moment >= to
}

/// Whether two files hold the same bytes.
fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
    let length = fs::metadata(a)?.len();
    if length != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (File::open(a)?, File::open(b)?);
    let mut left = vec![0; 1 << 20];
    let mut right = vec![0; 1 << 20];
    let mut remaining = length;
    while remaining > 0 {
        let chunk = remaining.min(left.len() as u64) as usize;
        a.read_exact(&mut left[..chunk])?;
        b.read_exact(&mut right[..chunk])?;
        if left[..chunk] != right[..chunk] {
            return Ok(false);
        }
        remaining -= chunk as u64;
    }
    Ok(true)
}

/// The check `contenido`: the published dataset holds the content of a clean generation, compared
/// as in AC-M3 (PRD, "Reglas comunes").
fn check_content(context: &Context, result: &mut VariantResult, reference: &Path, output: &Path) {
    let (ok, detail) = match context.compare(reference, output) {
        Ok(Ok(())) => (true, String::new()),
        Ok(Err(report)) => (false, report),
        Err(error) => (false, format!("no se pudo comparar: {error}")),
    };
    result.check("contenido", ok, detail);
}

fn did_not_open(who: &str, run: &Running) -> io::Error {
    io::Error::other(format!(
        "{who} no abrió el conjunto: {}",
        run.stderr().trim()
    ))
}

fn no_current(output: &Path) -> io::Error {
    io::Error::other(format!("la copia {} no tiene CURRENT", output.display()))
}

/// The copy folder of a variant: its id without slashes.
fn copy_name(id: &str) -> String {
    id.replace('/', "-")
}

fn millis(duration: Duration) -> u64 {
    duration.as_millis() as u64
}

fn join(items: impl IntoIterator<Item = String>) -> String {
    items.into_iter().collect::<Vec<_>>().join(", ")
}

/// The first three problems and how many there were.
fn first_few(problems: &[String]) -> String {
    let shown = join(problems.iter().take(3).cloned());
    if problems.len() > 3 {
        format!("{shown} … ({} en total)", problems.len())
    } else {
        shown
    }
}

/// A text cut to 200 characters for a detail line.
fn shorten(text: &str) -> String {
    let mut short: String = text.chars().take(200).collect();
    if short.len() < text.len() {
        short.push('…');
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_cover_a_wait_with_only_short_gaps() {
        let t = Instant::now();
        let s = Duration::from_secs;
        let moment = s(1);
        let at = |a: u64, b: u64| (t + s(a), t + s(b));
        assert!(covered(vec![at(0, 5), at(5, 10)], t, t + s(10), moment));
        assert!(covered(vec![at(6, 10), at(0, 5)], t, t + s(10), moment));
        assert!(!covered(vec![at(0, 5), at(7, 10)], t, t + s(10), moment));
        assert!(!covered(vec![at(0, 5)], t, t + s(10), moment));
        assert!(!covered(Vec::new(), t, t + s(10), moment));
        assert!(covered(vec![at(0, 20)], t + s(2), t + s(10), moment));
    }

    #[test]
    fn json_outputs_match_whatever_version_they_read() {
        let a = r#"{"query":{"q":1},"dataset":{"version":"dataset-1","generated_at":"x","schema_version":1},"results":[
{"ID":"1"}
]}"#;
        let b = r#"{"query":{"q":1},"dataset":{"version":"dataset-2","generated_at":"y","schema_version":1},"results":[
{"ID":"1"}
]}"#;
        let c = r#"{"query":{"q":1},"dataset":{"version":"dataset-2","generated_at":"y","schema_version":1},"results":[]}"#;
        assert!(same_records(a, b));
        assert!(!same_records(a, c));
        assert!(!same_records(a, "12"));
    }

    #[test]
    fn one_warning_is_exactly_the_wait_warning_of_the_dataset() {
        let dataset = Path::new("datos");
        let warning = warning_line(dataset);
        assert!(one_warning(&format!("{warning}\n"), dataset));
        assert!(!one_warning(&format!("{warning}\n{warning}\n"), dataset));
        assert!(!one_warning("", dataset));
        assert!(!one_warning(&warning_line(Path::new("otro")), dataset));
        assert_eq!(warning_count(&format!("{warning}\nAviso: otro\nnada\n")), 2);
    }

    #[test]
    fn files_are_the_same_when_their_bytes_are() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, text: &str| {
            let path = dir.path().join(name);
            fs::write(&path, text).unwrap();
            path
        };
        let a = file("a", "uno\ndos\n");
        let b = file("b", "uno\ndos\n");
        let c = file("c", "uno\ndoz\n");
        let d = file("d", "uno\n");
        assert!(same_file(&a, &b).unwrap());
        assert!(!same_file(&a, &c).unwrap());
        assert!(!same_file(&a, &d).unwrap());
        assert!(same_file(&a, &dir.path().join("missing")).is_err());
    }

    #[test]
    fn the_rejection_window_is_30_to_35_seconds() {
        assert!(!in_rejection_window(Duration::from_millis(29_900)));
        assert!(in_rejection_window(Duration::from_millis(30_100)));
        assert!(in_rejection_window(Duration::from_secs(35)));
        assert!(!in_rejection_window(Duration::from_millis(35_100)));
    }
}
