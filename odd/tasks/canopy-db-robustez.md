# canopy-db-robustez — ODD feature document

- **Objective**: implement `docs/prd/chicago-crime/canopy-db-robustez.prd.md` v1.0 (scenarios R1–R13, their
  runner, the big CSV tool, the scenario log, the issue drafts, KR7 in the CanopyDB query app and this child's
  part of the module closing; AC-KR1–AC-KR8, AC-M11–AC-M13). Branch `feat/canopy-db-robustez` (worktree copy)
  from `3d07096`. Delivery: local work-unit commits, no PRs, merge into the base only when the user says so.
  Runner: `cargo test --workspace` plus the scenario runner.
- **Timing**: estimate **4 h – 7 h**. Real start **2026-10-22 11:46:54 +0100**. Phase log
  `odd/tasks/canopy-db-robustez-phases.tsv`.

## Specs

The PRD wins on any product question; read it until its "Referencias". Every rule of its ancestry holds. The
`S#` below name the PRD parts each task covers; scenario rows are quoted from the PRD by id, never reworded.

- **S1 — Scenarios R1–R13** (PRD "Escenarios": "Reglas comunes", each row's "Escenario y variantes" and "Lo
  esperado", "CSV grande", "Sistemas"). Kill = `TerminateProcess` / `SIGKILL`; Ctrl+C = the signal the app
  honors; a signal counts only if it lands in the variant's phase, up to five attempts, else "no ejecutado";
  racy scenarios (R8, R9, R11, R12) at least five repetitions per variant, one failure fails it.
- **S2 — Runner output (KR2, KR6, PRD "Salida")**: per variant its id, state and what it observed; states
  cumplido / no cumplido / `NOT MET` with its finding / no ejecutado with the reason / no aplicable en este
  sistema. `NOT MET` only when what fails is only what the associated finding describes; an association naming
  a scenario, variant or finding that does not exist: nothing runs, code 1. Code 1 if any variant is not met or
  not run, else 0. Associated variant that passes: cumplido plus a notice that its finding may be resolved.
- **S3 — KR7**: "un consultor CanopyDB que ya abrió una versión termina con los resultados de esa versión aunque
  el generador la borre"; "Uno que aún no la ha abierto, esté esperando o no, vuelve a leer el puntero en cuanto
  la versión desaparece y responde con la nueva, sin un segundo aviso y sin pasar de los 30 s de KC2 desde su
  aviso." "Si el puntero no ha cambiado y la versión está incompleta, sigue valiendo el rechazo de AC-KC4."
- **S4 — Big CSV tool (KR3, "CSV grande", AC-KR5)**: header once, the original then two copies; each copy gets
  new numeric IDs greater than any original ID and new case numbers with the same two initial letters and
  format, unique; other fields unchanged; it says how many records it wrote. R13 checks free space first.
- **S5 — Scenario log (PRD "Registro de escenarios", AC-KR6)**: Spanish document next to the findings log, rows
  appended per run, one row per scenario, variant and system, with its seven fields and values.
- **S6 — Issue drafts (KR8, AC-KR7)**: an English draft per finding H1–H5 and any new one (title, description,
  minimal reproduction, version), linked from a new `Borrador del issue` field of each entry; nothing published.
- **S7 — Closing (PRD "Cierre del módulo" steps 1, 2 and 5; AC-KR8)**: AC-M11–AC-M13; annotations of the parent
  (KR5 in "Reparto en PRD hijos" and in this child's row; `Borrador del issue` in "Registro de hallazgos") and of
  the consultor PRD (KR7 in its flow; its "Fuera de alcance" items now here), Spanish and English; README and
  Linux guides say how to run the scenarios, build the big CSV and where the scenario log is. Steps 3 and 4
  only if `canopy-db-comparacion` is already closed (it is not: no PRD).
- **S8 — No app contract change**: no option, message or output of any app changes ("Validaciones y mensajes").
  The phase hooks the runner needs are hidden (environment variables, not in any help) and change nothing when
  unset ("Queda a criterio": "sin añadir ninguna opción visible a las aplicaciones ni cambiar una ejecución
  normal").

## Tasks

| Id | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S3, S8 | inline | done (RED: partial version → `NotFound`, replaced version waited 10 s; GREEN 4/4 + markers) | `fbc7bc7`, `01e8e41` |
| T2 | S1 (framework, R5 as model), S2 | inline | in progress | — |
| T3 | S1 (R1, R2, R10) | delegated: parallel with T4a, T4b | pending | — |
| T4a | S1 (R3, R4, R6, R7) | delegated: parallel with T3, T4b | pending | — |
| T4b | S1 (R8, R9, R11, R12) | delegated: parallel with T3, T4a | pending | — |
| T5 | S4 | delegated: parallel with T6 (launched while T1 was reviewed) | done (RED 0/14, GREEN 22/22) | see L8 |
| T6 | S6 | delegated: parallel with T5 | done | `804c84b` |
| T7 | S1 (R13) | inline, after T2 and T5 | pending | — |
| T8 | S1, S5 | runs on Windows and Linux; scenario log | pending | — |
| T9 | S7 | inline | pending | — |

Route evidence: T3, T4a and T4b are independent units of behavior (one scenario file each) with disjoint edit
surfaces and several minutes of work each, launched together once T2 fixes their shared framework; T5 and T6 are
independent of everything else and were launched together earlier (Writer trigger (a)). T1, T2, T7 and T9 are the
shared contract or depend on the others.

## Log

- **L1** (2026-10-22, user): "escribe el PRD para el punto 5 canopy-db-robustez" — PRD written and agreed in
  this session; then "haz el commit e implementa el cambio." Estimate 4 h – 7 h confirmed with "Sí".
- **L2** (exploration): release binaries have no phase hooks (fail points are a dev feature); the generator logs
  no line around the final CanopyDB checkpoint, the manifest or `CURRENT.tmp`. No Rust code sends a real Ctrl+C;
  on Windows only `scripts/acceptance/send-ctrl-c.ps1` does (`GenerateConsoleCtrlEvent` in its own console). The
  query app's open path turns a partly deleted version (folder still there) into `not_found` or an engine error
  instead of retrying: the gap of KR7.
- **L3** (decision): phase hooks through two hidden environment variables read by `crime-dataset`: one names a
  file where each app appends the phase it enters, the other pauses at one phase for some milliseconds, so a
  signal can land in a short phase. Unset, they do nothing. The runner is a new Rust binary
  (`crime-canopy-scenarios`), so it runs the same on Windows and Linux; Ctrl+C on Windows goes through a hidden
  console of its own, on Linux through `SIGINT`.
- **L4** (decision): "durante la apertura de la base" (R3) is the pause right after the query app opens the
  CanopyDB environment, so the lock is held when it dies.
- **L5** (RDD, T1): commits `3d07096..fbc7bc7`, lenses resilience and reliability (item 4 escalated: the query
  app's open races a generator). Result: approved and acknowledged. Findings: R3-001 WARNING (reliability), the
  query app's `opened` and `query` markers had no test: fixed in `01e8e41`. Lineage `review-f6bb71545eaced32`.
- **L6** (T6): five English issue drafts in `docs/canopy-db/issues/`, field `Borrador del issue` in every entry of
  the findings log; the H2, H3 and H5 snippets compiled and reproduced against canopydb 0.2.5 (H5 took 55.1 s
  with other writers running). Nothing published.
- **L7** (decision, T2): the runner is `crates/crime-canopy-scenarios`; states print as `CUMPLIDO`, `NO CUMPLIDO`,
  `NOT MET (Hn)`, `NO EJECUTADO`, `NO APLICA`; the last line starts with `PASS:` or `FAIL:`; associations are
  `--not-met <escenario>/<variante>/<comprobación>=<hallazgo>`, so `NOT MET` absorbs only the named check (KR6);
  each run writes the scenario-log rows to `<work>/registro-<Sistema>-<fecha>.md`. The big CSV tool is only
  required when R13 runs.
- **L8** (default taken during the run, to list in the closing report): S4's "un número de caso nuevo con las dos
  mismas letras iniciales y el mismo formato" cannot hold on the reference CSV: prefixes HH, HJ, HK, HL and HM
  already use 447,126–485,389 of their 1,000,000 two-letters-plus-six-digits numbers, so two copies do not fit,
  and 480,094 case numbers have another shape (479,997 are one letter plus six digits). Default: the copy keeps the
  first two characters and takes the lowest free value of that space; when it is full it widens to 7, 8 and 9
  digits (`HH0000000`). Uniqueness always holds. F2 (exact case number) is unchanged and F9 (`JA`, which does not
  overflow) still triples, so R13's expectations hold. The tool stops with code 1 if a prefix runs out at 9 digits.
