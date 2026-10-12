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
- [ ] **T4 — Secondary index SSTables**. Route: delegated writer.
  Index entries from the deduplicated stream, second external sort, one SSTable per index, entry counts.
  Tests: entries per index equal valid records; IDs ascending within a value; `Nil` for empty optionals; a
  shared case number lists every ID.
- [ ] **T5 — Atomic publication, manifest, overwrite and leftovers**. Route: delegated writer.
  Staging and atomic switch (mechanism chosen in the task), previous dataset intact on any failure, overwrite
  option, leftover cleanup with message, manifest JSON, empty-CSV warning.
  Tests: refuse without overwrite; replace with overwrite; simulated failure keeps the previous dataset;
  leftovers cleaned; empty CSV.
- [ ] **T6 — Cancellation, memory, summary and exit codes**. Route: delegated writer.
  Ctrl+C as failure, exit codes (0 completed even with rejections or memory warning, non-zero on abort or
  cancel), peak private committed memory on Windows with a warning above 512 MiB, final summary (valid,
  rejected, duration, peak memory, file sizes, log path).
  Tests: exit codes through the binary, cancel path, summary content.
- [ ] **T7 — Acceptance on the reference CSV**. Route: delegated writer plus a verification run.
  Independent verifier for AC-G1, AC-G3 and AC-G4; full-run memory (AC-G2); determinism (AC-G10); kill
  mid-run (AC-G6). Tune segment sizes if memory exceeds the limit. Record timings and sizes.

## Acceptance criteria

AC-G1..AC-G10 of the child PRD. Mapping: AC-G5/G8/G9 → T2, T5, T6 tests; AC-G7 → T6; AC-G1..G4, G6 and G10
→ T7 on the reference CSV.

## Progress and evidence

| Task | Commit | Checks | RDD | Notes |
|---|---|---|---|---|
| T1 | `ab6e21d` (+ `c6eba90` chore: project PRD skill, user-approved so the RDD candidate has no untracked files) | fmt ok · clippy -D warnings ok · test 27/27 ok (RED 27/27 `todo!()` first; parent re-ran 27/27) | medium → granted → **approved**, acknowledged (lineage `review-b037a5d15672c977`, 1 lens: reliability). Reviewed boundary → `c6eba90`. | Delegated writer. `foundationdb-tuple` 0.11.0 with `default-features = false` depends only on `memchr` (pure Rust). ~1,217 lines (626 code, 566 tests): six contract areas, above the heuristic by nature. Files: `primary.sst`, `index-<name>.sst`, `manifest.json`. |
| T2 | `6806f4c` | fmt ok · clippy ok · test 77/77 workspace (RED 46/46 `todo!()`; GREEN 45/46 then a CRLF line-number fix; parent re-ran 77/77) · full CSV release run: 8,656,904 valid / 0 rejected, 13.5 s, SHA-256 `5cb5fa41…ae35d` over 2,406,401,982 bytes (matches `sha256sum`) | medium → granted → **approved**, acknowledged (lineage `review-1d20f6160cc09491`). Reviewed boundary → `6806f4c`. Three advisory findings were turned into T2a. |
| T2a | `97e7573` | fmt ok · clippy ok · test 82/82 (RED: `--log`=`--input` truncated the CSV, an existing log was overwritten, a multi-line entry spanned 3 and then 2 lines; the mid-stream read error was already handled and is now pinned by a test; parent re-ran 82/82) | pending assess | Delegated writer. The log is opened with `create_new`, so it refuses any existing file. `RunLog::line` escapes CR/LF in one place, which keeps every entry on one physical line. Known edges: `--log`=`--input` with a missing input reports a header mismatch; two runs in the same second with the default log name make the second one abort, which is safe. |
| T3 | (this commit; hash recorded in T4) | fmt ok · clippy ok · test 116/116 (RED 31 `todo!()` + unknown option; first GREEN failed 2 tests because of the extsort data-loss bug; parent re-ran 116/116) · full CSV release run: 8,656,904 valid / 0 rejected, 43.1 s, `primary.sst` 865 MiB (Snappy), 35 segments, **peak private commit 86.7 MiB**, no leftovers; an independent reader confirmed 8,656,904 strictly increasing IDs 634..14,353,993 | T2a+T3 slice: see assess | Delegated writer. **extsort 0.5.0 rejected**: it returns an empty stream when the item count fills the last segment exactly, and it writes or truncates segments named `0`, `1`, … in the sort dir without deleting them. Replaced by an in-house k-way merge sort with stable ties and a per-sort temp folder cleaned on drop. sstable quirks handled: `add` panics on out-of-order keys (`TableWriter` validates first); short writes are ignored (`WriteAll` adapter); `Status` Display recurses forever (only `err` is read); a builder left broken after an I/O error. Bloom filter off (the reader loads the whole filter on open; index tables are prefix-scanned). Provisional staging `<output>/.crime-generator-staging-*`, which T5 replaces with the full atomic publish. ~920 code + ~1,020 tests. | Delegated writer. ~1,060 code + ~1,150 tests. The csv crate's `position().line()` lags after CRLF and blank lines, so the start line is derived from the record end. BOM stripped before parsing. Exit codes so far: 0 ok, 1 abort, 2 usage (T6 finalizes them). Rejection log line: `<mensaje> Contenido original: <fila CSV>`. |

Notes carried to T2 (done in T2): reject non-finite Latitude/Longitude (`NaN`/`inf` parse as `f64`); map empty optional fields to `None` (the decoder accepts `Some("")`).

Advisory RDD findings (non-blocking, later work):
- `R3-manifest-schema-version-unchecked` (WARNING): `Manifest::from_json` accepts any `schema_version`. The query app must reject an unknown version; it is tracked for the query change.
- `R3-decoder-accepts-empty-and-nonfinite` (SUGGESTION): the decoder accepts `Some("")` and non-finite doubles. The writer side is fixed in T2; tightening the decoder side is tracked for the query change.

## Next step

T4 — Secondary index SSTables (extension point: the `on_accepted` callback of `write_primary`, a second
`ExternalSort<IndexEntry>`, and `TableWriter` per index). Standing user instruction (2026-10-12): finish without
asking, always grant RDD review, merge into `main` after full verification.
