# startswith — ODD feature document

- **Objective**: implement `docs/prd/chicago-crime/startswith.prd.md` v1.0 (prefix conditions in the query app,
  AC-S1…AC-S12). Branch `feat/startswith-odd` from `0971a37`. Delivery: local work-unit commits, no PRs, merge
  into the base only when the user says so (worktree rule). Runner: `cargo test --workspace` (baseline 425 green).
- **Timing**: estimate **1–2 h**. Real start **2026-10-18 09:03:19 +0100**. Phase log `odd/tasks/startswith-phases.tsv`.

## Specs

The PRD wins on any product question; the parent rules C1–C11 still hold.

- **S1 — Option.** "`--startswith CAMPO=PREFIJO`", repeatable, "solas o junto a las condiciones que ya existen".
  "La ayuda del consultor la describe junto a las demás condiciones."
- **S2 — Comparison.** "El consultor compara sin distinguir mayúsculas, igual que la igualdad (C1): `crim`, `Crim`
  y `CRIM` son equivalentes. El prefijo se toma tal cual, espacios incluidos; los códigos no se completan con ceros
  (`8` no es prefijo de `008`, `00` sí)."
- **S3 — OR/AND (P1).** "Todos los valores y prefijos pedidos sobre un mismo campo son alternativas (O lógico)":
  `--where "Primary Type=THEFT" --startswith "Primary Type=CRIM"` "devuelve THEFT y todos los tipos que empiezan
  por CRIM. Sobre campos distintos se cumplen todas (Y lógico)." "sin valor" pedido con la igualdad es una
  alternativa más.
- **S4 — Indexed or filter (P3).** "Sobre número de caso, tipo principal o distrito, la condición es
  **indexada**: cuenta como la condición indexada que exige C10 y puede ir sola. Sobre IUCR, Location
  Description, Beat o FBI Code es un **filtro sin índice** y necesita, como cualquier filtro, al menos una
  condición indexada."
- **S5 — Empty field.** "Un registro "sin valor" en el campo nunca empieza por ningún prefijo."
- **S6 — Output.** "La salida no cambia … un mismo registro no sale dos veces aunque cumpla varios prefijos
  alternativos. Las estadísticas indican el índice usado o el filtro aplicado como hoy. La salida JSON incluye la
  condición tal como se aplicó, con el prefijo en minúsculas (C9)."
- **S7 — Memory (C7).** "La memoria no crece con el número de resultados ni con el número de valores distintos
  que abarca el prefijo": `H` in case number "usa una memoria comparable a la de un prefijo que abarque uno solo".
- **S8 — Validations**, verbatim:
  empty prefix → `El prefijo de "<campo>" no puede estar vacío.` ·
  non-text field → `"Empieza por" solo se admite en campos de texto: Case Number, Primary Type, District, IUCR, Location Description, Beat y FBI Code.` ·
  Block/Description → `El campo "<nombre>" no admite condiciones. Campos disponibles: ...` ·
  unknown field → `El campo "<nombre>" no existe. Campos disponibles: ...` ·
  no `=` (exit code 2) → `La condición "<texto>" no tiene la forma CAMPO=PREFIJO.` ·
  only prefixes on non-indexed fields → the C10 message ·
  no match → empty result, no error, no warning (P4).
- **S9 — Independent selector.** "Se amplía con la misma condición para comprobar los criterios contra el CSV
  sin usar código del consultor."
- **S10 — Acceptance.** AC-S1…AC-S12 of the PRD, figures of the reference CSV (CRIM 1.256.646; JA 269.655;
  THEFT+CRIM 3.095.148; H 5.762.199), with AC-S12: the consultor criteria repeated give the same results.
- **Out of scope**: generator changes, new indexes, schema change, endswith/contains/wildcards, negation, a time
  target, a minimum length.

### Defaults taken (no open product question; PRD says none is left)

- D1: JSON condition object gains `"startswith": [<lowercase prefixes>]`; `"values"` stays when the field also
  has equality alternatives and is omitted otherwise. Conditions without prefixes keep today's shape.
- D2: Check order per `--startswith`: unknown field → not conditionable → not text → empty prefix. Empty prefix
  is exit code 1 (query data, C6); only the missing `=` is exit code 2 (PRD row).
- D3: Fixed plan (no cost model, C10): the date intervals drive when present; otherwise the first indexed field
  with a prefix drives by a scan of its disjoint key ranges (every alternative of that field, merged); otherwise
  today's sorted merge. Other equalities are checked per ID by point lookup as today; other indexed prefix fields
  are checked on the record. Stats list indexed prefix fields under "Índices usados" (S4/S6).
- D4: The case-number warning still applies to equality alternatives only (P4).
- D5: AC-S1…AC-S11 are added as a new section of `scripts/acceptance/run-query-acceptance.ps1` (reusing its
  dataset and helpers); one run of it gives AC-C1…AC-C18 (= AC-S12) and AC-S1…AC-S11.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1–S6, S8 | inline: CLI, query model, filters, JSON, help | done (RED compile → GREEN) | `55e0278` |
| T2 | S3, S4, S6, S7 | inline: prefix index streams and plan | done (5 RED → GREEN) | `abcb963` |
| T3 | S9 | delegated writer A (parallel with T1–T2: disjoint crate) | done | `f159556` |
| T4 | S10 | delegated writer A after T3 (acceptance script section) | done (parse check; run in T6) | `1a34a39` |
| T5 | S1 | inline: README | done | `c190bef` |
| T6 | S10 | acceptance run on the reference CSV, closing report | done: 91/91 PASS on `c190bef` | this report commit |

## Log

- **L1** (2026-10-18 09:03, user): "Implementa docs\prd\chicago-crime\startswith.prd.md usando ODD"
- **L2**: estimate 1–2 h sent at 09:03:02; user confirmed ("Sí"); start 09:03:19 +0100. RDD on (global).
- **L3**: route — T3+T4 to one writer (the acceptance side, `crates/crime-acceptance` + the script) launched
  together with T1+T2 inline (`crates/crime-query`): disjoint edit surfaces, each several minutes of work.
- **L4**: D1 revised before any code used it: `"values"` is always present (empty with only prefixes), so
  every condition keeps today's shape; `"startswith"` is added only with prefixes. Writer told.
- **L5**: RDD assess per commit: `55e0278` medium/under_budget. After T5 the slice `0971a37..c190bef` was
  medium/under_budget (15 paths, 1512 lines); per the project closing rule its review was requested
  (lenses resilience, readability, reliability; lineage `review-de594d3b5bfc6638`, consent granted per the
  project rule).
- **L6**: Writer A returned: selector `--startswith` with 8 tests, the AC-S1…AC-S12 section in
  `run-query-acceptance.ps1` (`-Criterion` parameter on three helpers, AC-S12 = every AC-C row passed),
  README selector paragraph. Its own selector run on the reference CSV gave CRIM 1256646, JA 269655,
  THEFT+CRIM 3095148, H 5762199, District=8 0. Parent spot check: `cargo test -p crime-acceptance` green.
- **L7**: Checks after T5: `cargo fmt --all --check` clean, clippy clean, `cargo test --workspace` 453 passed
  (baseline 425).
- **L8**: Incident: the first acceptance run was started from bash with `-WorkDir work\\query-acceptance`,
  which PowerShell received as `workquery-acceptance` at the copy's root. Stopped after a few minutes and
  rerun with `-WorkDir work/query-acceptance`. Deleting the stray folder (787 MB, untracked) was denied;
  it is left for the user. RDD reviews exclude untracked files.
- **L9**: RDD review 1 — commits `55e0278..c190bef` (base `0971a37`), lenses resilience, readability,
  reliability; lineage `review-de594d3b5bfc6638`; result **approved**, acknowledged (authority burned).
  Findings, all non-blocking: R2-001 WARNING `exec.rs:152-153` `on_record` mutated by `remove(0)` to pick
  the driver; R2-002 SUGGESTION `exec.rs:371-373` `disjoint_runs` relies on distinct prefixes unstated;
  R2-003 SUGGESTION `query.rs:130-142` `Equality::matches` re-implements `Value::accepts`. Resilience and
  reliability: no findings. Resolution: all three fixed in `d6e3ae6` (crime-query 174 tests green, clippy
  clean).
- **L10**: RDD review 2 — last slice `d6e3ae6` (base `c190bef`), lenses readability, reliability; lineage
  `review-f5c0d1972beb4cb1`; result **approved**, acknowledged (authority burned). Findings, non-blocking:
  R2-001 SUGGESTION `query.rs:210-217` `Value::from_index` allocates a String per value per record on the
  record-check path; R3-001 SUGGESTION `exec.rs:324-351` `disjoint_runs` relies on distinct prefixes (now
  documented; `Builder` deduplicates). Resolution: left as follow-ups, no defect.
- **L11**: Acceptance rerun 09:18:33–09:32:47 on the binaries of `c190bef`: **91/91 PASS, 30 criteria**
  (AC-C1…AC-C18, AC-S1…AC-S12). AC-S8: H 15.2 MiB vs JK438266 7.5 MiB (ratio 2.02). AC-S3 took 23.4 s.
  Final checks on `d6e3ae6`: fmt and clippy clean, 453 tests green. Phase log ended 09:33:55.
- **Closing**: report [startswith-report.md](startswith-report.md). Not merged (worktree rule); nothing pushed.
