# Chicago crime store on CanopyDB — PRD

**Version**: 1.0
**Date**: 2026-10-20
**Parent document**: [Queryable Chicago crime store on SSTables — PRD](chicago-crime.prd.en.md) (v1.2)

> English translation of [canopy-db.prd.md](canopy-db.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build a second pair of applications, a **generator** and a **query app**, that do the same as the current ones
but keep the dataset in **CanopyDB**, a transactional key-value storage engine written in Rust, instead of in
SSTables. This document is the umbrella of the module: it sets its objectives, the contract the two pairs
share and the rules that apply to the whole module, and splits it into smaller child PRDs, each with its own
execution and its own closing (see [Split into child PRDs](#split-into-child-prds)).

**Objectives**:

1. **Compare the performance** of CanopyDB with the original SSTable approach on the same CSV, the same
   machine and the same queries, with a benchmark that can be repeated whenever a new CanopyDB version is
   released.
2. **Find CanopyDB bugs** by using it with a real volume (8.66 million records) and with the demands of the
   current contract: limited memory, atomic publication, interruptions and Linux.
3. **Contribute to the CanopyDB repository** with what is found (issues with a minimal reproduction and, when
   the cause is found, a PR with the fix), to help it mature.

**Why**: the original approach is already implemented and accepted, so it serves as a measured reference.
CanopyDB is a young project: its author declares it early-stage software and does not recommend using it with
production data. A real and demanding case like this one is exactly what it needs to mature, and the
comparison shows whether a transactional engine pays off against immutable tables for this kind of query.

**Common contract** (decision K1): the CanopyDB applications have the same options, messages, outputs and exit
codes as the original ones, and comply with the rules of the parent, of the [generator PRD](generador.prd.en.md),
of the [query app PRD](consultor.prd.md), of the [Linux support PRD](soporte-linux.prd.md) and of the
[acceptance with any CSV PRD](aceptacion-cualquier-csv.prd.en.md). The exceptions are of two kinds and apply
only to the CanopyDB applications; the original ones keep all their rules:

- those set by this document: decisions K2, K6, K8 and K9;
- those that depend on the format and are set by `canopy-db-generador`: the names of the applications, the
  shape of the published folder and of the manifest, the list of files in the final summary and what the
  no-compression option does.

*Annotation of 2026-10-22: the CanopyDB query app adds two exceptions, its help and the `schema_version` of
its JSON output (decisions KC1 and KC6 of [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*

The query batch (K7) is the only novelty that also reaches the original query app. This way, for the same
query, the two pairs return the same records and the comparison is fair.

### Starting state

Verified on 2026-10-20 on the repository, the public CanopyDB repository and a test on the reference machine:

| Aspect | Situation |
|---|---|
| Original applications | Generator and query app on SSTables implemented and accepted (parent v1.2, generator v1.0, query app v1.1), with a shared library that describes the dataset contract, an independent verifier and acceptance scripts for Windows. They also compile and pass the tests on Linux. |
| Original query app | Answers a single query per run (parent decision D6). It cannot run several queries in one process. |
| Foreign dataset in the original query app | It first reads the schema version from the manifest: if it is not its own, it rejects with the incompatible schema message; if the manifest does not have the expected shape, it rejects with `No se encontró un conjunto de datos válido en <carpeta>.` |
| Previous version when publishing | The original generator deletes the previous version if it can; if it cannot (on Windows, for example, because a query has it open), it notes it in the log and a later generation cleans it up. |
| Memory of the original query app | Its acceptance check requires for AC-C13 that the highest peak does not exceed 3 times the lowest. Measured: counting `THEFT`, 14.5 MiB; counting a rare type, 7.5 MiB; CSV file with all the thefts, 16.6 MiB. |
| CanopyDB | Latest published version: 0.2.5, from 2025-11-22, MIT license. No changes in the repository since then. 3 open issues: checkpoint management, observability functions and a profile-guided optimization report. Its continuous integration passes the tests on Windows, Linux and macOS. |
| CanopyDB on the reference machine | Compiles on Windows 11 without C++ tools. A smoke test writes and reads 100,000 entries without errors. On Linux it has not been tried on this machine; only its continuous integration is on record. |
| Readers in several processes | CanopyDB supports many simultaneous readers **within a single process**. Across processes, opening the database takes an exclusive lock: with one process reading, a second process that opens the same database waits 5 seconds and fails. Checked on the reference machine. |
| Reading that writes | Opening a database just to read also opens its internal files for writing and may truncate or reorganize its write-ahead log. If the database was closed without flushing that log to the data file, every opening applies it again in full in memory and, after 30 seconds, writes to disk. CanopyDB has no read-only open mode. |
| CanopyDB memory | The page cache grows on demand up to 1 GiB by default, double the generator's 512 MiB limit. Its size can be configured. |
| Compression | CanopyDB only compresses large values. The records of this dataset average about 138 bytes (derived from parent decision D4), so with the default configuration they would not be compressed. The original generator compresses with Snappy and has an option to not do so (decision G2). |
| CanopyDB documentation | The third example in its README configures the cache size but opens the environment without those options, so the setting has no effect (finding H1). |

**Product gaps**: none open in this document. The details of each piece are assigned to its child PRD in the
"Sets" column of [Split into child PRDs](#split-into-child-prds), and each child settles them with the user
before its execution.

## Actors

- **Generator operator**: the one from the parent. Runs the CanopyDB generator just like the original.
- **Analyst**: the one from the parent. Uses the CanopyDB query app just like the original and, with either
  query app, can run a batch of queries in a single run (decision K7).
- **Developer**: runs the comparison benchmark, investigates the findings and prepares the contributions.
- **Project owner** (the user): approves, one by one, the public actions outside this project (decision K4).
- **CanopyDB maintainer** (external): receives the issues and PRs. There is no commitment that they will
  accept them.

## Flow

### A. Generation with CanopyDB

1. The operator runs the CanopyDB generator with the same options as the original.
2. The generator validates, converts and rejects records with the same rules and messages as the original, and
   sorts by ID with the same memory budget.
3. It stores the dataset in a CanopyDB database: the main table and the parent's five secondary indexes
   (decision D2), with the same logical content as the SSTables. They are the same keys and the same values,
   including the lowercase text-index values of the query app's decision C1.
4. Before publishing, it leaves the database ready to read: querying it afterwards does not change its content
   or add files to the folder (decision K6).
5. It publishes atomically, just like the original: an incomplete dataset is never visible and a failed,
   cancelled or interrupted generation does not touch the previous dataset. The previous version is deleted
   just like in the original, also when a CanopyDB query app has it open: if it cannot be done, it is noted in
   the log and a later generation cleans it up.
6. It shows the same final summary as the original, with the same lines and the measured memory peak; the list
   of published files shows those that the folder shape set by `canopy-db-generador` publishes.

### B. Query with CanopyDB

1. The analyst runs the CanopyDB query app with the same options as the original and gets the same records,
   with the same output and the same exit codes.
2. While a CanopyDB query app has a dataset open, another process that tries to open the same dataset waits. If
   it is released within the wait time, it continues normally; if not, it is rejected with its own message and
   exit code 1 (decisions K6 and C6). If the analyst cancels during the wait, it ends like any cancellation,
   with code 130.
3. An engine error that has no equivalent in the original is shown with the closest existing message, and its
   detail goes only to the log or to the error output (decision K9).

### C. Query batch

1. The analyst prepares a file with one query per line, written with the same options as a single query.
2. They run either query app with that file. The query app opens the dataset once, runs the queries one after
   the other, in the order of the file, and ends (decision K7). All the queries of the batch read the same
   version of the dataset, the one it opened, even if the generator publishes another one in the meantime.
3. The output separates the results of each query and reports the time of each one.
4. The memory peak does not grow with the number of results or with the number of queries in the batch
   (decision K8).

### D. Performance comparison

1. The developer runs the comparison benchmark on a CSV and a working folder.
2. The benchmark generates the dataset with both pairs and runs the fixed set of queries with both query apps,
   single and in batch (decision K3).
3. It writes in the repository a report with the figures of both pairs, the CanopyDB version, the project
   version and the machine it was measured on. The report has no performance thresholds: the only limits are
   those inherited from the contract and the repetition tolerance of AC-M10. If a repetition falls outside
   that tolerance, the report says so; what else the benchmark does is set by `canopy-db-comparacion`.

### E. Findings and contribution

1. Every CanopyDB behavior that departs from what its documentation promises or from what the contract
   requires, including a performance problem that the developer shows with figures, is noted in the findings
   log with a minimal reproduction and the affected version (see [Findings log](#findings-log)).
2. For each finding, the issue is prepared and, if the cause is found, a PR with the fix and its test.
3. The project owner approves each public action separately, that is, any write outside this project: opening
   an issue or a PR, commenting, replying to a review, changing an already published PR, closing an issue,
   creating the fork or pushing commits to it. Without that approval it is not done (decision K4). Each
   approval is noted in the log.
4. As long as an own fix is not published in a CanopyDB version, the applications may use a fork with that
   fix, pinned to a specific commit that the log notes.
5. If CanopyDB cannot meet an inherited criterion (for example, the 512 MiB, the answer by ID in under 1
   second or the memory of C7), the child goes ahead and closes: the criterion is left as "no cumplido por
   CanopyDB" (not met by CanopyDB) in the log of its execution and in its closing report, with its finding in
   the log (decision K10). A failure caused by the project's code is always fixed.

## Split into child PRDs

Each child is a change with its own PRD, its execution and its closing report, and names this document as its
parent. Each one updates the help, the README and the guides of what it delivers. They are written and run in
this order. 1 and 2 do not depend on each other; 3 depends on 1 and 2; 4, on the previous three; 5, on 1 and
3. The module closes with the closing of 5.

| Order | Child PRD | Delivers | Sets, among other things |
|---|---|---|---|
| 1 | `canopy-db-generador` | The CanopyDB generator; an independent verifier that reads the CanopyDB database without any generator code; its acceptance check with the reference figures; and the findings log with H1–H4 and their reproductions. | The name of the application; what the no-compression option does; the shape of the published folder and of the manifest, including the schema version and the CanopyDB version (or the fork commit) it notes; a manifest shape with which the original query app, without any change, rejects a CanopyDB dataset with one of its existing messages, and which one; what happens with and without `--overwrite` on a folder with a dataset from the other engine; the leftovers of an interrupted generation; which existing message each engine error uses in the generator (K9); how the same logical content of K2 is checked, also against the SSTables; the name and place of the findings log. |
| 2 | `canopy-db-lote` | The query batch (K7) in the original query app. | How the batch is requested; the shape of the file (quotes, empty lines, comments, line endings, encoding); which options are accepted on a line and which on the command line, and how they combine (`--dataset`, `--format`, `--limit`, `--count`, `--output`); the output of each query and its separation; a badly written line (reject the whole file before starting or fail on that line); an empty file; cancellation in the middle of the batch; the errors and the exit code. |
| 3 | `canopy-db-consultor` | The CanopyDB query app, with the batch from the start, and its acceptance check. | The name of the application; the queries of the fixed set, which the benchmark reuses; the wait time for a locked dataset, its message and whether a notice is shown during the wait (K6); which existing message each engine error uses in the query app (K9); the rejection of a dataset from the other engine and of a dataset written with another CanopyDB version; how the engine's memory is limited (K8). |
| 4 | `canopy-db-comparacion` | The comparison benchmark and the first report. | The number of repetitions; the cold and warm measurement; how the memory peak is measured (in a batch, that of the whole batch); the options of the two generators during the measurement, such as compression or the temporary folder; the tolerance between two runs; what the benchmark does if the counts of the two pairs do not match; the systems it runs on; the shape of the report. |
| 5 | `canopy-db-robustez` | A targeted bug hunt in CanopyDB and the closing of the module. | The scenarios: interruptions during generation, cancelling or killing a query app or a batch midway (lock released, next opening), concurrent processes, queries during publication, large data and Linux; and how the result of each one is noted. |

Findings can come from any of the five and always follow flow E.

### Criteria and annotations per child

| Child | Verifies | Notes on closing |
|---|---|---|
| `canopy-db-generador` | AC-M1 (generator part), AC-M3, AC-M4 | Generator PRD, in G5 and AC-G2: the CanopyDB generator follows K2. |
| `canopy-db-lote` | AC-M7 with the original query app, AC-M9 | Parent decision D6 and query app PRD (flow and D6): the batch exists (K7). |
| `canopy-db-consultor` | AC-M1 (query app part), AC-M2, AC-M5, AC-M6, AC-M7 with the CanopyDB query app, AC-M8 with both query apps | Query app PRD, in the rule "el consultor nunca escribe en la carpeta del conjunto" (the query app never writes in the dataset folder) and in C7: the CanopyDB query app follows K6 and K8. |
| `canopy-db-comparacion` | AC-M2 (with the benchmark), AC-M10 | — |
| `canopy-db-robustez` | AC-M11, AC-M12, AC-M13 and the final check of the module | Parent: the CanopyDB module exists, with a link to this document. |

## Validations and messages

The CanopyDB applications apply all the validations and messages of the parent, the generator, the query app
and the Linux support PRD without changing a single word. This document only adds rules for the whole module;
the new messages are set by the child that introduces them:

| Situation | Rule | Message to the user |
|---|---|---|
| The original query app receives a CanopyDB dataset | It rejects it with an existing message and code 1, with no change in the original query app | The one set by `canopy-db-generador` |
| The CanopyDB query app receives an SSTable dataset, of any schema version | It rejects it as an invalid dataset, with code 1 | `No se encontró un conjunto de datos válido en <carpeta>.` |
| Another process has the CanopyDB dataset open | It waits; if not released in time, it rejects without results, with code 1 | The one set by `canopy-db-consultor` |
| Cancellation during that wait | It ends like any cancellation, with code 130 | `Consulta cancelada.` |
| Engine error with no equivalent in the original | The closest existing message is used; the detail goes only to the log or to the error output (K9) | The one set by the child of each application for each error |
| Public action outside the project without approval | It is not done (K4) | — |

## Output

- **Applications**: the same public output as the original ones (console, error output, CSV, JSON, output
  file, summary and generator log), with the exceptions of the [common contract](#objective-and-context) and
  the batch output, which `canopy-db-lote` sets.
- **Comparison report**: a document in the repository for each benchmark run that is worth keeping. At a
  minimum it includes, for each pair:
  - generation: duration, memory peak and size on disk of the published dataset, with the options used;
  - each query of the fixed set, single: time (median of the repetitions), memory peak and number of results;
  - the batch with all the fixed queries: time of each query, total time and memory peak of the batch;
  - the CanopyDB version (and the fork commit, if used), its configuration, the project commit, the CSV used
    (with its fingerprint) and the machine.

  Its exact shape is set by `canopy-db-comparacion`.

### Findings log

A document in the repository, in Spanish, common to the whole module, with one entry per finding. The issues
and PRs that get published are written in English, the language of the CanopyDB repository.

| Field | Content |
|---|---|
| Identifier | `H1`, `H2`… in order of creation; never reused. |
| Date | Date of creation. |
| CanopyDB version | The version in which it reproduces. |
| Type | Bug, limitation, documentation or performance. |
| Description and effect | What happens and how it affects the module. |
| Minimal reproduction | Steps or a program that reproduce it without the rest of the project; in a documentation finding, the quote of the affected text. |
| Status | Detected, issue published, PR published, accepted, published in a version or dismissed (with the reason). |
| Approvals | Each approved public action, with its date and its link. |
| Links | Issue, PR and version that fixes it, if they exist. |
| Fork | The commit of the fork the applications depend on, if the fix is not yet published. |

The log starts with what was checked when writing this document:

| Id | Type | Finding |
|---|---|---|
| H1 | Documentation | The third README example configures the cache size but opens the environment without those options; the setting has no effect. |
| H2 | Limitation | Two processes cannot open the same database at the same time: the second one fails after 5 seconds. |
| H3 | Limitation | There is no read-only open: reading opens the internal files for writing and may modify them. |
| H4 | Documentation | The default cache size (1 GiB) and its on-demand growth do not appear on the front page of the documentation, and they condition any memory budget. |

## Reuse

- **Parent, generator, query app, Linux support and acceptance with any CSV**: all their rules, validations,
  messages, outputs and decisions, except for the exceptions of the common contract.
- **Shared dataset contract**: record schema, key and value encoding with FoundationDB tuples, the five
  indexes, the pointer to the active version and the manifest. The two pairs store the same keys and the same
  values; only where changes.
- **CSV reading and validation, external sorting and atomic publication** of the original generator.
- **Query resolution and output formats** of the original query app (decisions C1–C11).
- **Independent verifier, direct CSV filtering and acceptance scripts**: the basis for checking the CanopyDB
  applications with the same figures. On a CanopyDB dataset, the verifier and the queries of the acceptance
  check run one at a time (K6).
- **Memory threshold of AC-C13**: the maximum ratio of 3 that the original query app's acceptance check already
  uses.
- **Memory peak measurement** on Windows and on Linux (decision L3 of the Linux support PRD).

## Out of scope

- Replacing or retiring the original applications: they remain the reference.
- Changing the original applications, except for the query batch in the query app (K7).
- Optimizing the original applications for the comparison.
- A read-only mode in CanopyDB or access by several processes at once: they are recorded as findings (H2 and
  H3) and an issue may be opened, but developing them is not part of the module.
- CanopyDB datasets on read-only folders or media: the CanopyDB query app needs to be able to write its
  internal files (K6), so it rejects them with the corresponding existing message (K9).
- Reading or converting datasets between engines: a query app that reads both formats or moving an SSTable
  dataset to CanopyDB without the CSV.
- Guaranteeing that a dataset is readable with another CanopyDB version: if it is not, it is regenerated; how
  it is rejected is set by `canopy-db-consultor`.
- Durability against power cuts, as in the parent.
- Features that CanopyDB would allow and the contract does not have: incremental updates, deletes,
  user-visible transactions, server mode and any other query that the original query app does not support.
- In the batch: running queries in parallel, reading the batch from standard input and interactive mode.
- Comparing with other engines (RocksDB, LMDB, redb, Fjall…).
- Maintaining a CanopyDB fork beyond the own fixes pending publication.
- Any commitment on what the CanopyDB maintainer does with the issues and PRs.
- Publishing the comparison reports outside the repository.
- Running the acceptance scripts on Linux, as in the original.
- macOS and other systems, as in the Linux support PRD.

## Left to the implementation

- CanopyDB's internal configuration, including how its memory is limited (K8), as long as the contract and
  the memory limits are met. The comparison report states the configuration used.
- The CanopyDB version: the latest published one when each child starts, today 0.2.5, or a fork with own fixes
  pending publication (flow E).
- When a performance problem deserves a finding: the developer decides it and justifies it with figures.

## Acceptance criteria

These are the criteria of the module; each child also has its own, and the table
[Criteria and annotations per child](#criteria-and-annotations-per-child) says who verifies each one. The
figures correspond to the reference CSV, with the same caveat as in the parent. An inherited criterion that
CanopyDB cannot meet is handled according to K10.

1. **AC-M1** — Given the reference CSV, when the datasets are generated and queried with the CanopyDB
   applications, then they meet the acceptance criteria of the parent, the generator, the query app and
   acceptance with any CSV with the same figures, with these substitutions: AC-M3 replaces AC-G10; AC-M4,
   AC-G2 and AC-L8; AC-M8, AC-C13; and the rejection of an SSTable dataset in
   [Validations and messages](#validations-and-messages), the second half of AC-C18 (dataset with schema 1).
2. **AC-M2** — Given the same CSV generated with both pairs, when each query of the fixed set is run with both
   query apps, then the count is equal and the set of returned records is the same, regardless of order (C11).
   In queries with a limit, each query app returns as many records as the limit, all of them from the
   result without a limit.
3. **AC-M3** (decision K2) — Given the same CSV, when it is generated twice with the CanopyDB generator, then
   the two datasets have exactly the same keys and the same values in the main table and in each index, even
   if their files are not identical; and those keys and values are the same as those of the SSTables generated
   with the original.
4. **AC-M4** (decision K2) — Given the complete reference CSV, when the CanopyDB generator is run, then the
   measured memory peak does not exceed 512 MiB (except K10) and the final summary shows it. On Windows the
   acceptance check verifies it; on Linux it is checked by hand, like AC-L8, and noted in the closing report of
   `canopy-db-generador`.
5. **AC-M5** (decision K6) — Given a CanopyDB dataset and a batch of queries running on it, which lasts longer
   than the wait time set by `canopy-db-consultor` (the test may repeat queries until it manages to), when a
   second query app opens the same dataset, then it waits, is rejected when the wait ends with its message and
   code 1, and the batch ends normally. When the batch lasts less than the wait, the second query app waits and
   answers normally. When the analyst cancels the second one during the wait, it ends with `Consulta cancelada.`
   and code 130.
6. **AC-M6** (decision K6) — Given a freshly published CanopyDB dataset, when all the queries of the fixed set
   are run, single and in batch, then the dataset folder has the same file names as before, with no new ones,
   and the manifest is identical; and afterwards, the logical content (checked as in AC-M3) has not changed.
   The content of the engine's internal files may vary (K6).
7. **AC-M7** (decision K7) — Given a file with several queries, when it is run as a batch with either query
   app, then each query returns the same count and the same set of records as when run alone; those with a
   limit return as many records as the limit, all of them from the result without a limit. When the generator
   publishes a new version midway through the batch, all the queries of the batch return results from the
   version that was opened.
8. **AC-M8** (decision K8) — Given, with each query app separately, a count of `THEFT`, a count of a primary
   type with few records and a batch with all the queries of the fixed set, when the memory peak of each run is
   measured, then the highest of the three peaks does not exceed 3 times the lowest. The peaks of one query app
   are not compared with those of the other.
9. **AC-M9** — Given the original applications, when their tests and their acceptance check are run, then all
   of them still pass, plus the new ones for the batch.
10. **AC-M10** (decision K3) — Given the comparison benchmark, when it is run on the reference CSV, then it
    produces a report with all the figures of [Output](#output) for both pairs; and repeating it on the same
    commit gives figures within the tolerance set by `canopy-db-comparacion`.
11. **AC-M11** — Given each scenario set by `canopy-db-robustez`, when that child is closed, then the scenario
    has been run and its result, with or without a bug, is noted; each bug found has its entry in the findings
    log.
12. **AC-M12** (decision K4) — Given the findings log at the close of the module, when it is compared with the
    list of issues, PRs and comments published in the CanopyDB repository and in the fork, then each finding
    has its minimal reproduction, its version and its status, and each public action in that list has the
    project owner's approval noted.
13. **AC-M13** — Given the repository on Windows and on Linux (Ubuntu 24.04 on WSL2), when the tests are run,
    then all of them pass, including those of the CanopyDB applications.

## Decisions

All were made with the user on 2026-10-20.

- **K1 — New applications, same contract**: the module creates a new generator and a new query app on
  CanopyDB, with the same options, messages, outputs and exit codes as the original ones. Reason: the
  comparison is only fair if the two pairs answer the same way to the same thing, and the originals remain the
  reference. Discarded: an option to choose the engine in the current applications (it changes their command
  line) and applications with a free contract (the outputs would stop matching).
- **K2 — Guarantees required and measured in the CanopyDB generator**: it must meet the 512 MiB peak and
  atomic publication, because they are promises to the operator; if it is CanopyDB that prevents it, K10
  applies. For it, the deterministic output of G5 becomes "same logical content": two generations of the same
  CSV store the same keys and values, even if the files differ. The original generator keeps G5 as is. Reason:
  a page-based engine with free space can hardly produce byte-identical files, and CanopyDB does not promise
  it.
- **K3 — Repeatable comparison benchmark**: a benchmark runs the two pairs on the same CSV and the same machine
  with a fixed set of queries (by ID, by case number, primary type `THEFT`, date range, combination of
  conditions, count and list), single and in batch, and writes a report in the repository. Reason: it can be
  repeated with each new CanopyDB version or after fixing a bug. Discarded: a single manual report and a
  comparison limited to generation and query by ID.
- **K4 — Contribution with approval**: each finding is first documented in the repository with a minimal
  reproduction. Then the issue is prepared and, if the cause is found, a PR with the fix and its test. No
  public action outside this project (issue, PR, comment, reply, change to a PR, closing, fork or commits in
  the fork) is done without the project owner's approval, one by one. As long as an own fix is not in a
  published version, the applications may use a fork pinned to a commit. Reason: publishing is an external and
  irreversible action, and the repository has had no changes for 11 months, so an issue without a fix will
  probably remain unresolved.
- **K5 — Split into five children**: generator, batch, query app, comparison and robustness, in that order (see
  [Split into child PRDs](#split-into-child-prds)). Reason: the whole module is too big for a single change.
  The batch was added when K7 was decided.
- **K6 — The CanopyDB query app may lock the dataset**: while a CanopyDB query app has a dataset open, another
  process that tries to open it waits and, if not released in time, is rejected. The query app may open the
  engine's internal files for writing, but the queryable content and the manifest never change and no new files
  appear; the generator publishes the database ready to read. Reason: CanopyDB 0.2.5 only supports concurrent
  readers within a single process and has no read-only open (H2 and H3). Developing that mode would be a small
  and additive change in CanopyDB, but it depends on the maintainer accepting it, and the user preferred to
  accept the limitation. Only for the CanopyDB query app, it replaces the original query app's rule "el
  consultor nunca escribe en la carpeta del conjunto" (the query app never writes in the dataset folder) and
  its implicit possibility of running several queries at once on the same dataset; the original query app does
  not change.
- **K7 — Query batch in both query apps**: both query apps can read a file with one query per line, written
  with the same options as a single query; they open the dataset once, run the queries one after the other and
  end. Reason: if each query is a new process, the comparison mostly measures startup and cold opening, not the
  engine; and with K6, the batch is the way to run several queries in a row on a CanopyDB dataset. It is still
  a run that starts, answers and ends (parent D6), but it changes the original query app. Discarded: interactive
  mode (the parent left it for a later change and it is harder to measure) and a batch only inside the
  benchmark (the analyst would gain nothing).
- **K8 — Memory of the query apps**: rule C7 of the original query app applies to both query apps, single and
  in batch: the peak does not grow with the number of results or with the number of queries in the batch. It is
  checked with each query app separately, with the maximum ratio of 3 between the highest and the lowest peak
  that the original query app's acceptance check already uses. In the CanopyDB query app this requires limiting
  the memory the engine uses; how it is limited is left to the implementation and appears in the comparison
  report. Reason: the CanopyDB query app has the same contract, and with the default configuration the peak
  would grow with what each query reads. Discarded: a fixed 512 MiB cap, which would be a new demand on the
  original query app; only measuring, which would break C7; and a ratio below 2, which the original query app
  does not meet today in its own AC-C13 check (2.21 between the CSV file of all the thefts and the count of a
  rare type). The peak of a batch has never been measured with the original query app: `canopy-db-consultor`
  measures it for the first time when verifying AC-M8, and if the original does not meet the ratio, it is a
  failure of the project and is fixed. *Annotation of 2026-10-21: in a batch, the checked queries of the file
  may be kept; what does not grow is the memory of each query (decision B7 of the
  [query batch PRD](canopy-db-lote.prd.en.md)).*
- **K9 — Engine errors**: a CanopyDB error with no equivalent in the original applications (for example, a
  damaged database or one that cannot be opened) is shown with the closest existing message, and its detail
  goes only to the log or to the error output. Reason: the contract keeps the messages unchanged down to the
  word, and the technical detail is useful for investigating findings. The only exception is the locked dataset
  of K6, which has its own message.
- **K10 — Criteria that CanopyDB cannot meet**: if CanopyDB prevents meeting an inherited criterion, the child
  goes ahead and closes; the criterion is left as "no cumplido por CanopyDB" (not met by CanopyDB) in the log of
  its execution and in its closing report, and the finding goes into the log. A failure caused by the project's
  code is always fixed. Reason: the module is looking precisely for the limits of the engine, and blocking it
  on one of them would prevent finishing the comparison.

## References

- Parent document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
- [Generator PRD](generador.prd.en.md), [query app PRD](consultor.prd.md),
  [Linux support PRD](soporte-linux.prd.md) and
  [acceptance with any CSV PRD](aceptacion-cualquier-csv.prd.en.md)
- CanopyDB: https://github.com/arthurprs/canopydb and https://docs.rs/canopydb

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 1b4d192c | 2026-10-20 11:20 UTC+01:00 | 3 h 38 min | 2 h 25 min | 1 h 13 min | 30 min | 43 min | 16 |

<!-- prd-time: [{"session": "1b4d192c-7b21-4a20-a982-50c7db1fafce", "start": "2026-10-20T10:20:49.911000+00:00", "wall": 13098, "discarded": 8708, "agent": 1803, "human": 2586, "interventions": 16, "idle_minutes": 10}] -->
