# crime-generator — ODD feature document

- **Locator**: `odd/tasks/crime-generator.md` · Engram mirror topic `odd/crime-generator/tasks`
- **Product source**: `docs/prd/chicago-crime/generador.prd.md` (child, v1.0) and its parent
  `docs/prd/chicago-crime/chicago-crime.prd.md` (v1.2). The PRDs win on any product question.
- **Branch**: `feat/crime-generator`, branched from `main` at `e2e69f6` (baseline docs commit).

## Objective

Deliver the **generator**: a Rust CLI that reads the Chicago crime CSV once and publishes a dataset made of
a primary SSTable keyed by ID, five secondary-index SSTables and a manifest, under a 512 MiB peak of private
committed memory.

## Problem and why

The 2.3 GB CSV (8,656,904 records) only supports full scans. A sorted, immutable store enables point and
range lookups. The query app (next change) depends on this dataset, and the project is also a learning case
for Rust, external sorting and FoundationDB tuple encoding.

## Scope

Authorized: everything in the child PRD's flow, validations and AC-G1..AC-G10, plus the parent's flow A,
field rules, generator validations and decisions D1–D5.

Out of scope (from the PRD): the query app, an inspection mode in the generator, configurable index sets,
single-index regeneration, multiple input CSVs, non-Windows verification, installers.

## Constraints and current evidence

- Reference CSV (measured 2026-10-12): 8,656,904 records, 22 columns, IDs 634..14,353,993 with no
  duplicates and **not sorted**, Case Number not unique (632 repeats), 253 Date values in the nonexistent
  spring DST hour, 1,663 in the ambiguous fall hour.
- Measured tuple value size (without ID and Location): ~1,155 MiB, avg 140 B/record.
- Toolchain: Rust 1.99 / Cargo 1.99 on Windows 11 (32 threads, 64 GB RAM). No remote repository.
- Memory limit: peak private committed bytes of the process, including runtime and libraries, excluding the
  OS file cache. Exceeding it only logs a warning; it never stops the run.
- User-facing messages are in Spanish exactly as the PRD tables specify. Code, identifiers, comments and
  this document are in English.

## Implementation decisions

| Topic | Decision | Rationale |
|---|---|---|
| Layout | Cargo workspace: `crates/crime-dataset` (shared dataset contract library) and `crates/crime-generator` (binary). `crates/crime-query` is created in its own change. | Both apps share the contract; the compiler enforces that the query app cannot reach generator internals. Approved by the user on 2026-10-12. |
| Tuple encoding | `foundationdb-tuple` crate if it is pure Rust with no `foundationdb-sys`; otherwise a minimal in-house encoder following the tuple spec. | D1/D4: FDB tuple serialization only, not FoundationDB itself. |
| Dates | Civil local time stored as `i64` seconds since `1970-01-01T00:00:00` with no time zone (naive). | D3: no UTC conversion; DST-gap and ambiguous times stay valid and sortable. |
| Primary SSTable | Key `pack((id,))`; value `pack(...)` with the 21 typed fields in CSV order, ID first, Location excluded, `Nil` for empty optionals. | D4. |
| Index SSTables | One per index (date, primary type, district, community area, case number). Key `pack((value or Nil, id))`, empty value. | D2; `Nil` sorts first and keeps "sin valor" queryable. |
| SSTable writer | `sstable` crate (LevelDB table format), Snappy by default, optional none. Keys are validated as strictly increasing **before** every `add`. | D1/G2. The crate only checks order at block boundaries and panics instead of returning an error. |
| Sorting | External sort by `(id, csv_line)` with a configurable segment size (start at 250,000) and temp dir. | Bounded memory; the line tiebreak makes "first occurrence in the file" deterministic. |
| Duplicates | Detected in the merged sorted stream by comparing each ID with the previous one: keep the first, log the rest. | D5. Duplicates are adjacent after the merge no matter how far apart they were; per-segment checks would miss cross-segment duplicates. |
| Index build | Index entries come from the deduplicated primary stream into a second external sort ordered by `(index, value, id)`. | Indexes contain exactly the accepted records. |
| Source identity | SHA-256 of the CSV computed while streaming it. | G4; the operator can cross-check it with `Get-FileHash`. |
| Determinism | No timestamps or run-dependent data inside SSTables; total order in every sort. | G5. |
| Verification independence | The acceptance verifier reads the tables with `sstable` + `foundationdb-tuple` + `csv` only, without generator or `crime-dataset` code. | Child PRD "Verificación sin consultor". |

## Delivery

- **Strategy**: local, no pull requests (no remote; the user's decision on 2026-10-12). Each task closes with a
  work-unit commit on `feat/crime-generator`. After the full verification, merge into `main`.
- **Forecast**: ~2,900 authored changed lines over 7 tasks. The ~400 lines per task figure is a planning
  heuristic only.
- **RDD**: on (global). After each work-unit commit, run `gentle-ai review assess --committed-only` against the
  last reviewed boundary (first boundary `e2e69f6`) and follow `review_due`.

## Tasks

Applicable checks for every code task: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Test-first: observe RED on a
relevant deterministic test before implementing GREEN.

- [x] **T1 — Workspace and dataset contract** (`crime-dataset`). Route: delegated writer (2+ non-trivial files).
  Typed `CrimeRecord`, expected header, naive civil datetime codec (parse and format
  `MM/DD/YYYY hh:mm:ss AM|PM`), primary key/value tuple layout and decoding, the five index definitions and
  key layout, Location reconstruction, manifest model with schema version 1.
  Tests: round-trips, key byte order equals numeric ID order, `Nil` sorts first, leading zeros preserved,
  DST-gap date accepted, invalid dates rejected.
- [x] **T2 — CSV ingestion, validation and run log** (`crime-generator`). Route: delegated writer.
  CLI (input, output, temp dir, log, overwrite, no-compression), preflight checks, header validation, row
  validation into a record or a rejection (line, column, reason, raw content), per-run log file (default next
  to the output folder) mirrored to the console, SHA-256 while streaming.
  Tests: one fixture per rejection type, header mismatch, each preflight failure.
- [x] **T2a — Fix from the T2 review: the run log never destroys data**. Route: delegated writer. Added after RDD
  advisory `R3-log-truncates-arbitrary-path`: `--log` pointing at the input CSV truncated the source. The log
  must be created exclusively and refuse an existing file. Also fold in `R3-multiline-raw-breaks-log-lines`
  (keep each rejection's raw content on one physical log line) and `R3-midstream-io-and-log-failure-untested`
  (test a mid-stream read error).
- [x] **T3 — External sort, duplicates and primary SSTable**. Route: delegated writer.
  Sort by `(id, line)`, duplicate detection in the merged stream, primary SSTable writer with Snappy or none
  and pre-`add` order validation.
  Tests: unsorted input comes out sorted; duplicates across segments (tiny segment size) keep the first and log
  the rest; independent read-back.
- [x] **T4 — Secondary index SSTables**. Route: delegated writer.
  Index entries from the deduplicated stream, second external sort, one SSTable per index, entry counts.
  Tests: entries per index equal valid records; IDs ascending within a value; `Nil` for empty optionals; a
  shared case number lists every ID.
- [x] **T5 — Atomic publication, manifest, overwrite and leftovers**. Route: delegated writer.
  Staging and atomic switch (mechanism chosen in the task), previous dataset intact on any failure, overwrite
  option, leftover cleanup with message, manifest JSON, empty-CSV warning.
  Tests: refuse without overwrite; replace with overwrite; simulated failure keeps the previous dataset;
  leftovers cleaned; empty CSV.
  Also from the T2a+T3 review (advisory, accepted): test that a failure *after* staging exists leaves no
  staging folder and no published table (`R3-staging-cleanup-on-failure-untested`); explicitly flush and
  sync each table before the switch so a full disk at the last write is reported
  (`R3-table-not-synced-before-rename`); test the `TableWriter` I/O-failure path with a failing writer
  (`R3-table-io-failure-path-untested`); test that a corrupt segment mid-merge yields one error and then the
  end of the stream (`R3-merge-decode-error-untested`).
  Also from the T4 review (advisory, accepted): an abort while both sorts are alive removes both temp folders
  and the staging folder (`R3-index-sort-cleanup-on-abort-untested`); unit-test the two ordinal guards in
  `write_indexes` (`R3-index-stream-order-guard-untested`).
- [x] **T5a — Fix from the T5 review: never delete user folders that look like versions**. Route: delegated writer.
  RDD advisory `R3-version-prefix-folders-deleted-as-leftovers` (WARNING): any folder passing
  `is_version_dir_name` (prefix + letters/digits/dashes, e.g. `dataset-backup`) and not named by `CURRENT`
  was deleted as a leftover. Tighten the contract to the exact generated shape and treat anything else as
  foreign. Fold in `R3-leftovers-message-ignores-removal-failure`: log the discard message only for
  leftovers actually removed, and warn about the ones that could not be removed.
- [x] **T6 — Cancellation, memory, summary and exit codes**. Route: delegated writer.
  Ctrl+C as failure, exit codes (0 completed even with rejections or memory warning, non-zero on abort or
  cancel), peak private committed memory on Windows with a warning above 512 MiB, final summary (valid,
  rejected, duration, peak memory, file sizes, log path).
  Tests: exit codes through the binary, cancel path, summary content.
- [x] **T7 — Acceptance on the reference CSV**. Route: delegated writer plus a verification run.
  Independent verifier for AC-G1, AC-G3 and AC-G4; full-run memory (AC-G2); determinism (AC-G10); kill
  mid-run (AC-G6). Tune segment sizes if memory exceeds the limit. Record timings and sizes.

## Acceptance criteria

AC-G1..AC-G10 of the child PRD. Mapping: AC-G5/G8/G9 → T2, T5, T6 tests; AC-G7 → T6; AC-G1..G4, G6 and G10
→ T7 on the reference CSV.

## Progress and evidence

| Task | Commit | Checks | RDD | Notes |
|---|---|---|---|---|
| T1 | `ab6e21d` (+ `c6eba90` chore: project PRD skill, user-approved so the RDD candidate has no untracked files) | fmt ok · clippy -D warnings ok · test 27/27 (RED 27/27 `todo!()` first; parent re-ran 27/27) | medium → granted → **approved**, acknowledged (lineage `review-b037a5d15672c977`, 1 lens: reliability). Reviewed boundary → `c6eba90`. | Delegated writer. `foundationdb-tuple` 0.11.0 with `default-features = false` depends only on `memchr` (pure Rust). ~1,217 lines (626 code, 566 tests). Files: `primary.sst`, `index-<name>.sst`, `manifest.json`. |
| T2 | `6806f4c` | fmt ok · clippy ok · test 77/77 (RED 46/46 `todo!()`; GREEN 45/46 then a CRLF line-number fix; parent re-ran 77/77) · full CSV: 8,656,904 valid / 0 rejected, 13.5 s, SHA-256 `5cb5fa41…ae35d` over 2,406,401,982 bytes (matches `sha256sum`) | medium → granted → **approved**, acknowledged (lineage `review-1d20f6160cc09491`). Reviewed boundary → `6806f4c`. Three advisory findings were turned into T2a. | Delegated writer. ~1,060 code + ~1,150 tests. The csv crate's `position().line()` lags after CRLF and blank lines, so the start line is derived from the record end. BOM stripped before parsing. Rejection log line: `<mensaje> Contenido original: <fila CSV>`. |
| T2a | `97e7573` | fmt ok · clippy ok · test 82/82 (RED: `--log`=`--input` truncated the CSV, an existing log was overwritten, a multi-line entry spanned several lines; the mid-stream read error is now pinned by a test; parent re-ran 82/82) | `under_budget` (213 lines) at its own commit; reviewed inside the T2a+T3 slice (see T3). | Delegated writer. The log is opened with `create_new`, so it refuses any existing file. `RunLog::line` escapes CR/LF in one place. Known edges: `--log`=`--input` with a missing input reports a header mismatch; two runs in the same second with the default log name make the second one abort, which is safe. |
| T3 | `2397151` | fmt ok · clippy ok · test 116/116 (RED 31; first GREEN failed 2 tests because of the extsort data-loss bug; parent re-ran 116/116) · full CSV: 8,656,904 valid / 0 rejected, 43.1 s, `primary.sst` 865 MiB (Snappy), 35 segments, **peak private commit 86.7 MiB**, no leftovers; an independent reader confirmed 8,656,904 strictly increasing IDs 634..14,353,993 | T2a+T3 slice (base `6806f4c`, 2,198 lines) medium → granted (standing user instruction) → **approved**, acknowledged (lineage `review-448cbe25e4dff735`). Reviewed boundary → `2397151`. Four advisory findings were moved into T5. | Delegated writer. **extsort 0.5.0 rejected**: it returns an empty stream when the item count fills the last segment exactly, and it writes or truncates segments named `0`, `1`, … without deleting them. Replaced by an in-house k-way merge sort with stable ties and a per-sort temp folder cleaned on drop. sstable quirks handled: `add` panics on out-of-order keys (`TableWriter` validates first); short writes are ignored (`WriteAll` adapter); `Status` Display recurses forever; a builder left broken after an I/O error. Bloom filter off. ~920 code + ~1,020 tests. |
| T4 | `e93bd93` | fmt ok · clippy ok · test 136/136 (RED 24; GREEN first pass; parent re-ran 136/136) · full CSV: exit 0, 73.8 s, **peak private commit 93.4 MiB**, 35 + 44 segments, no leftovers. Index sizes: date 54.6 MB, primary_type 28.3 MB, district 28.7 MB, community_area 31.4 MB, case_number 51.7 MB. An independent reader rebuilt every expected index key from `primary.sst`: each of the 5 indexes has 8,656,904 entries with 0 mismatches. **AC-G4 evidence**: `THEFT` = 1,838,502 ✓, community area Nil = 613,720 ✓, district Nil = 47, 528 shared case numbers cover 1,160 IDs (632 repeats ✓). | medium (1,137 lines) → granted (standing instruction) → **approved**, acknowledged (lineage `review-0f5b265234c43ecc`). Reviewed boundary → `e93bd93`. Two advisory test gaps were moved into T5. | Delegated writer. `IndexEntry { ordinal, key }` sorted by (ordinal, key bytes); default index segment 1,000,000 (hidden `--index-segment-size`). The entries-per-index = valid invariant is checked before publish. Cost: about +31 s from decoding values and sorting 43.3M index entries. ~345 code + ~780 tests. |
| T5 | `f2619e3` | fmt ok · clippy ok · test 165/165 (RED 48; GREEN first pass; publish/binary/primary/indexes suites 5× repeated green; parent re-ran 165/165 + release build) · full CSV: (a) first run exit 0, 63.5 s, peak 92.7 MiB, manifest with SHA-256 `5cb5fa41…ae35d`, 8,656,904 valid / 0 rejected, sizes match T4; (b) rerun without `--overwrite` refused in 0.1 s, every file byte-identical; (c) `--overwrite` exit 0, 64.2 s, peak 93.1 MiB, new `CURRENT`, old version removed, **all six tables byte-identical to (a)** (early AC-G10 evidence) | medium (2,162 lines) → granted (standing instruction) → **approved**, acknowledged (lineage `review-e7d4f65ffbdc6eb6`). Reviewed boundary → `f2619e3`. The WARNING became T5a; the leftover-message suggestion was folded into T5a; the non-Windows lock suggestion is out of scope (Windows only, per the PRD). | Delegated writer. Layout: `<output>/CURRENT` names `dataset-<YYYYMMDD-HHMMSS>-<pid>`; the contract lives in `crime-dataset` (`resolve_current`). Exclusive `.generator.lock` (share_mode 0 + delete-on-close, released even if the process is killed). Foreign content is refused; leftovers are cleaned under the lock. Tables and manifest are synced, then `CURRENT.tmp` is renamed over `CURRENT`. The `fail-points` feature is enabled only for this crate's tests (self dev-dependency). All six advisory items absorbed. ~830 code + ~1,240 tests. |
| T5a | `f71d7c6` | fmt ok · clippy ok · test 170/170 (RED 6: `dataset-backup` accepted as a version, user folders deleted, discard message logged on failed removal, missing warning; parent re-ran 170/170) | see Notes | Delegated writer. `is_version_dir_name` accepts only `dataset-<8 digits>-<6 digits>-<pid>[-<n>]`; anything else is foreign and never deleted. `version_dir_name` is the single naming function, pinned against the contract. The discard message is logged only for removed leftovers, with a warning for each kept one (fail point `LeftoverRemoval`). ~112 code + ~289 tests. RDD: medium (410 lines) → granted → **approved**, acknowledged (lineage `review-8281dd41bd338b31`), boundary → `f71d7c6`; suggestion `R3-attempt-zero-accepted` folded into T6. |
| T6 | `01fd4b5` | fmt ok · clippy ok · test 191/191 (RED 21; GREEN first pass; cancel/binary/publish suites 5× green; parent re-ran 191/191) · full CSV `--overwrite`: exit 0, 63.3 s, program peak 93,1 MiB = external `PeakPagedMemorySize64` 93.04 MiB, all six tables byte-identical to T5 · real Ctrl+C (GenerateConsoleCtrlEvent in its own console) at 25 s (primary merge) and 5 s (rows): exit 130, exact operator message, temp empty, output byte-identical | medium (1,401 lines) → granted → **approved**, acknowledged (lineage `review-2b9d8dac0c6db48b`). Boundary → `01fd4b5`. Suggestion (automated binary-level Ctrl+C) covered by the T7 acceptance script (AC-G7). | Delegated writer. `ctrlc` 3.5.2 (Windows: only windows-sys) sets a `CancelFlag` checked per row, per merge entry and before the switch. Exit codes: 0 ok (even with rejections or memory warning), 1 abort, 2 usage, 130 cancelled. Peak from `GetProcessMemoryInfo.PeakPagefileUsage` at phase boundaries; one warning above the limit (hidden `--memory-limit-mib`). Spanish final summary with counts, duration, peak, dataset folder, the 7 file sizes and the log path. Layout pid/attempt are `[1-9][0-9]*`. ~600 code + ~730 tests. |
| T7 | `46de4f0` (A: stale sort folders) + B1 verifier + B2 script/README (hashes in the closing commit) | fmt ok · clippy ok · test 228/228 (A: RED 9 → GREEN; verifier helpers RED 15 + 9 integration → GREEN; parent re-ran 228/228) · **acceptance script on the reference CSV: exit 0, 14/14 PASS in 367 s** (report: scratchpad `acceptance/acceptance-report.md`, summarized below) | T7 slice assessed after B2 (see the closing row) | Delegated writer. A: sort folders `crime-generator-sort-<pid>-<rand>`; folders of dead pids (OpenProcess + STILL_ACTIVE) are removed at startup, only after the output lock. B: `crates/crime-acceptance` depends only on csv, foundationdb-tuple, serde_json, sha2, sstable, with a hand-written date formatter and no crime-dataset or crime-generator in [dependencies]. `scripts/acceptance/run-acceptance.ps1` (UTF-8 BOM for PS 5.1) + `send-ctrl-c.ps1`. README at the root. |

Notes carried to T2 (done in T2): reject non-finite Latitude/Longitude (`NaN`/`inf` parse as `f64`); map empty optional fields to `None` (the decoder accepts `Some("")`).

Advisory RDD findings (non-blocking, later work):
- `R3-manifest-schema-version-unchecked` (WARNING): `Manifest::from_json` accepts any `schema_version`. The query app must reject an unknown version; it is tracked for the query change.
- `R3-decoder-accepts-empty-and-nonfinite` (SUGGESTION): the decoder accepts `Some("")` and non-finite doubles. The writer side is fixed in T2; tightening the decoder side is tracked for the query change.

## Acceptance results (reference CSV, 2026-10-12, T7 script)

| Criterion | Result | Evidence |
|---|---|---|
| AC-G1 | PASS | `primary.sst` 8,656,904 entries, strictly increasing IDs 634..14,353,993 = manifest valid = CSV rows − 0 rejected |
| AC-G2 | PASS | Run A peak private commit 93.2 MiB (program) / 93.1 MiB external; Run B 93.0 MiB; limit 512 MiB; no warning |
| AC-G3 | PASS | **all** 8,656,904 CSV rows compared field by field with `primary.sst`: 0 mismatches; parent AC3 sample (first, last, no coordinates, no Ward) equal; 253 DST-gap rows stored exactly as published (parent AC12) |
| AC-G4 | PASS | each of the 5 indexes equal entry by entry to the keys rebuilt **from the CSV** (8,656,904 each); THEFT 1,838,502; community area Nil 613,720; 528 shared case numbers cover 1,160 IDs |
| AC-G5 | PASS | header altered → abort (parent AC9); 21-column row + invalid date + repeated ID → 999 valid / 3 rejected, each logged (parent AC13) |
| AC-G6 | PASS | hard kill at 25 s during the merge: `CURRENT` and the previous version byte-identical; the next run discards the stale sort folders and the leftover version and completes |
| AC-G7 | PASS | real Ctrl+C at 25 s: exit 130 in 0.36 s, exact operator message, temp folder empty, dataset byte-identical |
| AC-G8 | PASS | rerun without `--overwrite` refused in 0.1 s, nothing changed; with `--overwrite` `CURRENT` switched, old version removed |
| AC-G9 | PASS | the aborted run's log exists and explains the cause |
| AC-G10 | PASS | Run A and Run B: the six tables are byte-identical (SHA-256 per file) |
| Manifest | PASS | SHA-256 `5cb5fa41…ae35d` and size computed independently; counts and file sizes match the tables |

Timing: full generation 72–75 s on the reference machine (63 s in T5/T6; not investigated, likely machine load, the memory peak is unchanged). Sizes: `primary.sst` 865 MiB, indexes 27–52 MiB each.

Known limits (accepted, documented): the verifier compares numbers by exact text (the reference CSV has no redundant zeros); stale sort folders are kept when their pid was reused by a live process; non-Windows behavior is out of scope.

## Next step

Close: assess the T7 slice, then merge into `main` (standing user instruction (2026-10-12): finish without asking, always grant RDD review, merge into `main` after full verification.
