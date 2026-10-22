//! The result of each variant and of the whole run (canopy-db-robustez PRD, "Salida", KR6).
//!
//! A variant is a list of named checks plus what the scenario observed. Its state follows from
//! its checks and from the `NOT MET` associations given on the command line: a failed check
//! associated with a finding of CanopyDB does not fail the run, any other failed check does.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One named check of a variant: `name` is what an association names (`R11/lote/version=H6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// What one variant of a scenario gave in one system.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VariantResult {
    /// `<scenario>/<variant>`, for example `R1/carga/con-conjunto`.
    pub id: String,
    pub checks: Vec<Check>,
    /// What the scenario saw: exit codes, messages, times, order, repetitions.
    pub observed: Vec<String>,
    /// Why the variant did not run (the signal never landed in its phase, no disk space).
    pub not_run: Option<String>,
    /// Why the variant does not apply in this system.
    pub not_applicable: Option<String>,
}

impl VariantResult {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    /// A variant that did not run, and why.
    pub fn not_run(id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            not_run: Some(reason.into()),
            ..Self::new(id)
        }
    }

    /// A variant that does not apply in this system, and why.
    pub fn not_applicable(id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            not_applicable: Some(reason.into()),
            ..Self::new(id)
        }
    }

    /// Adds the check `name`; `detail` says what was expected and what was seen when it fails.
    pub fn check(&mut self, name: &str, ok: bool, detail: impl Into<String>) -> bool {
        self.checks.push(Check {
            name: name.to_owned(),
            ok,
            detail: detail.into(),
        });
        ok
    }

    /// Adds `expected == actual` as the check `name`.
    pub fn check_eq<T: PartialEq + std::fmt::Debug>(
        &mut self,
        name: &str,
        expected: T,
        actual: T,
    ) -> bool {
        let ok = expected == actual;
        let detail = if ok {
            String::new()
        } else {
            format!("se esperaba {expected:?}, se obtuvo {actual:?}")
        };
        self.check(name, ok, detail)
    }

    /// Adds a line to what the scenario observed.
    pub fn observe(&mut self, text: impl Into<String>) {
        self.observed.push(text.into());
    }

    fn failed(&self) -> Vec<&Check> {
        self.checks.iter().filter(|check| !check.ok).collect()
    }
}

/// Merges the repetitions of a racy scenario (PRD, "Reglas comunes"): one result per variant,
/// which fails if one repetition failed, with how many repetitions ran and failed.
pub fn merge_repetitions(repetitions: Vec<Vec<VariantResult>>) -> Vec<VariantResult> {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, Vec<VariantResult>> = BTreeMap::new();
    for result in repetitions.into_iter().flatten() {
        if !by_id.contains_key(&result.id) {
            order.push(result.id.clone());
        }
        by_id.entry(result.id.clone()).or_default().push(result);
    }
    order
        .into_iter()
        .map(|id| {
            let runs = by_id.remove(&id).unwrap_or_default();
            merge_one(id, runs)
        })
        .collect()
}

fn merge_one(id: String, runs: Vec<VariantResult>) -> VariantResult {
    let total = runs.len();
    if let Some(applicable) = runs.iter().find(|run| run.not_applicable.is_some()) {
        return applicable.clone();
    }
    let mut merged = VariantResult::new(id);
    let mut failed_runs = 0;
    let mut not_run = Vec::new();
    for (index, run) in runs.into_iter().enumerate() {
        if let Some(reason) = run.not_run {
            not_run.push(format!("repetición {}: {reason}", index + 1));
            continue;
        }
        if !run.failed().is_empty() {
            failed_runs += 1;
        }
        for check in run.checks {
            match merged
                .checks
                .iter_mut()
                .find(|known| known.name == check.name)
            {
                Some(known) if known.ok && !check.ok => *known = check,
                Some(_) => {}
                None => merged.checks.push(check),
            }
        }
        for line in run.observed {
            if !merged.observed.contains(&line) {
                merged.observed.push(line);
            }
        }
    }
    if !not_run.is_empty() {
        merged.not_run = Some(not_run.join("; "));
    }
    merged.observed.insert(
        0,
        format!("repeticiones: {total}, con fallo: {failed_runs}"),
    );
    merged
}

/// The state of a variant (PRD, "Salida").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Met,
    /// Met, but associated with findings: they may be resolved.
    MetDespiteFindings(Vec<String>),
    NotMet,
    /// Only checks associated with these findings failed.
    NotMetBy(Vec<String>),
    NotRun(String),
    NotApplicable(String),
}

impl State {
    /// The word of the state in the run's output.
    pub fn label(&self) -> String {
        match self {
            State::Met | State::MetDespiteFindings(_) => "CUMPLIDO".to_owned(),
            State::NotMet => "NO CUMPLIDO".to_owned(),
            State::NotMetBy(findings) => format!("NOT MET ({})", findings.join(", ")),
            State::NotRun(_) => "NO EJECUTADO".to_owned(),
            State::NotApplicable(_) => "NO APLICA".to_owned(),
        }
    }

    /// Whether the state makes the run end with 1.
    pub fn fails_the_run(&self) -> bool {
        matches!(self, State::NotMet | State::NotRun(_))
    }
}

/// `NOT MET` associations: `<scenario>/<variant>/<check>` → finding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Associations(pub BTreeMap<String, String>);

impl Associations {
    /// Reads `variant/check=Hn` items, separated by commas or given one by one.
    pub fn parse(items: &[String]) -> Result<Self, String> {
        let mut map = BTreeMap::new();
        for item in items.iter().flat_map(|item| item.split(',')) {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let (target, finding) = item.split_once('=').ok_or_else(|| {
                format!("La asociación «{item}» no tiene la forma <escenario>/<variante>/<comprobación>=<hallazgo>.")
            })?;
            let target = target.trim();
            let finding = finding.trim();
            if target.matches('/').count() < 2 || finding.is_empty() {
                return Err(format!(
                    "La asociación «{item}» no tiene la forma <escenario>/<variante>/<comprobación>=<hallazgo>."
                ));
            }
            if map.insert(target.to_owned(), finding.to_owned()).is_some() {
                return Err(format!(
                    "La comprobación {target} está asociada más de una vez."
                ));
            }
        }
        Ok(Self(map))
    }

    /// Checks every association against the known variants, their checks and the findings
    /// (PRD, "Salida": an association that names something that does not exist stops the run).
    pub fn validate(
        &self,
        variant_checks: &BTreeMap<String, Vec<String>>,
        findings: &[String],
    ) -> Result<(), String> {
        for (target, finding) in &self.0 {
            let (variant, check) = target.rsplit_once('/').expect("parsed with two slashes");
            let Some(checks) = variant_checks.get(variant) else {
                return Err(format!(
                    "La asociación {target}={finding} nombra una variante que no existe: {variant}."
                ));
            };
            if !checks.iter().any(|known| known == check) {
                return Err(format!(
                    "La asociación {target}={finding} nombra una comprobación que la variante {variant} no tiene: {check}. Comprobaciones: {}.",
                    checks.join(", ")
                ));
            }
            if !findings.iter().any(|known| known == finding) {
                return Err(format!(
                    "La asociación {target}={finding} nombra un hallazgo que no está en el registro de hallazgos: {finding}."
                ));
            }
        }
        Ok(())
    }

    /// The finding associated with the check `check` of `variant`, if any.
    fn finding(&self, variant: &str, check: &str) -> Option<&String> {
        self.0.get(&format!("{variant}/{check}"))
    }

    /// The findings associated with any check of `variant`.
    fn findings_of(&self, variant: &str) -> Vec<String> {
        let prefix = format!("{variant}/");
        let mut findings: Vec<String> = self
            .0
            .iter()
            .filter(|(target, _)| {
                target
                    .strip_prefix(&prefix)
                    .is_some_and(|check| !check.contains('/'))
            })
            .map(|(_, finding)| finding.clone())
            .collect();
        findings.sort();
        findings.dedup();
        findings
    }
}

/// The state of `result` under `associations`.
pub fn state_of(result: &VariantResult, associations: &Associations) -> State {
    if let Some(reason) = &result.not_applicable {
        return State::NotApplicable(reason.clone());
    }
    if let Some(reason) = &result.not_run {
        return State::NotRun(reason.clone());
    }
    let failed = result.failed();
    if failed.is_empty() {
        let findings = associations.findings_of(&result.id);
        return if findings.is_empty() {
            State::Met
        } else {
            State::MetDespiteFindings(findings)
        };
    }
    let mut findings = Vec::new();
    for check in failed {
        match associations.finding(&result.id, &check.name) {
            Some(finding) => findings.push(finding.clone()),
            None => return State::NotMet,
        }
    }
    findings.sort();
    findings.dedup();
    State::NotMetBy(findings)
}

/// The line of a variant in the run's output.
pub fn line(result: &VariantResult, state: &State) -> String {
    let mut text = format!("{:<14} {}", state.label(), result.id);
    let details = details(result, state);
    if !details.is_empty() {
        let _ = write!(text, "  {}", details.join("; "));
    }
    text
}

/// What a variant's line says after its state and id: why it did not run or apply, the notice of
/// a finding that may be resolved, its failed checks and what it observed.
fn details(result: &VariantResult, state: &State) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    match state {
        State::NotRun(reason) | State::NotApplicable(reason) => parts.push(reason.clone()),
        State::MetDespiteFindings(findings) => parts.push(format!(
            "cumplido aunque está asociado a {}: el hallazgo puede estar resuelto",
            findings.join(", ")
        )),
        _ => {}
    }
    for check in result.checks.iter().filter(|check| !check.ok) {
        parts.push(format!("falla {}: {}", check.name, check.detail));
    }
    parts.extend(result.observed.iter().cloned());
    parts
}

/// The last line of the run and its exit code.
pub fn summary(states: &[State]) -> (String, u8) {
    let count = |wanted: fn(&State) -> bool| states.iter().filter(|state| wanted(state)).count();
    let met = count(|state| matches!(state, State::Met | State::MetDespiteFindings(_)));
    let not_met = count(|state| matches!(state, State::NotMet));
    let by_findings = count(|state| matches!(state, State::NotMetBy(_)));
    let not_run = count(|state| matches!(state, State::NotRun(_)));
    let not_applicable = count(|state| matches!(state, State::NotApplicable(_)));
    let applicable = states.len() - not_applicable;
    let code = u8::from(states.iter().any(State::fails_the_run));
    let verdict = if code == 0 { "PASS" } else { "FAIL" };
    (
        format!(
            "{verdict}: {met} de {applicable} variantes cumplidas, {not_met} no cumplidas, {by_findings} NOT MET por hallazgos de CanopyDB, {not_run} no ejecutadas, {not_applicable} no aplicables en este sistema."
        ),
        code,
    )
}

/// The row of a variant for the scenario log (PRD, "Registro de escenarios"), or `None` for a
/// variant that did not run: it is not logged, it is run again.
pub fn log_row(
    result: &VariantResult,
    state: &State,
    system: &str,
    date: &str,
    canopydb: &str,
    commit: &str,
) -> Option<String> {
    let outcome = match state {
        State::Met | State::MetDespiteFindings(_) => "`Sin bug`".to_owned(),
        State::NotMetBy(findings) => format!("`Hallazgo` {}", findings.join(", ")),
        State::NotMet => "`Bug del proyecto` o `Hallazgo`: por clasificar".to_owned(),
        State::NotApplicable(_) => "`No aplica en este sistema`".to_owned(),
        State::NotRun(_) => return None,
    };
    let (scenario, variant) = result
        .id
        .split_once('/')
        .unwrap_or((result.id.as_str(), "—"));
    let details = details(result, state);
    let observed = if details.is_empty() {
        "—".to_owned()
    } else {
        details.join("; ").replace('|', "\\|")
    };
    Some(format!(
        "| {scenario} {variant} | `{system}` | {date} | {canopydb} | {commit} | {outcome} | {observed} |"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: &str, checks: &[(&str, bool)]) -> VariantResult {
        let mut result = VariantResult::new(id);
        for (name, ok) in checks {
            result.check(name, *ok, if *ok { "" } else { "mal" });
        }
        result
    }

    fn associations(items: &[&str]) -> Associations {
        Associations::parse(
            &items
                .iter()
                .map(|item| (*item).to_owned())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    #[test]
    fn a_variant_whose_checks_pass_is_met() {
        let state = state_of(
            &result("R3/f3", &[("codigo", true)]),
            &Associations::default(),
        );
        assert_eq!(state, State::Met);
    }

    #[test]
    fn only_the_associated_check_failing_is_not_met_by_its_finding() {
        let failed = result("R11/lote", &[("codigo", true), ("version", false)]);
        let state = state_of(&failed, &associations(&["R11/lote/version=H6"]));
        assert_eq!(state, State::NotMetBy(vec!["H6".to_owned()]));
        assert!(!state.fails_the_run());
    }

    #[test]
    fn another_failing_check_is_not_absorbed_by_an_association() {
        let failed = result("R11/lote", &[("codigo", false), ("version", false)]);
        let state = state_of(&failed, &associations(&["R11/lote/version=H6"]));
        assert_eq!(state, State::NotMet);
        assert!(state.fails_the_run());
    }

    #[test]
    fn an_associated_variant_that_passes_says_its_finding_may_be_resolved() {
        let passed = result("R11/lote", &[("version", true)]);
        let state = state_of(&passed, &associations(&["R11/lote/version=H6"]));
        assert_eq!(state, State::MetDespiteFindings(vec!["H6".to_owned()]));
        assert!(line(&passed, &state).contains("puede estar resuelto"));
    }

    #[test]
    fn an_association_must_name_a_known_variant_check_and_finding() {
        let mut known = BTreeMap::new();
        known.insert("R11/lote".to_owned(), vec!["version".to_owned()]);
        let findings = vec!["H6".to_owned()];
        assert!(
            associations(&["R11/lote/version=H6"])
                .validate(&known, &findings)
                .is_ok()
        );
        assert!(
            associations(&["R99/x/version=H6"])
                .validate(&known, &findings)
                .is_err()
        );
        assert!(
            associations(&["R11/lote/codigo=H6"])
                .validate(&known, &findings)
                .is_err()
        );
        assert!(
            associations(&["R11/lote/version=H9"])
                .validate(&known, &findings)
                .is_err()
        );
        assert!(Associations::parse(&["R11=H6".to_owned()]).is_err());
    }

    #[test]
    fn not_met_and_not_run_fail_the_run_and_the_others_do_not() {
        let (_, code) = summary(&[State::Met, State::NotMetBy(vec!["H6".into()])]);
        assert_eq!(code, 0);
        let (_, code) = summary(&[State::Met, State::NotApplicable("Linux".into())]);
        assert_eq!(code, 0);
        let (_, code) = summary(&[State::Met, State::NotRun("sin espacio".into())]);
        assert_eq!(code, 1);
        let (line, code) = summary(&[State::NotMet]);
        assert_eq!(code, 1);
        assert!(line.starts_with("FAIL: 0 de 1"));
    }

    #[test]
    fn a_repetition_that_fails_fails_the_merged_variant() {
        let merged = merge_repetitions(vec![
            vec![result("R8/a", &[("codigo", true)])],
            vec![result("R8/a", &[("codigo", false)])],
            vec![result("R8/a", &[("codigo", true)])],
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(
            state_of(&merged[0], &Associations::default()),
            State::NotMet
        );
        assert_eq!(merged[0].observed[0], "repeticiones: 3, con fallo: 1");
    }

    #[test]
    fn the_observed_field_holds_only_what_the_variant_saw() {
        let mut met = result("R1/dump/con", &[("codigo", true)]);
        met.observe("exit 0 en 1.2 s");
        let row = log_row(&met, &State::Met, "Windows", "2026-10-22", "0.2.5", "abc").unwrap();
        assert_eq!(
            row,
            "| R1 dump/con | `Windows` | 2026-10-22 | 0.2.5 | abc | `Sin bug` | exit 0 en 1.2 s |"
        );
        let empty = result("R2/x", &[("codigo", true)]);
        let row = log_row(&empty, &State::Met, "Linux", "2026-10-22", "0.2.5", "abc").unwrap();
        assert!(row.ends_with("| `Sin bug` | — |"), "{row}");
    }

    #[test]
    fn a_variant_that_did_not_run_has_no_log_row() {
        let not_run = VariantResult::not_run("R1/dump/con", "la señal no llegó en su fase");
        let state = state_of(&not_run, &Associations::default());
        assert_eq!(
            log_row(&not_run, &state, "Windows", "2026-10-22", "0.2.5", "abc"),
            None
        );
        let met = result("R1/dump/con", &[("codigo", true)]);
        let row = log_row(&met, &State::Met, "Windows", "2026-10-22", "0.2.5", "abc").unwrap();
        assert!(
            row.starts_with("| R1 dump/con | `Windows` | 2026-10-22 | 0.2.5 | abc | `Sin bug` |")
        );
    }
}
