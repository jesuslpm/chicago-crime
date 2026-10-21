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
| T4 | S9 | inline | done | `17ae3da`: `Abort::WithDetail` (detail line logged before the cause), storage closed before the version folder on early return (RDD 1 advisories), engine faults injected by feature `fail-points` (disk full, write failure, internal, panic, multi-line): 5 tests, RED = did not compile before the fault module; compression test (256 KiB value smaller with LZ4), mutation check: fails when the option is ignored. Temp-file case not applicable (L9). `873a29b` locks the self dev-dependency |
| T5 | S12 | inline | done | `0d9b159`: register (summary + one section per finding, H1–H4 dated 2026-10-20, Detectado, `—`), `docs/canopy-db/repro` standalone (`[workspace]`, canopydb =0.2.5): `cargo run -- h2` → B fails after 5.0 s with EnvironmentLocked, REPRODUCED; `cargo run -- h3` → read-only base PermissionDenied, WAL 1,048,576 → 34,121 bytes on a read-only open, REPRODUCED |
| T6 | S1, S2, S3, S6, S15 | delegated (Writer trigger (b): long-session context backstop; parallel with T7) | done | `78790d8`: 869 lines; writer checks: parse 0 errors, the four validation errors exact (exit 1, nothing run), 200k-row CSV + one DST row: PASS 17/17 in 20.5 s, NOT MET branch proven in a throwaway copy with a 100 MiB limit (PASS 16/17 + 1 not met; without the association FAIL). Parent spot check: parse 0, `AC-G1=H1` → exact error, exit 1. Writer bug fixed before delivery (`$runs` = `$RunS`, case-insensitive) left an untracked folder `System.Collections.Specialized.OrderedDictionary/` at the root (its delete was denied to the writer; left for the user) |
| T7 | S14 | inline | done | `a25e60a`: README section, status row, verifier usage, layout rows; Linux guides (command + manual AC-M4). Verify section of the script added after T8 |
| T8 | S1, S3, S13 | inline | done | Windows: `run-canopy-acceptance.ps1` on the reference extract PASS 17/17 (L15); verifier alone PASS 19 (L13); Linux: 512 tests, AC-M4 212.2 MiB (L12) |
| T9 | S17 | inline | done | `27d7b1d` annotations of `generador.prd.md` and its translation (G5, AC-G2); closing report [canopy-db-generador-report.md](canopy-db-generador-report.md) |

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
- L7 (RDD review 1, slice `5ded28e..094af06` = T1+T2, base boundary after the passive PRD commit `5ded28e`, which assessed `passive`): high (`process_boundary`), 2,455 lines, lenses risk, resilience, readability, reliability, consent granted (project rule). Result **approved**, acknowledged, lineage `review-8455fdd65dc89407`. No blocking finding. Advisories (WARNING unless noted): R4-storage-drop-order and R3-drop-order-open-base (storage dropped after the version folder on an early return) → fixed in T4; R4-engine-error-cause-lost and R2-002 (engine cause discarded) → fixed in T4 (KG7 detail line); R3-compression-unproved → test added in T4; R2-001 (unordered-keys abort names an `.sst` file for CanopyDB) → accepted, internal defect message; R2-003 (Cargo.lock lists the T3 crate) → artifact of the parallel writer, consistent after `0d25696`; R2-004 SUGGESTION (Built/RunSummary duplication) → accepted; R3-close-errors-swallowed SUGGESTION (close errors invisible on drop) → accepted, CanopyDB 0.2.5 reports none. Boundary → `094af06`.
- L8 (RDD review 2, slice `094af06..b04477c` = T3 + execution docs): high (`process_boundary`), 2,528 lines, four lenses, consent granted. **Approved**, acknowledged, lineage `review-cb918a1f9303d9a1`. No blocking finding. Advisories: R1-001, R4-stored-options-silent-fallback, R3-options-unproved (WARNING: the verifier falls back to its own options when OPTIONS is unreadable, and would rewrite it) → fixed in T3b; R3-reread-names-only (WARNING: re-read test checks names only) → T3b; R2-001 WARNING, R2-003 SUGGESTION (table list repeated) → T3b; R2-002 SUGGESTION (doc of FIELDS) → T3b; R4-compare-same-base-lock SUGGESTION (same base on both sides waits 5 s) → T3b; R3-verify-open-failure-untested SUGGESTION → T3b. Boundary → `b04477c`.
- L9 (T4 finding): CanopyDB 0.2.5 writes temporary files only for the write batch of its write-ahead log (`write_batch.rs`); with `use_wal = false` it creates none, so the PRD's "error del motor en sus archivos temporales" case cannot occur with this configuration. Recorded as not applicable for AC-KG7 (the PRD row says "si los usa").
- L10 (T3b): `1228833` fixes the RDD 2 advisories: no fallback options (unreadable OPTIONS → the base is not opened), same dataset on both sides read once, TABLES from the original verifier's constants, FIELDS doc; tests: unreadable options (RED), same dataset (RED), OPTIONS byte-identical after a CSV-mode read, CSV mode with the base held (FAIL AC-G1 after LAYOUT, MANIFEST-SCHEMA, MANIFEST-FILES). 504 → workspace tests green.
- L11 (T8, full run, Windows, release): `crime-canopy-generator` on the reference CSV: 8,656,904 valid, 0 rejected, duration 2 min 41.2 s, summary peak 216.8 MiB (limit 512), files `canopydb/LOCK` 0, `crime_<id>_l/DATA` 2,718,945,280, `OPTIONS` 300, `manifest.json` 1,149. Linux: branch bundled into WSL Ubuntu 24.04 (`~/canopy-db-generador`, fresh clone; the existing `~/chicago-crime` untouched).
- L12 (T8, Linux, WSL Ubuntu 24.04.1, commit `a25e60a`): `cargo test --workspace` 512 passed, 0 failed (53 test binaries). AC-M4 by hand on the reference CSV: 8,656,904 valid, 0 rejected, 1 min 54.8 s, summary peak (VmHWM) 212.2 MiB ≤ 512 MiB; the output folder holds only `CURRENT` and the version folder (`canopydb/`, `manifest.json`), no lock file (AC-L3).
- L13 (T8, Windows): `crime-canopy-acceptance --csv chicago-crime-data.csv --dataset work/full1/out --reference` on the full run of L11: RESULT: PASS (19 checks) in 2 min 35.9 s — LAYOUT, MANIFEST-SCHEMA, MANIFEST-FILES, CSV, MANIFEST-SOURCE, MANIFEST-TABLES, AC-G1 (8,656,904), AC-G3 (0 mismatches), AC-G3-SAMPLE, AC-G3-DST (253), AC-G4 ×5, AC-G4-THEFT (1,838,502), AC-G4-COMMUNITY-NIL (613,720), AC-G4-SHARED-CASES (632 repeats), AC-C18-LOWERCASE.
- L14 (independent verify, sonnet, HEAD `27d7b1d`, scratch archive of HEAD and of `18ee4ba`): no blockers. S2, S4–S8, S10–S14, S16, S17 PASS (most by probe); S1, S3, S15 PASS on tests/code, full-CSV rows left to the acceptance run. Workspace tests in scratch: 508 passed, 0 failed (53 binaries). Differential of the original generator, verifier and query app between `18ee4ba` and HEAD: identical (help, usage errors, summary normalized, six `.sst` SHA-256, manifest minus `generated_at`, `crime-acceptance` report). Advisories: (1) AC-KG7 temp-file case not reachable (L9) → stated in the closing report; (2) AC-M4 NOT MET also needs the external sample → kept: a missing measurement is a failure of the script, not of CanopyDB, so FAIL is right; (3) two runs in the same second collide on the default log name → baseline behavior; (4) summary goes to stderr → baseline; (5) Linux count differs (Linux-only tests) → expected.
- L15 (T8, acceptance, Windows, commit `78790d8` + uncommitted execution docs): `run-canopy-acceptance.ps1` on the reference extract, 09:06:07 → 09:35:11 (1,744.4 s): **PASS: all 17 checks passed**. AC-M4 peak 219.5 MiB in the summary and 219.5 MiB external, Run A 101.0 s; AC-G1/G3/G4/Manifest/AC-C18 PASS (reference figures); AC-KG4 PASS; AC-M3 PASS (Run B `--no-compression` 330.2 s, 8,656,904 equal entries per table); AC-M3 (SSTables) PASS (Run S 198.6 s); AC-G8 refusal/replacement, AC-G9, AC-G5 (AC9, AC13), AC-G7 (exit 130 after 0.24 s), AC-G6 kill + next run PASS. My bash quoting turned `-WorkDir work\acceptance` into `workacceptance` at the worktree root; moved afterwards to `work/acceptance` (8.7 GB at the end; bases 2.6 GB each, so the script help now says 2.6 GB measured and a ~14 GB peak under the 18 GB it requires).
- L16 (RDD review 3, last slice `b04477c..01a6e7e` = T4, T3b, lock, T5, T7, T6, annotations, docs): assess medium (`under_budget` at `78790d8`), reviewed as the last slice per the project's closing rule; lenses risk, resilience, readability, reliability; untracked stray folder excluded (`--untracked-scope=exclude`); consent granted. **Approved**, acknowledged, lineage `review-7b945d672441c0cb`. No blocking finding. Advisories, all accepted (no change after review): R3-panic-hook-stderr WARNING and R4-001 SUGGESTION (Rust's panic hook also prints the panic to stderr; K9 allows the detail on the log or stderr, and before the change the process crashed with 101); R2-001 (TABLES built by index from INDEXES), R2-002/R2-003 (test helpers' doc and duplicated database-folder lookup), R2-004 (unused closure argument), R2-005 (K9 vs KG7 cited), R3-engine-faults-only-in-commit (faults injected in commit only; begin and finish share `guarded`), R3-wall-clock-assertion (same-dataset compare test bounded at 4 s against the 5 s lock wait). Boundary → `01a6e7e`.
- L17 (T8, original acceptance, AC-KG11): `run-acceptance.ps1 -WorkDir work/acceptance-original` on the reference extract, end 09:57:26: **PASS: all 15 checks passed**; AC-G2 peak 93.4 MiB; Run A took 251.5 s (65–75 s usually) because the four RDD reviewers ran on the same machine at the same time. Final checks after the last review: `cargo fmt --all --check` ok, `cargo clippy --workspace --all-targets -- -D warnings` ok, `cargo test --workspace` 508 passed, 0 failed.

## Closing

Closing report: [canopy-db-generador-report.md](canopy-db-generador-report.md). Active time 1 h 40 min against an
estimate of 4–7 h. Not merged into the base (worktree copy: the developer decides with `Fusiona el cambio en la
base`); nothing pushed.
