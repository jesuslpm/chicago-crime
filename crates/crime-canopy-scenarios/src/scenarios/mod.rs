//! The catalog of scenarios R1–R13 (canopy-db-robustez PRD, "Escenarios").
//!
//! Each scenario declares its variants and the checks of each one up front, so that a `NOT MET`
//! association can be checked before anything runs; its `run` function returns one
//! [`VariantResult`] per variant, whose ids and check names must be the declared ones.

mod big;
mod concurrency;
mod generation;
mod queries;

use std::collections::BTreeMap;
use std::io;

use crate::context::Context;
use crate::report::VariantResult;

/// The systems a scenario runs in (PRD, "Sistemas").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Systems {
    Both,
    /// R11 and R12: only Linux lets a generator delete a version a query app has open.
    LinuxOnly,
    /// R13 (decision KR3).
    WindowsOnly,
}

impl Systems {
    /// Why the scenario does not apply in this system, if it does not.
    pub fn not_here(self) -> Option<&'static str> {
        match self {
            Systems::LinuxOnly if cfg!(windows) => {
                Some("solo en Linux: en Windows el generador no puede borrar una versión abierta")
            }
            Systems::WindowsOnly if !cfg!(windows) => {
                Some("solo en Windows: los datos grandes no se repiten en Linux (decisión KR3)")
            }
            _ => None,
        }
    }
}

/// One scenario of the catalog.
pub struct Scenario {
    pub id: &'static str,
    pub systems: Systems,
    /// Whether it has races, so it runs [`Context::repetitions`] times (PRD, "Reglas comunes").
    pub racy: bool,
    /// Each variant (without the scenario id) and the names of its checks.
    pub variants: Vec<(&'static str, Vec<&'static str>)>,
    pub run: fn(&Context) -> Vec<VariantResult>,
}

impl Scenario {
    /// The full id of `variant`: `<scenario>/<variant>`.
    pub fn variant_id(&self, variant: &str) -> String {
        format!("{}/{variant}", self.id)
    }
}

/// Every scenario, in the PRD's order.
pub fn catalog() -> Vec<Scenario> {
    let mut all = Vec::new();
    all.extend(generation::scenarios());
    all.extend(queries::scenarios());
    all.extend(concurrency::scenarios());
    all.extend(big::scenarios());
    all.sort_by_key(|scenario| scenario.id[1..].parse::<u32>().unwrap_or(u32::MAX));
    all
}

/// The checks of every variant of `scenarios`, by full variant id.
pub fn variant_checks(scenarios: &[Scenario]) -> BTreeMap<String, Vec<String>> {
    scenarios
        .iter()
        .flat_map(|scenario| {
            scenario.variants.iter().map(|(variant, checks)| {
                (
                    scenario.variant_id(variant),
                    checks.iter().map(|check| (*check).to_owned()).collect(),
                )
            })
        })
        .collect()
}

/// How many times a signal may miss its phase before the variant does not run (PRD, "Reglas
/// comunes").
pub const SIGNAL_ATTEMPTS: usize = 5;

/// Whether one attempt landed its signal in the variant's phase.
pub enum Landed<T> {
    Yes(T),
    /// It missed, and why (the phase it landed in, or that the app had ended).
    Missed(String),
}

/// Runs `attempt` until its signal lands in its phase, up to [`SIGNAL_ATTEMPTS`] times; the
/// variant `id` does not run when none lands or the runner itself fails.
pub fn until_landed<T>(
    id: &str,
    mut attempt: impl FnMut(usize) -> io::Result<Landed<T>>,
) -> Result<T, VariantResult> {
    let mut last = String::new();
    for number in 1..=SIGNAL_ATTEMPTS {
        match attempt(number) {
            Ok(Landed::Yes(value)) => return Ok(value),
            Ok(Landed::Missed(why)) => last = why,
            Err(error) => return Err(failed_to_run(id, &error)),
        }
    }
    Err(VariantResult::not_run(
        id,
        format!("la señal no llegó en su fase tras {SIGNAL_ATTEMPTS} intentos ({last})"),
    ))
}

/// The check every interrupted query scenario shares (R3–R7): a query launched after the
/// interrupted one opens `dataset` without the wait warning of KC2 and gives the clean result of
/// the count F3. Adds the checks `siguiente-sin-espera` and `siguiente-resultado`.
pub fn next_query_opens(
    context: &Context,
    result: &mut VariantResult,
    dataset: &std::path::Path,
) -> io::Result<()> {
    let args = context.fixed_args(3);
    let clean = context.clean_run(&args)?;
    let (code, stdout, stderr) = crate::process::run_to_end(
        context.query(dataset, &args, "next"),
        crate::context::QUERY_TIMEOUT,
    )?;
    result.check(
        "siguiente-sin-espera",
        !stderr.contains("Aviso:"),
        format!("el siguiente consultor esperó: {}", stderr.trim()),
    );
    result.check(
        "siguiente-resultado",
        code == Some(0) && stdout == clean.stdout,
        format!(
            "el siguiente consultor dio el código {code:?} y {:?}, no {:?}",
            stdout.trim(),
            clean.stdout.trim()
        ),
    );
    Ok(())
}

/// The temporary files the query apps leave next to the output file `name` in `dir`
/// (`.<name>.<random>.tmp`).
pub fn output_temporaries(dir: &std::path::Path, name: &str) -> Vec<String> {
    crate::context::entries_starting_with(dir, &format!(".{name}."))
}

/// A variant that could not run because the runner itself failed (a folder that could not be
/// copied, a binary that did not start): it is reported as not run, with the error.
pub fn failed_to_run(id: &str, error: &io::Error) -> VariantResult {
    VariantResult::not_run(id, format!("el ejecutor falló: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_ids_are_unique_and_every_variant_has_checks() {
        let scenarios = catalog();
        let mut ids: Vec<&str> = scenarios.iter().map(|scenario| scenario.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), scenarios.len());
        for scenario in &scenarios {
            assert!(
                !scenario.variants.is_empty(),
                "{} has no variants",
                scenario.id
            );
            for (variant, checks) in &scenario.variants {
                assert!(
                    !checks.is_empty(),
                    "{}/{variant} has no checks",
                    scenario.id
                );
            }
        }
    }
}
