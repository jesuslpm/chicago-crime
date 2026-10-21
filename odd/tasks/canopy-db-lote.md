# canopy-db-lote — ODD feature document

- **Objective**: implement `docs/prd/chicago-crime/canopy-db-lote.prd.md` v1.0 (query batches in the original
  query app, AC-B1…AC-B10; verifies AC-M7 with the original query app and AC-M9). Branch `feat/canopy-db-lote`
  from `18ee4ba`. Delivery: local work-unit commits, no PRs, merge into the base only when the user says so
  (worktree rule). Runner: `cargo test --workspace`.
- **Timing**: estimate **1 h 30 min – 3 h**. Real start **2026-10-21 07:58:20 +0100**. Phase log
  `odd/tasks/canopy-db-lote-phases.tsv`.

## Specs

The PRD wins on any product question; every rule of its ancestry (canopy-db, consultor C1–C11, startswith
P1–P6, root D1–D6) still holds for each query of a batch. Messages are quoted verbatim from the PRD.

- **S1 — Option (B1).** "`--batch <ARCHIVO>` junto a `--dataset`." Each line admits "las opciones de una
  consulta suelta salvo `--dataset`, `--batch`, `--help` y `-h`, es decir `--where`, `--startswith`, `--range`,
  `--limit`, `--count`, `--format` y `--output`, también con la forma `--opción=valor`". With `--batch`, those
  options on the command line: `Con --batch, la opción <opción> va en cada línea del lote, no en la línea de
  comandos.` (2), naming the first one typed. Existing command-line errors win ("`--batch` sin `--dataset` da el
  mensaje de opciones obligatorias, `--batch` repetido el de argumentos no válidos y `--limit abc` el del
  límite, todos con el código 2"). "Con `--batch`, la línea de comandos no necesita ninguna condición."
- **S2 — File form (B2).** UTF-8 with or without BOM; LF or CRLF, mixed; last line may lack a newline; lines
  numbered from 1 as in the file. Ignored: empty or only spaces/tabs, and lines starting with `#` after optional
  spaces; "Un `#` en cualquier otro sitio es un carácter más. Una línea con algo más, aunque sean solo unas
  comillas vacías (`""`), es una consulta." Words: "solo los espacios y tabuladores separan"; double or single
  quotes group and "pueden abrirse a mitad de palabra"; inside quotes the other kind is a character; "No hay
  secuencias de escape: `\` es un carácter más"; any other character (lone CR, NUL, NBSP, BOM not at the start)
  is a character of its word.
- **S3 — Output (B3).** Stdout: each query exactly as alone, concatenated, nothing added; a query with
  `--output` writes nothing there. Stderr: per query `--- Consulta <i> de <total> (línea <n>) ---`, then its
  warnings, its error if it fails and its stats line exactly as alone; header and `Línea <n>: ` numbers without
  thousands separator. At the end `Lote completado: <total> consultas en <t> s.` (`1 consulta`), numbers and
  time as the stats line; the time covers the whole run (read and check the file, open the dataset, all
  queries). No final line when the batch is rejected, fails, is cancelled or stdout is closed. Output files equal
  the single query's on the same version.
- **S4 — Upfront checks and codes (B4).** Order: (1) command line (S1); (2) reading and encoding: `No se puede
  leer el lote <archivo>.` (1; missing, folder, unreadable, `--batch ""`, `--batch -`), `El lote <archivo> no es
  un texto UTF-8 válido.` (1); (3) each line in order: `Línea <n>: falta cerrar unas comillas.` (2),
  `Línea <n>: la opción <opción> no se admite dentro del lote.` (2), `Línea <n>: ` + the single query's message
  with its code (2 or 1, the message unchanged including ` Usa --help para ver las opciones.`), output reuse
  (S5); (4) `El lote <archivo> no contiene ninguna consulta.` (1); (5) opening the dataset, single messages
  without prefix. Only the first error; no check creates or modifies an output file.
- **S5 — Output reuse (B5).** `Línea <n>: el archivo de salida <ruta> ya lo usa la línea <m>.` (1) and
  `Línea <n>: el archivo de salida <ruta> es el propio lote.` (1), paths compared lexically: absolute from the
  current folder, without `.` and `..`, without touching the disk, case-insensitive on Windows. Whether an output
  file can be written is checked when its query arrives.
- **S6 — Failures and cancellation (B6).** A query that fails (dataset unreadable, output not preparable or
  writable, stdout fails) stops the batch with code 1, its message under its header; Ctrl+C (also before the
  first query or between two) stops it with `Consulta cancelada.` and 130; between two queries the next does not
  start and its header is not written. The current query leaves no half output file, earlier ones keep theirs,
  later ones do not run, no final line. Stdout closed: stop silently with 0, what stderr got stays. Left to the
  implementation: Ctrl+C after the last query ended may give 0 or 130.
- **S7 — Memory (B7).** No query keeps its results or anything derived from its query for the next; the only
  shared thing is the open dataset. The checked queries of the file may be kept.
- **S8 — Help and docs (AC-B9).** `--help` describes `--batch`, the file form, the options each line admits and
  the batch exit codes; the README explains the batch with an example.
- **S9 — Acceptance (AC-B1…AC-B10, AC-M7, AC-M9).** The query acceptance verification gains batch checks: every
  query it runs successfully alone, also in one batch, with the same count and record set (limits: as many as the
  limit, all from the unlimited result); the real Ctrl+C at least once; existing checks unchanged.
- **S10 — Annotations at close.** Root D6 and flow B step 1; consultor flow step 1 and C6; canopy-db K8, with
  the texts of the PRD's "Anotaciones al cerrar".

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S4, S5, S8 (help) | inline: `--batch` on the command line, batch file reading, words, upfront checks | done (tests after code, see L4) | `6c1070d` |
| T2 | S3, S6, S7 | inline: batch run, stderr headers, final line, failures and cancellation | done (tests after code, see L4) | `6c1070d` |
| T3 | S9 | delegated writer (parallel with T1–T2: disjoint surface, PowerShell script) | done (parse check 0; run in T5) | `bbdf250` |
| T4 | S8 (README), S10 | inline: README; PRD annotations at close | done | `8c71d58`, `4828ba8` |
| T5 | S9 | acceptance run on the reference CSV, verify, RDD, closing report | done: 106/106 PASS on `bbdf250` | `1c4dd7e` (review fixes), report commit |

## Log

- **L1** (2026-10-21, user): "Implementa docs\prd\chicago-crime\canopy-db-lote.prd.md usando ODD"
- **L2** 07:58:20 start after "Adelante". Estimate 1 h 30 min – 3 h. No open product questions: the PRD fixes
  every point. Default taken: the README is the query app's guide (there is no separate guide), so AC-B9's
  "guía del consultor" is the README section.
- **L3** Route: T3 delegated (writer reason: parallel unit with a disjoint surface, several minutes of work);
  T1, T2, T4 inline. PRD committed first as `f8f60b6` (docs only). Baseline `cargo test --workspace`: 453 green.
- **L4** T1+T2 in one commit `6c1070d` (+1364/−46, about 800 of them tests): the CLI, the batch module and the
  run refactor do not compile apart. Test-first exception: the implementation was written before the tests, so
  no RED was observed; the new tests (21: `tests/batch.rs`, 4 in `tests/cli.rs`, 4 unit tests of the words and
  paths) would not compile on the base. Two defects found by the tests and fixed before the commit: `--count`
  counted as typed next to `--batch` (clap gives it a default; now only values from the command line count),
  and `--batch ""` got clap's error instead of `No se puede leer el lote .` (now read as text). Checks: fmt and
  clippy clean, 474 green. Differential check against a build of `18ee4ba` in `/tmp/tmp.FONUejTUYE` on a
  3,000-row dataset: 14 single queries (counts, table, CSV, JSON, output file, every command-line error) give
  the same stdout, stderr (times masked), exit code and output file; only the two `--batch` lines differ, as
  requested. Risk: item 3 (the single-query parser and run path are refactored and shared). RDD assess on
  `18ee4ba..6c1070d`: high (`executable_change` + escalation item 3), review due.
- **L5** Independent verifier (sonnet, high) on `18ee4ba..6c1070d`: no blockers. S1–S7 PASS (about 120 probes on a
  3,000-row dataset in `/tmp/tmp.Puql0Ni8ny`), 18 single commands identical to the base build; S8 help PASS;
  real Ctrl+C and a mid-batch unreadable dataset not driven by it (covered by the Rust tests and the acceptance
  run). Advisories, not fixed (outside the PRD's domain): `-hx` on a batch line gives the clap argument error
  instead of `la opción -h no se admite`; `--batch f --output x.csv` without a format gives the existing output
  format error first, as S1 says; a word `-h=1` counts as `-h`.
- **L6** Acceptance: the first run failed before any check because the bash call passed `work\acceptance`
  unquoted and the folder became `workacceptance` (the generator could not use its temp folder there). The
  misnamed folder (66 MB, untracked, made by that run) is still in the worktree: deleting it was denied; it is
  not committed. Rerun with `-WorkDir C:/wt/chicago-crime-canopy-db-lote/work/acceptance -SkipBuild` on
  `bbdf250`: **PASS, all 106 checks (36 criteria)**, including AC-B1, AC-B2, AC-B4, AC-B5, AC-B8 (real Ctrl+C:
  exit 130, `first.csv` complete, no `second.csv`, no temporary file, no final line) and AC-B10 (91 earlier rows
  unchanged). Report `work/acceptance/query-acceptance-report.md`.
- **L7** RDD review 1 — commits `f8f60b6..bbdf250` (base `18ee4ba`), lenses resilience, readability,
  reliability; lineage `review-1f8103dcc942fb9b`. The first bound STATUS timed out while the acceptance run
  loaded the machine; retried after it. Result: **approved**, acknowledged (authority burned). Findings, all
  informational: R2-001 SUGGESTION `main.rs` exit-code table incomplete → fixed in `1c4dd7e`; R2-002 WARNING and
  R3-001 WARNING `run-query-acceptance.ps1` `Get-UnlimitedIds` only knows the THEFT and CRIM limited queries, a
  hidden coupling with AC-B1 → documented at both places in `1c4dd7e`; R2-003 SUGGESTION `split_words` exported
  without callers → made private in `1c4dd7e`; R4-001 SUGGESTION duplicate-output check is quadratic in the
  number of `--output` lines → not fixed (batch size limits are out of scope; negligible for realistic batches).
- **L8** RDD review 2 (last slice) — commits `1c4dd7e..4828ba8` (base `bbdf250`; assess: medium, under budget),
  lenses readability, reliability; lineage `review-7fc125509a265fd3`. Result: **approved**, acknowledged. Findings,
  all informational, none fixed: R2-001 SUGGESTION the C6 annotation names only code 2 for batches (the PRD's
  text for that annotation; code 1 cases are in the batch PRD); R2-002 SUGGESTION the acceptance comment lists
  today's limited queries and may go stale; R2-003 SUGGESTION no note on why `split_words` became private;
  R3-001 WARNING removing the export could break an outside caller → none exists: the whole workspace builds
  and its 474 tests pass.
- **L9** Final checks on `4828ba8`'s code (`1c4dd7e`): `cargo fmt --all --check` clean, `cargo clippy --workspace
  --all-targets -- -D warnings` clean, `cargo test --workspace` 474 passed, 0 failed; acceptance script parse
  check 0 errors. Scratch to remove by the user: `/tmp/tmp.FONUejTUYE` (base build), `/tmp/tmp.Puql0Ni8ny`
  (verifier), `/tmp/tmp.apiJ8IwbTO` (T3 smoke), `/tmp/rdd*` files, and `workacceptance/` in the worktree.

## Closing

Closing report: [canopy-db-lote-report.md](canopy-db-lote-report.md).
