# canopy-db-comparacion — ODD feature document

- **Objective**: implement `docs/prd/chicago-crime/canopy-db-comparacion.prd.md` v1.0 (the comparison bench
  `run-canopy-comparison.ps1` and its first report; verifies AC-M2 with the bench, AC-M10, AC-KM1–AC-KM6). Branch
  `feat/canopy-db-comparacion` from `8786248`. Delivery: local work-unit commits, no PRs, merge into the base
  only when the user says so (worktree rule). Runners: the bench's PowerShell tests and `cargo test --workspace`.
- **Timing**: estimate **3 h – 4 h 30 min**. Real start **2026-10-22 11:33:35 +0100**. Phase log
  `odd/tasks/canopy-db-comparacion-phases.tsv`.

## Specs

The PRD is the spec and wins on any question: its flow, its "Validaciones y mensajes" tables, its "Salida"
(report header, sections, data table) and decisions KM1–KM10 are quoted there verbatim and are not repeated
here. Every rule of its ancestry (canopy-db K1–K10, consultor KC1–KC9, lote B1–B7, root D1–D6) holds. The
criteria, verbatim:

- **S1 — AC-M2** (con el banco) — Dado el CSV de referencia, cuando se ejecuta el banco, entonces la sección
  `Results` del informe dice `SAME` en las once consultas y el banco termina con 0.
- **S2 — AC-M10** (K3, KM5) — Dado el banco, cuando se ejecuta sobre el CSV de referencia, entonces el informe
  tiene la cabecera y todas las secciones de Informe, sin `—` en las secciones `Generation`, `Single queries` y
  `Batch`; y cuando se ejecuta otra vez, sin cambios en el código entre las dos, con `-Baseline` apuntando a la
  tabla de datos de la primera, entonces ninguna fila de `Baseline` dice `outside tolerance`, con la tolerancia
  de KM5 o la que fije su ampliación por ruido del equipo.
- **S3 — AC-KM1** (KM6) — Dado un banco en el que un consultor devuelve en una consulta un número de resultados
  distinto del otro, cuando termina, entonces esa consulta dice `DIFFERENT`, la línea `Result` dice
  `FAIL: 0 of <n> runs failed, 1 of 11 queries returned different results`, las demás secciones tienen sus
  cifras y el código es 1.
- **S4 — AC-KM2** (KM6) — Dada una ejecución de una aplicación que termina con un código distinto de 0, cuando
  el banco termina, entonces `Notes` la muestra con su código, la tabla de datos tiene sus filas con ese código,
  no cuenta en las medianas, `Result` dice `FAIL` y el código es 1; y cuando fallan todas las generaciones de
  una pareja, el banco no ejecuta consultas, `Results` dice `NOT RUN` en las once y `Result` termina en
  `queries not run`.
- **S5 — AC-KM3** — Dado cada caso de las comprobaciones previas de Validaciones y mensajes, cuando se ejecuta
  el banco, entonces muestra su mensaje, termina con 1 y no ejecuta ninguna aplicación ni escribe informe; y
  con dos casos a la vez, muestra el del orden menor.
- **S6 — AC-KM4** (KM8) — Dado un informe, cuando se lee su tabla de datos, entonces tiene la cabecera y una
  fila por medida de cada ejecución, incluidas la 0 y las fallidas, con los valores de Tabla de datos, y cada
  mediana del informe coincide con la calculada de ella; con `-Repetitions 1` y `-Repetitions 2`, las medianas
  siguen las reglas comunes.
- **S7 — AC-KM5** — Dado el CSV de referencia en el equipo de referencia, cuando se cierra este hijo, entonces
  el primer informe y su tabla de datos están confirmados en el repositorio con `Result: PASS`, y el README, la
  ayuda del script, las guías de Linux y H5 tienen lo que pide Documentación.
- **S8 — AC-KM6** — Dado el repositorio en Windows y en Linux (Ubuntu 24.04 sobre WSL2), cuando se pasan las
  pruebas, entonces pasan todas, y en Windows las cuatro verificaciones de aceptación siguen pasando.

## Tasks

| Id | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S8 (no regression) | inline | done (`72843da`; acceptance proof in T5) | Extract the C# helpers and the query-output readers of the acceptance scripts into shared files, no behavior change. Smoke: both `.cs` compile, `Get-Stats`, `Split-BatchWords`, `Read-BatchStderr` answer as before, 0 parse errors in the three query scripts. |
| T2 | S1–S6 | inline | done (feat commit after `4826d1a`) | The bench: `canopy-comparison.ps1` (library), `run-canopy-comparison.ps1` (script and help), `tests/canopy-comparison.tests.ps1` (17 tests, fake applications): all pass. Smoke on 200,000 rows of the reference CSV, R=1: `PASS: 50 runs, same results in all 11 queries`. |
| T3 | S7 | inline | done with T2 (H5 link in T4) | Script help, README section "Compare the two pairs" and layout, Linux guides. |
| T4 | S1, S2, S7 | inline | pending | Two bench runs on the reference CSV; first report and data table committed; H5 link. |
| T5 | S8 | inline | pending | fmt, clippy, tests on Windows and WSL; the four acceptance checks on Windows. |

Route evidence: one understood change per task, no parallel units, so every task stays inline (no writer
trigger). T1 touches shared acceptance code (risk item 6: only the acceptance runs of T5 catch a regression).

## Log

- **L1** (2026-10-22, user, verbatim): "Arranca la ejecucióbn" — on the PRD `canopy-db-comparacion.prd.md`,
  written in this session ("Escribe el prd para el punto 4 canopy-db-comparacion").
- **L2** Estimate 3 h – 4 h 30 min, confirmed by the user ("Sí"); start 11:33:35 (phase log `--start`).
- **L3** Exploration: no bench exists; the acceptance scripts hold what the bench needs: `LiveRun` (C#; exit time
  taken by a waiting thread, no polling), `QueryAcceptance.PeakPrivateBytes` (the peak, readable after the exit
  while the handle is open), `FirstSharedCaseNumber`, `CompareRowSetsHashed`, `CheckLimited`,
  `SplitBatchOutput`, `Split-BatchWords`, `Get-Stats`, `Read-BatchStderr` and their regexes, and
  `reference-extract.ps1`. They are embedded in `run-query-acceptance.ps1` and `query-acceptance-canopy.ps1`,
  so T1 moves them to shared files instead of copying them. CanopyDB settings (header line): generator
  `page_cache_size` 64 MiB, `use_wal` false, `write_txn_memory_limit` and `checkpoint_target_size` 32 MiB,
  `throttle_memory_limit` and `stall_memory_limit` 128 MiB, `wal_write_batch_tempfile_dir` the temp folder;
  query app `page_cache_size` 1 MiB, `file_lock_timeout` 0 s (its database options are the stored ones). RDD is
  on (global). Windows PowerShell 5.1; Pester 3.4 only, so the tests are a plain PowerShell script.
- **L4** Defaults taken without asking (CLAUDE.md: no questions once the run starts): the bench's tests live in
  `scripts/acceptance/tests/` and run with `powershell -File`; testing seam = the bench library's main function
  takes its process runner and environment probes as parameters (the script passes the real ones), so no visible
  option exists; CanopyDB settings are read from the Rust sources of the measured commit, `unknown` per setting
  when a pattern no longer matches.
- **L5** RDD review 1 — commits `8f30aec..4826d1a` (T1 and this document; base `8f30aec`, assessed `high`,
  process boundary in the moved C#). Lenses readability, reliability. Result: approved and acknowledged
  (authority burned), lineage `review-406a602ce04b176f`. Findings, all informational: R2-001 and R3-001
  WARNING, `query-acceptance-canopy.ps1` uses `Split-BatchWords` from `query-output.ps1` without saying so
  (it gets it through `run-query-acceptance.ps1`): resolved in T2 with a header note; R2-002 and R3-002
  SUGGESTION, the `-as [type]` guard keeps a stale type in a long-lived session: resolved in T2 with a comment;
  R2-003 SUGGESTION, comments name the bench before it exists: resolved by T2; R2-004 SUGGESTION, T1 still
  `pending` in this table: updated. Reviewed boundary: `4826d1a`.
- **L6** T2 (2026-10-22): the bench library, script and tests were written together; RED was observed on the
  bench, not test-first: the first test run failed 15 of 17 (an array passed whole through `Sort-Object`, a
  `@()` that wrapped a series again so `Runs` read `1 of 2`, a PowerShell 5.1 `foreach` over `@(List)` under
  strict mode, and patterns of the tests), then all 17 passed. Also moved to `query-output.ps1`: the JSON
  readers and `Read-FixedQueries` (the acceptance's `Initialize-FixedQueries` now uses it); the stats and
  batch patterns gained a `seconds` group. Defaults taken: the header's CanopyDB settings list what each
  app's own source sets (the query app also opens with the database options the generator stored), without
  `compress_overflow_values` (the Options line shows compression); a failed run's note keeps the stderr
  line's own final dot (`...existente..`), as the PRD's literal `<motivo>.` gives. Smoke on a 200,000-row
  CSV (R=1, real build): PASS, 50 runs, 119.6 s; its report was deleted, not kept.
