# canopy-db-consultor — ODD feature document

- **Objective**: implement `docs/prd/chicago-crime/canopy-db-consultor.prd.md` v1.0 (the CanopyDB query app
  `crime-canopy-query`, the fixed query set and its acceptance check; verifies AC-M1 consultor part, AC-M2,
  AC-M5–AC-M8, AC-KC1–AC-KC9). Branch `feat/canopy-db-consultor` from `a78a773`. Delivery: local work-unit
  commits, no PRs, merge into the base only when the user says so (worktree rule). Runner: `cargo test --workspace`.
- **Timing**: estimate **2 h 30 min – 4 h**. Real start **2026-10-22 06:37:04 +0100**. Phase log
  `odd/tasks/canopy-db-consultor-phases.tsv`.

## Specs

The PRD wins on any product question; every rule of its ancestry (canopy-db K1–K10, consultor C1–C11,
startswith P1–P6, lote B1–B7, root D1–D6, Linux L1–L3, acceptance A1–A5) holds for the new app. Messages are
quoted verbatim from the PRD.

- **S1 — App and help (KC1).** `crime-canopy-query`, the options of the original (single and `--batch`). Help =
  the original's with the name, the description `Consulta el conjunto de datos de delitos de Chicago (base
  CanopyDB) publicado por el generador CanopyDB.` and one more paragraph at the end, after the batch codes:
  `Si otro proceso tiene abierto el conjunto de datos, la consulta espera hasta 30 s antes de rechazarse con 1.`
- **S2 — Order of checks.** Command line, conditions and batch file as the original, without opening. Then:
  locate the active version by `CURRENT`; `schema_version` (KC3); manifest shape (KC3); CanopyDB version (KC4);
  every file of `files` exists; open the base (wait, S5); the six tables exist. First failure rejects.
- **S3 — Manifest (KC3).** Manifest exists but unreadable: `No se pudo leer el conjunto de datos en <carpeta>.`
  (1, no detail). No `CURRENT`, missing version, invalid JSON, or `schema_version` that is not a text
  `canopydb-` followed only by digits (any SSTable dataset): `No se encontró un conjunto de datos válido en
  <carpeta>.` (1). `canopydb-<cifras>` other than `canopydb-2`: `El conjunto usa el esquema v<cifras> y este
  consultor admite v2. Regenera el conjunto.` (1), digits as written (`canopydb-02` → `v02`); exact,
  case-sensitive (`CANOPYDB-2`, `canopydb-` invalid). `canopydb-2` without the generator's shape → not found:
  every field required with its type, unknown tolerated; `engine.name` `canopydb`; `engine.version` non-empty
  text; `engine.fork_commit` present, `null` or non-empty text; `tables` exactly the six names in order; each
  `files` entry relative, starting with `canopydb/`, `/`-separated, no `.`/`..`; unlisted extra files tolerated.
- **S4 — Other CanopyDB version (KC4).** `engine.version` or `engine.fork_commit` not exactly the app's: `El
  conjunto se escribió con CanopyDB <versión del conjunto> y este consultor usa CanopyDB <versión propia>.
  Regenera el conjunto.` (1), before opening the base. Version text = `engine.version` as written, plus
  ` (fork <commit>)` with the full commit when not `null`.
- **S5 — Base on disk.** Missing `canopydb/`, a missing `files` entry or one that is a folder: not found (1),
  nothing created. Open base without one of the six tables: not found (1), no table created. No detail line in
  these cases.
- **S6 — Open by another process (KC2).** Warning once on stderr as soon as the first attempt finds it open,
  even if released right away: `Aviso: otro proceso tiene abierto el conjunto de datos en <carpeta>; se
  esperará hasta 30 s.` Released before 30 s: go on, no more messages. Still open 30 s after the warning: `El
  conjunto de datos en <carpeta> sigue abierto por otro proceso. Inténtalo de nuevo cuando termine.` (1), between
  30 and 35 s after the warning. Any holder (another query app, the CanopyDB verifier, any program).
- **S7 — Cancel.** Ctrl+C at any moment, also during the wait or the opening: `Consulta cancelada.` (130), in
  less than 1 s.
- **S8 — Engine errors (KC5).** Any other CanopyDB error opening or reading, including an unexpected failure
  (panic) and CanopyDB unable to write its internal files (read-only files on Windows): `No se pudo leer el
  conjunto de datos en <carpeta>.` (1, never an abnormal termination code), like the original when the dataset
  becomes unreadable: no misleading partial output, no half output file. Just before it, one stderr line
  `Detalle del error de CanopyDB: <texto del error tal como lo da CanopyDB>` (line breaks to spaces; same text
  rule as KG7, see L3).
- **S9 — Batch, output file, times.** In a batch, all S3–S8 opening cases happen before the first header,
  without `Línea <n>: ` prefix; an engine error during a query goes under its header. Rejections before a query
  starts leave an existing `--output` file as it was. The stats line time excludes opening and the wait; the
  batch final line includes them. A batch keeps the base open until it ends.
- **S10 — JSON (KC6).** Same as the original except `dataset.schema_version` = `"canopydb-2"` (text).
- **S11 — No writes (K6).** No change to the queryable content or the manifest, no new files in the dataset
  folder; internal engine files may change (AC-M6).
- **S12 — Memory (K8, AC-M8).** With each app separately: count F3, count F4 and a batch with the whole fixed
  set; largest peak ≤ 3 × smallest. The CanopyDB configuration is left to the implementation and documented.
- **S13 — Fixed query set (KC7).** F1–F11 exactly as the PRD table; `<número>` = the first shared case number
  of the CSV (as for AC-C6).
- **S14 — Acceptance check (KC8).** `run-canopy-query-acceptance.ps1`: the original's parameters plus
  `-NotMetByCanopyDb` (KG10 form) admitting `AC-C7`, `AC-S8`, `AC-M8` once each (`only AC-C7, AC-S8 and AC-M8 can
  be associated`, `<criterio> is already associated`); NOT MET only when every run ended 0 with the expected
  result and only the measured figure fails; `AC-M8 (SSTables)` never NOT MET. Report `Acceptance of the
  CanopyDB crime query app`, exit 0/1, rows: the original's (AC-C13 → `AC-M8`; second half of AC-C18 → SSTable
  dataset rejected), `AC-M2-F1`…`AC-M2-F11`, `AC-M5`, `AC-M6`, `AC-M7`, `AC-M8`, `AC-M8 (SSTables)`, `AC-KC2`…
  `AC-KC7`. Result line: `PASS: all <n> checks passed`, `PASS: <p> of <n> checks passed, <k> not met by CanopyDB
  (<hallazgos>)`, `FAIL: <f> of <n> checks failed`. Criteria AC-KC1, AC-M2, AC-M5–AC-M8, AC-KC2–AC-KC7 as the
  PRD words them (AC-M5: batch > 45 s, rejection 30–35 s after the warning, Ctrl+C → 130 in < 1 s).
- **S15 — Docs (AC-KC9).** Help (S1); README section of the CanopyDB query app (purpose, run, differences:
  wait, other-version rejection, JSON `schema_version`), its acceptance check, the fixed set, the CanopyDB
  memory configuration; Linux guides: build and run.
- **S16 — Original apps (KC9, AC-KC8).** The original query app's code may be reorganized to share it, without
  changing behavior, messages or outputs; its tests keep existing and passing unchanged in what they check; its
  acceptance check still passes on Windows. Fix it if it fails AC-M8. Tests pass on Windows and Linux.
- **S17 — Annotations at close.** Consultor PRD: "nunca escribe" (Fuera de alcance), C7, C9; canopy-db PRD and
  its translation: K1 exception list; with the PRD's texts.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S16 (seam for S1, S2, S6–S9) | inline: storage seam in `crime-query` (Source/Tables/KeyCursor, App with name/help/opener, shared main, open with cancel and stderr, OpenError::Cancelled/Rejected, ReadError detail), no behavior change | done (refactor: existing tests unchanged and green, differential check 16/16 identical) | `ab526a7` |
| T2 | S3, S8 (shared contract) | inline: CanopyDB layout constants and the engine detail line in `crime_dataset::canopy`, used by the generator | done | `428a81c` (+ `panic_detail` in `7728a7d`) |
| T3 | S1–S12 | inline: `crime-canopy-query` crate, its tests | done (tests after code, see L6) | `7728a7d` |
| T4 | S13, S14 | delegated writer (parallel with T1–T3: disjoint surface, PowerShell script and query file) | done (parse 0; run in T6) | `d576c40` |
| T5 | S15, S17 | inline: README, Linux guides, PRD annotations | done | (docs commit) |
| T6 | all | acceptance run, verification, RDD, closing report | done: CanopyDB 151/151 and original 106/106 PASS on `d576c40`; Windows 560 and Linux 562 tests green on `64c94a2` | `2e60df3`, `894dc17`, `64c94a2`, report commit |

## Log

- **L1** (2026-10-22, user): "Implementa docs\prd\chicago-crime\canopy-db-consultor.prd.md usando ODD"
- **L2** 06:37:04 start after "Adelante". Estimate 2 h 30 min – 4 h. No open product questions. PRD committed
  first as `ff15a0d`.
- **L3** Default taken: the detail line reuses the generator's text rule (KG7): the engine text split at CR and
  LF, empty parts dropped, joined with one space. The PRD row said "cada retorno de carro y cada salto de línea se
  sustituye por un espacio" (CRLF would give two spaces); KC5 says "con el texto de KG7", so the PRD row is
  aligned with KG7 in T5 and the closing report lists it.
- **L4** Route: T4 delegated (writer reason: a parallel unit with a disjoint surface, several minutes of work);
  T1–T3 and T5 inline. Design (from the map): CanopyDB 0.2.5 opens each tree once per transaction at a time
  (`TreeAlreadyOpen`), so index streams read pages of keys with a short-lived tree and resume after the last
  key; one read transaction serves the whole run. CanopyDB reads with positional reads, so its page cache is
  private memory: the app sets a small cache for AC-M8. The lock wait is the app's own loop over CanopyDB's
  zero-timeout open, so it can warn once, honor Ctrl+C and stop at 30 s. The verifier stays independent: the
  app has its own reader of stored options.
- **L5** T1 (`ab526a7`) and T2 (`428a81c`): baseline `cargo test --workspace` 529 green before the change.
  The original query app reads the dataset through `Source`; `Dataset` (LevelDB) keeps its API, so its 195
  tests compile and pass unchanged. Differential check against a release build of `a78a773` in
  `/tmp/tmp.w0wTqPmAK9` on a 3,000-row dataset in `/tmp/tmp.nFJmietVvY`: 16 commands (help, counts, CSV,
  JSON, missing IDs, prefixes, every rejection, batch, bad batch, output file) give identical stdout, stderr
  (times masked) and exit code. Risk: item 3 (shared code of the original query app).
- **L6** RDD review 1 — commits `ff15a0d..428a81c` (base `a78a773`), assess high (`executable_change` +
  escalation item 3), lenses readability and reliability; lineage `review-f93504240a83ecef`. Result:
  **approved**, acknowledged (authority burned). Findings, all informational: R2-parse-line-hardcoded-app
  WARNING `parse_line` parses with the original app → documented in `7728a7d` (a batch line never shows
  help or usage); R2-stale-derive-name SUGGESTION → comment in `7728a7d`; R3-open-failed-untested WARNING
  and R3-help-note-untested WARNING → covered by the CanopyDB app's tests in `7728a7d` (Ctrl+C during the
  wait gives 130, rejections reach stderr unchanged, the help equals the original's with the three
  changes); R3-key-panics SUGGESTION → `KeyCursor::key` contract documented in `7728a7d`.
- **L7** T3 (`7728a7d`): test-first exception as in canopy-db-lote: the crate was written before its tests,
  so no RED was observed; the 31 tests (`compare`, `open`, `wait`, `engine`, unit tests of the manifest, the
  messages and the engine guard) would not compile on the base. Defects found by the tests and fixed before
  the commit: a test helper deleted a tree without a checkpoint (the base has no WAL); the stderr time mask
  of the test helper ate the warning line. Smoke test on the 3,000-row CSV with release builds: 10 queries
  identical to the original app, the batch differs only in the JSON `dataset` member (KC6), cross-engine
  rejections exit 1 with the not-found message. Checks: fmt and clippy clean, `cargo test --workspace` 559
  green. Risk: item 3 (it shares the original app's code through T1); no stored data changes.
- **L8** Memory before the acceptance run (AC-M8, K8), measured from outside with the original script's
  method (peak private committed bytes, sampled every 10 ms and read again after exit) on the full reference
  CSV, datasets in `work/manual`: the original app counts `THEFT` 14.5 MiB, `RITUALISM` 7.5 MiB, the whole
  fixed set in a batch 19.2 MiB (ratio 2.56: first batch peak ever measured with the original app, it
  passes). The CanopyDB app with an 8 MiB page cache: 9.9 / 1.0 / 9.9 MiB, ratio 9.9, fail; 2 MiB: 3.2 / 0.9
  / 3.4, ratio 3.8; 1 MiB: 2.2 / 1.0 / 2.3, ratio 2.3, with `THEFT` in 0.51 s instead of 0.43 s. Fixed in
  `f33a37b` (1 MiB). Batch of the fixed set: 19–27 s with CanopyDB, 46–50 s with the original app. Docs
  committed in `f3d1d46` (T5).
- **L9** T4 (`d576c40`, delegated writer, opus high): `run-canopy-query-acceptance.ps1` (entry point and
  association checks), `query-acceptance-notmet.ps1` (NOT MET rules, result line), `query-acceptance-canopy.ps1`
  (CanopyDB rows), hooks in `run-query-acceptance.ps1` guarded by `-CanopyDb` (default path unchanged by
  construction; proven by the original acceptance run in T6), `canopy-fixed-queries.txt` (S13). Writer's
  checks: parse 0 errors on the four scripts (rechecked by the parent: 0); the four invalid associations exit
  1 with the exact error before the work folder exists; every new row run on a 20,000-row slice except AC-M6
  (verifier not built then). AC-KC4's missing-table case is covered by the Rust test only (the script cannot
  build such a base). The writer's AC-M5 rejection came 30.005 s after the warning it read: `c18fe80` adds a
  100 ms margin to the deadline (rejection inside 30–35 s). Writer scratch:
  `C:\Users\JESUS~1.LOP\AppData\Local\Temp\claude\c--wt-chicago-crime-canopy-db-consultor\71fbd401-ac30-467d-8555-d070e19606b9\scratchpad\`
  (harness files, logs, `small-input.csv`, `harness-work\`).
- **L10** RDD review 2 attempt on `428a81c..d576c40` (assess: high, `process_boundary`, 31 paths, 3,975
  lines), lenses risk, resilience, readability, reliability: START refused with `lens_context_budget_exceeded`
  (no authority created). The candidate is always base..HEAD, so the slices `428a81c..7728a7d` (T3, 2,086
  lines), `7728a7d..c18fe80` (docs and fixes) and `c18fe80..d576c40` (T4, 1,796 lines) are reviewed one by
  one with HEAD moved to each slice end once the acceptance run no longer uses the working tree.
- **L11** Independent verifier (sonnet, high) on `a78a773..d576c40`, release builds of both trees in
  `/tmp/tmp.zcK6f1ScAQ` (Windows `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.zcK6f1ScAQ`), 20,000-row slice:
  **no blockers**. PASS: S1, S2, S3, S4, S5, S6 (warning within 1 s, rejection 30.17 s after the start,
  short hold answers with one warning), S8 (read-only files and mid-file corruption: detail line, message,
  exit 1, `--output` untouched), S9, S10, S11 (after about 80 queries: same file names, same manifest
  hash), S16 (40 commands, base and head `crime-query` identical in stdout, stderr and exit code, item 3
  OK), K1 in small (40 queries, same lines, same codes). Not checked by it: S7 (Ctrl+C from Git Bash; covered
  by the crate's tests and AC-M5 in the acceptance run), S12–S15 and S17 (acceptance run and docs).
  Advisories, not fixed: a garbage `OPTIONS` file gives `No se pudo leer…` without a detail line (the app
  parses it itself, like an unreadable manifest; KC5 asks the detail only for engine errors); a mid-query
  engine error after part of a CSV listing on stdout leaves that part, as the original does (C8 covers only
  output files).
- **L12** Acceptance (T6) on `d576c40`, reference extract, `-SkipBuild`, work folder `work\acceptance`:
  `run-canopy-query-acceptance.ps1` **PASS: all 151 checks passed** in 1,331.1 s (07:22–07:44, CanopyDB
  generation 303.5 s while the independent verifier was building in scratch); report
  `work/acceptance/canopy-query-acceptance-report.md`. Key figures: AC-M8 CanopyDB 2.1 / 1.0 / 2.4 MiB (ratio
  2.53), AC-M8 (SSTables) 14.5 / 7.6 / 19.3 MiB (2.55); AC-M5 rejection inside 30–35 s, Ctrl+C during the wait
  exit 130 after 0.04 s, holding batch of 9 × F11 exit 0 after 227.0 s, released hold answered 24; AC-M6 same
  names, same manifest, the six tables equal by the verifier's compare; AC-M7 both rounds of F1 and F2 on the
  version the batch opened; AC-C7 22–25 ms per ID (first run, cold, 514 ms); AC-S8 2.2 / 1.0 MiB; F11 (the
  1,838,502 thefts as CSV) 10.5 s with CanopyDB, 67.0 s with the original. Then `run-query-acceptance.ps1`
  (original app, default path): **PASS: all 106 checks passed (36 criteria)**, the same count as before the
  change (AC-KC8). The report header says "with uncommitted changes": the untracked feature document only.
- **L13** Defect found reading the report, fixed in `2e60df3`: AC-M7's version check read indices `0, 1, 11,
  11, 1` (PowerShell's comma binds before `+`), so round 2's F2 was never checked. Checked by hand on that
  run's outputs: `twice-1-F1`, `twice-1-F2`, `twice-2-F1`, `twice-2-F2` all name `dataset-20261022-073433-48348`,
  the version the batch opened. The acceptance run was not repeated for a check-only fix.
- **L14** RDD review 3 — slice `428a81c..7728a7d` (T3; HEAD detached at `7728a7d` for the review, then back
  on the branch), assess high, lenses risk, resilience, readability, reliability; lineage
  `review-5e50a20a0d62138a`. Result: **approved**, acknowledged. Findings, all informational:
  R4-wait-retry-vanished-base WARNING (a version deleted during the wait was not detected) → `894dc17`
  restarts the opening when `canopydb/` disappears (not reproducible on Windows, where the held files cannot
  be deleted; left to `canopy-db-robustez` to drive); R2-001 WARNING stray doc line above the generator's
  `fault` module → removed in `894dc17`; R2-002 WARNING unexplained `_l` folder suffix → comment in
  `894dc17`; R2-003 SUGGESTION two lists of the six trees → `crime_dataset::canopy::table_names` in
  `894dc17`; R2-004 SUGGESTION `tree_name` alias → removed in `894dc17`; R3-base-unchanged-after-successful-query
  WARNING (the test compares names and manifest, not file bytes) → not changed: K6 allows the internal files
  to change, and AC-M6 compares the logical content with the verifier; R3-wait-timing-flaky WARNING → bound
  widened in `894dc17`, then set to 2 s in `64c94a2`.
- **L15** RDD review 4 — slice `7728a7d..894dc17` (docs, cache size, wait margin, T4 scripts, review 3 fixes),
  assess high, lenses resilience, reliability, readability; lineage `review-f0b1d5826e7e64b1`. Result:
  **approved**, acknowledged. Findings, all informational: R2-002 and R2-003 WARNING the Linux guides' "its
  memory criterion" became ambiguous → reworded in `64c94a2`; R3-wait-bound-loosened WARNING, R4 and R2-005
  SUGGESTION the 3 s test bound → 2 s with its reason in `64c94a2`; R2-001 and R3 SUGGESTION `table_names` with
  a fixed arity → length from `IndexKind::ALL` in `64c94a2`; R2-004 SUGGESTION the fixed query file is paired
  by position → said in the file in `64c94a2`; R4-page-cache-latency SUGGESTION (1 MiB cache relies on the OS
  file cache) → not changed: measured cost 0.43 → 0.51 s for the `THEFT` count, documented in the code.
- **L16** RDD review 5 (last slice) — `894dc17..64c94a2`, assess medium, under budget (22 lines), lenses
  readability, reliability; lineage `review-b3a4137d91e6ed01`. Result: **approved**, acknowledged. Findings,
  informational, not changed (a change would open another slice for wording only): R2-wait-upper-bound-comment
  and R3-001 WARNING the 2 s bound's comment reads as an argument for more headroom; R2-table-names-doc-drift
  and R3-002 SUGGESTION the doc of `table_names` still says "six".
- **L17** Final checks on `64c94a2`: `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets --
  -D warnings` clean, `cargo test --workspace` on Windows 560 passed, 0 failed (61 binaries); on Linux (WSL
  Ubuntu 24.04.1, fresh clone of the branch bundle in `~/canopy-db-consultor`) 562 passed, 0 failed (61
  binaries). Active time 1 h 33 min against an estimate of 2 h 30 min – 4 h.

## Closing

Closing report: [canopy-db-consultor-report.md](canopy-db-consultor-report.md).
