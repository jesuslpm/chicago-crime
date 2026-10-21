# canopy-db-generador — ODD execution

Objective: implement `docs/prd/chicago-crime/canopy-db-generador.prd.md` (v1.0) with its ancestry
(`canopy-db.prd.md` → `chicago-crime.prd.md`, and the K1 contract `generador.prd.md`, `soporte-linux.prd.md`,
`aceptacion-cualquier-csv.prd.md`). Branch `feat/canopy-db-generador` (worktree copy; close without merging).
Delivery: work-unit commits on the branch, RDD per commit slice. Test runner: `cargo test --workspace`
(+ `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`); acceptance:
`scripts/acceptance/run-canopy-acceptance.ps1`. Estimate: 4–7 h; start 2026-10-21 08:18:02 +0100.

## Specs

The PRD is the specification: every `S#` is the PRD acceptance criterion of the same name, with its text, messages
and examples exactly as written in the PRD (read the PRD until "Decisiones", and its sections "Validaciones y
mensajes" and "Salida", before any edit). Workers quote messages from the PRD, never from this summary.

- S1 = AC-KG1 (AC-M1, generator part) · S2 = AC-M3 · S3 = AC-M4 · S4 = AC-KG2 · S5 = AC-KG3 · S6 = AC-KG4
- S7 = AC-KG5 · S8 = AC-KG6 · S9 = AC-KG7 · S10 = AC-KG8 · S11 = AC-KG9 · S12 = AC-KG10 · S13 = AC-KG11
- S14 = AC-KG12 · S15 = AC-KG13 · S16 = AC-KG14
- S17 = "Anotaciones al cerrar" (generador.prd.md and its translation, G5 and AC-G2).

Implementation decisions (PRD "Queda para la implementación"), fixed by the spike of L3:

- One CanopyDB environment in `canopydb/`, one database `crime`, six trees named as the manifest `tables`
  (`primary`, `date`, `primary_type`, `district`, `community_area`, `case_number`), same keys and values as the
  SSTables. The generator creates `canopydb/` itself (CanopyDB does not create its folder).
- Engine options: `DbOptions.use_wal = false` (bulk load; no WAL file left), page cache 64 MiB,
  `write_txn_memory_limit` and `checkpoint_target_size` 32 MiB, throttle/stall 128 MiB, one commit per
  100 000 entries, `checkpoint()` at the end, then close. `wal_write_batch_tempfile_dir` = the run's sort folder.
- Compression: tree option `compress_overflow_values` = CanopyDB default (`Some(12 KiB)`) → manifest `lz4`;
  `--no-compression` → `None` → `none`.
- Readers (verifier) open with the same 64 MiB page cache: CanopyDB's default 1 GiB cache gave a 1,161 MiB peak.
- CanopyDB version 0.2.5 (`=0.2.5` in the workspace).

## Tasks

| ID | Specs | Route | Status | Commit / evidence |
|---|---|---|---|---|
| T1 | S13 (KG11) | inline | done | `7b61063` refactor(generator): `Storage` trait (`SstStorage`), `App` identity, sort prefix, manifest JSON to `publish`. crime-generator tests 169 → 170 (+ golden help), clippy/fmt ok; differential vs base (help, usage errors, summary, tables SHA-256, manifest): identical. RDD assess medium, `under_budget` (pending in slice) |
| T2 | S1, S4, S5, S7, S8, S10, S14, S16 | inline | done | `094af06`: 18 tests (layout, manifest order/values, files = disk = summary, same keys/values as SSTables, two runs and both compressions equal, header-only CSV, re-read adds no file, original query refuses, cross-engine overwrite both ways, orphans of both engines with one message, own stale sort folders only, shared output lock, full help, Cargo.lock version, cancellation x3, first run removes output). Tests written after the code except the cancellation ones (RED only from test defects); smoke run on 20 000 rows: files LOCK + DATA + OPTIONS, no WAL |
| T3 | S2, S6, S11 | delegated (Writer trigger (a): independent unit, disjoint surface, parallel with T1–T2) | done | `0d25696`: crime-canopy-acceptance (CSV mode, `--compare`), crime-acceptance shares its checks through a `Tables` trait; 26 tests (RED observed for 22 with stubs); crime-acceptance 68 → 68 tests, identical `--list`; writer's differential of the original verifier (6 cases) identical. Writer deviation accepted: reads the stored OPTIONS and passes them back so OPTIONS stays byte-identical |
| T4 | S9 | inline | pending | feat: engine errors (KG7): messages, detail line, panic, temp files |
| T5 | S12 | inline | pending | docs: findings register `docs/canopy-db/hallazgos.md` + H2/H3 reproductions |
| T6 | S1, S2, S3, S6, S15 | inline | pending | feat(acceptance): `run-canopy-acceptance.ps1` |
| T7 | S14 | inline | pending | docs: README, Linux guides |
| T8 | S1, S3, S13 | inline | pending | acceptance runs (Windows), Linux tests and manual AC-M4 |
| T9 | S17 | inline | pending | PRD annotations, closing report |

## Log

- L1 (2026-10-21, user): "Implementa docs\prd\chicago-crime\canopy-db-generador.prd.md con ODD"
- L2 (2026-10-21 08:17:34): estimate 4–7 h sent; user confirmed ("Adelante"); start 08:18:02 +0100.
- L3 (spike, scratchpad, not in the repo): CanopyDB 0.2.5 release build, 8,656,904 primary entries of 138 bytes
  plus 5 × 8,656,904 index entries, 100 000 per commit, cache 64 MiB, no WAL: 66.6 s, external peak 222.4 MiB,
  files `LOCK` (0) + `crime_<id>_l/DATA` (3,284,643,840) + `crime_<id>_l/OPTIONS` (300). Reading every tree with
  the same options: 7.6 s, 73.4 MiB, no file added. Reading with `Environment::new` defaults: 7.9 s,
  **1,161.3 MiB** (H4 confirmed), no file added but `OPTIONS` rewritten (300 → 299 bytes: the stored DbOptions
  are replaced by the opener's). `Environment::with_options` fails with NotFound when the folder is missing.
- L4 (T1, 08:37): differential check `work/differential/run.sh` (base archive vs working copy of crime-generator, 20 000-row CSV): same help, no-args and unknown-option errors, summary (normalized), six tables byte-identical, manifest identical except `generated_at`. Parallel hazard seen: the T3 writer's crate with a Cargo.toml and no target made the whole workspace unloadable; the writer was asked to keep it loadable.
- L5 (T2/T3 seam): `cargo fmt --all --check` ok, `cargo clippy --workspace --all-targets -- -D warnings` ok, `cargo test --workspace` 498 passed, 0 failed. Commits `094af06` (T2), `0d25696` (T3), `bfa31d5` (execution docs, to clear the untracked inventory RDD requires).
- L6 (RDD): assess 18ee4ba..HEAD committed-only = high (`process_boundary` in the T3 test support), 6,033 lines; START with four lenses refused `lens_context_budget_exceeded` (lineage review-6a7e04accdebec5f, no authority created). Reviewing commit by commit instead, with HEAD detached at each commit (assess/status only take base..HEAD).
