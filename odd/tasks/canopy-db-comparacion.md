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
| T4 | S1, S2 (part), S7 | inline | done; AC-M10 repeat half NOT MET (external load) | Run 1 `2026-10-22-120128-fea8c09`: PASS, 154 runs, 11 SAME, 3,966.6 s. Run 2 `2026-10-22-130851-82ffd76` with `-Baseline` run 1: PASS, 154 runs, 11 SAME, 3,210.0 s; 37 time rows `outside tolerance`, every memory, size and count row inside. Both reports committed; H5 links them. |
| T5 | S8 | inline | done | fmt and clippy clean; Windows 560 passed, 0 failed (61 binaries); Linux (WSL Ubuntu, fresh clone of `b037f18`) 562 passed, 0 failed (61 binaries); bench tests 19 passed after the fixes (17 at T5); acceptance: query 106/106, CanopyDB query 151/151, generator 15/15, CanopyDB generator 17/17. |

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
- **L7** RDD assessment of `4826d1a..44ef53a` (T2+T3): `medium`, `review_due` false, `under_budget`; the
  slice stays pending until the closing review. Risk of T2: item 3 (it changes `query-output.ps1` and
  `Initialize-FixedQueries`, which the query app acceptance also uses): independent verifier at the
  feature's end, after T5. T4 starts: bench run 1 on the reference CSV, 5 repetitions, `-WorkDir
  work\comparison`, machine without other load.
- **L8** T4 (2026-10-22 12:01–14:02): run 1 (`-WorkDir` mistyped from bash as `work\comparison`, so it ran in
  `workcomparison` at the repo root; moved to `work/comparison-run1` afterwards, nothing deleted; the report
  shows that path) and run 2 (`work/comparison`, `-Baseline` run 1). Both `-SkipBuild` with the binaries built
  by the smoke run (no Rust change since). AC-M2: SAME in the 11 queries in both runs (met). AC-M10: the
  report has every section and figure (met); the repeat has 37 time rows outside the tolerance (not met).
  Cause, from the data table: external load episodes that slowed both engines at once (F3 runs 2–5 of run 1:
  1.1 s to 4.5 s in both pairs; F6 runs 1–3: two to three times slower), and run 2's generations slower
  (156.4 s against 111.6 s) while its queries were faster and steadier (its Batch table's within-run `Out of
  tolerance` is 0, against the Baseline section's seven batch rows outside the run-1 medians). Memory,
  sizes and counts reproduce. Decision (default, no questions during the run): KM5 is not widened, since its
  rule covers the variation of the reference machine without other load and the needed widening (about
  80 %) would make the bench useless; AC-M10's repeat stays pending a run on a quiet machine, in the closing
  report. README and script help: measured duration 54 and 66 minutes instead of the 40 estimated.
- **L9** T5 (2026-10-22): the four acceptance checks ran one after another on `b037f18` (14:03–16:10, slower
  than their README figures, the machine busy again): `run-query-acceptance.ps1` PASS 106 checks,
  `run-canopy-query-acceptance.ps1` PASS 151, `run-acceptance.ps1` PASS 15, `run-canopy-acceptance.ps1` PASS 17
  (reports in `work/acceptance-*`). The two query checks run the helpers moved by T1 and T2, so S8's "siguen
  pasando" holds and T1's risk (item 6) is closed. `cargo fmt --all --check` and `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo test --workspace` Windows 560 passed, Linux 562 passed (WSL
  Ubuntu, fresh clone of the branch bundle in `~/canopy-db-comparacion-<hhmmss>`), 0 failed; bench tests 17
  passed. A first WSL attempt failed on shell quoting, not on tests; `rm -rf` of the old WSL clone was
  refused by the safety check, so the old clone `~/canopy-db-consultor` and the new one stay.
- **L10** Independent verifier (sonnet, high) on `8786248..b037f18`: S1, S3–S7 met; S2 report half met, repeat
  half not met (known, L8; it recomputed the 104 Baseline rows: 0 mismatches, 37 outside); S8 partly met in
  its view only because T5 was not yet logged (it is, L9). It recomputed 224 figures per report from the
  data tables with its own decimal code: 0 mismatches; it confirmed both `.cs` files are byte-identical to
  the old inline blocks and the moved readers unchanged. Blocker B1: a relative `-Csv` or `-Baseline`
  reached .NET unresolved, so with the process folder different from the PowerShell location the bench read
  another file or failed with a raw .NET error (the query acceptance resolves `$Csv` up front). Fix in
  `dedc982`: both resolved with `Resolve-Path` right after their existence checks; the `Baseline` header keeps
  the text as passed. Regression test "Relative -Csv and -Baseline follow the PowerShell location, not the
  process folder": RED (`ReadAllText` looked for the baseline under the temp folder), then GREEN, 18 of 18
  tests pass. Advisories kept as they are: a failed single run 0 shows `—` while a failed batch run 0 keeps
  the counts it gave (KM6 asks for that in the batch); the Baseline header check tolerates a BOM; Ctrl+C has
  no test (no AC); report 1 records the mistyped work folder (L8); the 14 GB need is an estimate (the kept
  folders took 8.4 GB). The verifier's scratch folders were removed.
- **L11** Closing RDD review. The whole last slice `4826d1a..3bb79ad` was refused at START with
  `lens_context_budget_exceeded` (no authority created; the two report data tables are ~880 rows each), so
  it is reviewed as smaller candidates. Review 2 — `4826d1a..44ef53a` (the bench commit), from a detached
  HEAD at `44ef53a`, back on the branch afterwards: assessed `medium`; lenses resilience, readability,
  reliability; approved and acknowledged (authority burned), lineage `review-d9eb9280bf3778a4`. Findings, all
  informational: R2-001 SUGGESTION, the peak parsed with `ConvertFrom-SpanishSeconds`: renamed
  `ConvertFrom-SpanishDecimal`; R2-002 SUGGESTION, the `<número>` placeholder became an unnamed literal: named
  `$FixedQueryPlaceholder` in `query-output.ps1`; R3-002 SUGGESTION, a baseline with the header but a damaged
  row stopped with a raw cast error: `Read-ComparisonData` now rejects it with the PRD message, new test
  "AC-KM3: a baseline with the header but a damaged row is not a data file" (19 of 19 pass); R3-001 WARNING,
  `Split-FirstBatch` relies on run 0's result counts to split the batch output: kept, since a batch run 0
  without a query's stats line is already a failed run and its queries from there on are `INCOMPLETE`.
- **L12** Review 3 — `b037f18..5a34839` (B1 fix, review fixes, logs): `medium`; lenses reliability,
  readability; approved and acknowledged, lineage `review-1a642aa7c1bc8975`. Findings, informational: R2-001
  SUGGESTION, T5's row gave the bench tests before the fixes (17): the row now says 19; R2-002 SUGGESTION,
  `Read-ComparisonData`'s comment reads unevenly and does not say one bad row rejects the file: left as a
  follow-up; R3-001 WARNING, the rename has no alias for callers outside the candidate: checked, no script
  calls the old name (`grep` gives 0); R3-002 SUGGESTION, no test for a wrong field count or a non-numeric
  exit code in a baseline: left as a follow-up. Review 4 — `44ef53a..b037f18` (the two reports, H5,
  measured duration, logs), from a detached HEAD at `b037f18`: `medium`; lens readability; approved and
  acknowledged, lineage `review-3314685d6ed26845`. Findings, informational: R2-001 SUGGESTION, report 1 names
  the mistyped work folder with no note: kept as generated (explained in L8 and the closing report); R2-002
  SUGGESTION, L8's "0 out of tolerance" mixed the two tolerance checks: L8 now says which; R2-003 SUGGESTION,
  `run-canopy-comparison.ps1` gained a UTF-8 BOM: intended, Windows PowerShell 5.1 needs it in a script that
  may hold non-ASCII text, as the other acceptance scripts have. Every commit of the branch after the PRD has
  now been through RDD: reviews 1 to 4 cover `8f30aec..5a34839`.

## Closing

Closing report: [canopy-db-comparacion-report.md](canopy-db-comparacion-report.md).
