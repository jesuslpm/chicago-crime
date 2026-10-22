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
| T1 | S3, S8 | inline | pending | — |
| T2 | S1 (framework), S2 | inline | pending | — |
| T3 | S1 (R1, R2, R10) | delegated: parallel with T4, T5, T6 | pending | — |
| T4 | S1 (R3–R9, R11, R12) | delegated: parallel with T3, T5, T6 | pending | — |
| T5 | S4 | delegated: parallel with T3, T4, T6 | pending | — |
| T6 | S6 | delegated: parallel with T3, T4, T5 | pending | — |
| T7 | S1 (R13) | inline, after T2 and T5 | pending | — |
| T8 | S1, S5 | runs on Windows and Linux; scenario log | pending | — |
| T9 | S7 | inline | pending | — |

Route evidence: T3–T6 are independent units of behavior with disjoint edit surfaces, each several minutes of
work, launched together (Writer trigger (a)); T1, T2, T7 and T9 are the shared contract or depend on the others.

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
