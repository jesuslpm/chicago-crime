# CanopyDB comparison benchmark — PRD

**Version**: 1.0
**Date**: 2026-10-22
**Parent document**: [Chicago crime store on CanopyDB — PRD](canopy-db.prd.en.md) (v1.0)

> English translation of [canopy-db-comparacion.prd.md](canopy-db-comparacion.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Deliver the fourth child of the CanopyDB module (order 4 of [Split into child PRDs](canopy-db.prd.en.md#split-into-child-prds)):

- the **comparison benchmark** (K3 of the parent): a developer tool that generates the same CSV with the two
  pairs, runs the fixed set of queries with both query apps, one by one and as a batch, measures times, memory
  and size on disk, checks that the two pairs return the same results (AC-M2) and writes a report in the
  repository;
- the **first report**, measured with the reference CSV on the reference machine.

This document inherits all the rules of its ancestry and does not repeat them. It only sets what the parent
assigns to it ("Sets" column of the split): the number of repetitions, the cold and warm measurement, how the
memory peak is measured, the options of the generators during the measurement, the tolerance between two runs,
what the benchmark does if the results of the two pairs do not match, the systems it runs on and the shape of
the report. It narrows one of them: the cold measurement is not done (KM2).

**Why now**: the two query apps and the fixed set of queries already exist (`canopy-db-consultor`), and the
performance figures that are known come from verifications and one-off tests, not from a controlled
measurement. Finding H5 says so expressly: its figures "are not from a controlled benchmark; `canopy-db-comparacion`
measures them". Without a benchmark there is no way to know whether a new version of CanopyDB, or a fix of our
own, improves anything.

### Starting state

Verified on 2026-10-22 against the repository:

| Aspect | Situation |
|---|---|
| Comparison benchmark | None exists that compares the two pairs. What it measures today are the four acceptance checks, in PowerShell and only for Windows, and the minimal reproduction of H5, which measures writing to CanopyDB without the rest of the project. |
| Acceptance checks | They receive `-WorkDir` (required), `-Csv` (by default `chicago-crime-data.csv` in the repository root, which by convention is the reference extract) and `-SkipBuild`, among others. They build the applications, measure the duration of each run and its peak memory from outside (the maximum private committed memory of the process), and write a Markdown report in English to the work folder whose header has the lines `- **Result**:`, `- **Date**:`, `- **Commit**:` (`<commit> with uncommitted changes` if there are changes; `unknown` without git), `- **Machine**:`, `- **CSV**:`, the CSV mode line and `- **Work folder**:`. They end with 0 (`PASS`) or 1 (`FAIL`). Each one deletes its own entries in the work folder at the start. |
| Fixed set of queries | F1 to F11 (decision KC7 of [canopy-db-consultor.prd.en.md](canopy-db-consultor.prd.en.md#fixed-set-of-queries)), in a batch file of the check. The F2 line carries `<número>`, which is replaced before running with the first shared case number of the CSV. |
| Generators | `crime-generator` and `crime-canopy-generator`, with the same options: input, output, temp folder (the system one by default), log, overwrite and no compression. By default, the original compresses with Snappy and CanopyDB with LZ4 for large values. The final summary shows the valid and rejected records, the duration, the peak private committed memory and the size of each published file, with no total. They have no threads option. |
| Query apps | `crime-query` and `crime-canopy-query`. Each query writes its statistics line to the error output, with the number of results and the query time not counting the opening; the batch adds a header per query and `Lote completado: <n> consultas en <t> s.`, with the time of the whole run. Neither shows its memory. |
| Known figures, generation | Original: 65–75 s and about 1.1 GB with Snappy (README); peak of 93.4 MiB. CanopyDB: best measured generation 1 min 41 s and a database of 2,718,945,280 bytes (H5); peak of 219.5 MiB on Windows and 212.2 MiB on Linux (closing report of `canopy-db-generador`). In a verification with the machine busy with other processes, one generation of the original took 251.5 s. |
| Known figures, queries | From the closing report of `canopy-db-consultor`, a single run: F11 takes 67.0 s with the original and 10.5 s with CanopyDB; peaks of 14.5 / 7.6 / 19.3 MiB (`THEFT` count, `RITUALISM` count, batch F1–F11) with the original and 2.1 / 1.0 / 2.4 MiB with CanopyDB; the query by ID of the CanopyDB query app takes 22–25 ms of whole process, and 514 ms the first time. |
| Variation between runs | It has never been measured. H5 notes that the variation between its runs is not controlled. |
| CanopyDB | The applications use published CanopyDB 0.2.5, with no fork. The CanopyDB manifest records the version and `fork_commit` `null`. |
| H5 and its prototype | H5 (performance): CanopyDB has no bulk load of sorted entries. A test patch in the finding's documentation speeds up the writing of its minimal reproduction by about 2.5 times; the applications do not use it. |
| Reference machine | Windows 11, 32 threads, 64 GB of RAM (root document). The processor model and the disk are not recorded. |
| Linux | Ubuntu 24.04 on WSL2 is the reference Linux (L1). The acceptance checks are not run on Linux, and the Linux guides say so. |

**Product gaps**: none open. Each point of the parent's "Sets" column remains as a rule in this document or in
[Left to the implementation](#left-to-the-implementation).

## Actors

- **Developer**: runs the benchmark, reads the report, decides whether to keep it in the repository and
  registers the performance findings that appear (flow E of the parent).
- **Project owner** (the user): reads the reports to decide whether a transactional engine pays off and which
  contributions to CanopyDB are worthwhile.

## Flow

1. The developer runs `run-canopy-comparison.ps1` (KM1) on Windows, with the machine under no other load, with
   a work folder of their own and, optionally, the CSV, the number of repetitions `R`, the no-compression option
   and the data table of a previous report to compare with (KM10).
2. The benchmark does the preliminary checks of [Validations and messages](#validations-and-messages), in
   their order, and builds the applications in production mode, except with `-SkipBuild`. Any failure in this
   step ends with 1 without running any application or writing a report.
3. **Generation** (KM3): it repeats `R` times (runs 1 to `R`) the generation with the two generators,
   alternating them (original and then CanopyDB in each repetition), each time in an empty output folder and
   with the temp folder inside the work folder, with the same command-line options for both. From the final
   summary of each generator it takes the duration, the records, the peak and the size of the published set
   (KM4). From each pair it keeps the set of its last successfully finished generation.
4. If some pair has no successfully finished generation, the benchmark does not run queries: it writes the
   report with what was measured and ends with 1.
5. **Single queries** (KM2): on the kept set of each pair, it runs as a new process each query of the fixed
   set, from F1 to F11. For each query: run 0 with the original query app and with CanopyDB, and then runs 1 to
   `R`, each with the original and then with CanopyDB; then it moves on to the next query. The single queries
   use exactly the text of the batch lines. For each run it takes the query time (from the statistics line), the
   number of results, the process time and the peak memory (KM4).
6. **Batch**: after all the single queries, it runs the batch with the eleven queries: run 0 with the original
   and with CanopyDB, and then runs 1 to `R`, the same way. For each run it takes the time and the number of
   results of each query, the time of `Lote completado`, the process time and the peak memory of the whole
   process. A batch that fails is a single failed run; the queries it did not get to answer are left without
   measures.
7. **Same results** (AC-M2, KM6): it applies the rule of AC-M2 of
   [canopy-db-consultor.prd.en.md](canopy-db-consultor.prd.en.md#acceptance-criteria), which already narrows
   that of the parent (the same set of records only in F1, F2 and F11, and F10 within F11), to the single
   queries and to the batch, with this generalization for any CSV: F10 returns as many records as the lesser of
   1,000 and the number of results of F11, all of them from the result of F11.
8. It writes the report and its data table (see [Output](#output)), ends the console with two lines,
   `Result: <result>` and `Report: <path of the report>`, and ends with 0 if all the runs finished well and all
   the queries are `SAME`, or with 1 otherwise (KM6).

A run finishes well when the application ends with 0 and the benchmark obtains all its measures; if it ends
with 0 but some measure is missing (for example, there is no statistics line), it is a failed run.

## Validations and messages

The benchmark is a developer tool, like the acceptance checks: its messages and its report are in English, and
the preliminary checks fail as in them, with a PowerShell error that contains the message and code 1.
Preliminary checks, in this order; the first one that fails ends the benchmark without running any application
or writing a report:

| Order | Situation | Message |
|---|---|---|
| 1 | `-Repetitions` is not an integer | The PowerShell error when reading the parameter |
| 2 | `-Repetitions` less than 1 | `-Repetitions must be at least 1.` |
| 3 | The CSV does not exist | `Missing file: <path>` |
| 4 | `-Baseline` does not exist | `Baseline file not found: <path>` |
| 5 | The first line of `-Baseline` is not exactly the header of the [Data table](#data-table) | `Baseline file is not a comparison bench data file: <path>` |
| 6 | The CSV has no shared case number, so F2 cannot be written | `The CSV has no shared case number, so F2 cannot be built.` |
| 7 | The report or the data table with the name of this run already exists | `Report already exists: <path>` |
| 8 | There is not enough free space in the work folder | `Only <free> free in <folder>; about <n> GB are needed.` (`<n>` depends on the space the benchmark requires) |
| 9 | The build fails | `cargo build failed:` followed by the error output of cargo |
| 10 | One of the four applications is missing | `Missing binary: <path>` |

The work folder is created if it does not exist, and so is the reports folder of the repository that contains
the script. The benchmark only deletes what it creates itself, in this run or in a previous run of the
benchmark; what a cancelled run leaves behind is cleaned up by the next one.

During the measurement:

| Situation | Rule | Code |
|---|---|---|
| A run of an application fails (code other than 0 or some measure is missing) | It continues; the run stays in the data table and in `Notes`, and does not count in the medians | 1 |
| The valid or rejected records of the two pairs do not match | It continues; it is noted in `Notes` | no effect |
| No generation of a pair finishes well | Step 4 of the flow | 1 |
| The two pairs return different results in a query | It keeps measuring; the query is left as `DIFFERENT` (KM6) | 1 |
| A repetition falls outside the KM5 tolerance with respect to the median of its series | It continues; it counts in the `Out of tolerance` column | no effect |
| A figure exceeds an inherited limit (KM7) | It continues; it is noted in `Notes` | no effect |
| Ctrl+C | It stops the application in progress and ends without writing the report or the data table | other than 0 |

## Output

### Report

One Markdown file in English per run of the benchmark (KM8), in `docs/canopy-db/comparisons/`, named
`<YYYY-MM-DD-hhmmss>-<commit>.md` with the start date and time and the short commit (`unknown` without git). It
is always written when the benchmark passes the preliminary checks and is not cancelled; keeping it in the
repository is the developer's decision. It is never modified after being written. Content, in this order:

```text
# CanopyDB comparison bench

- **Result**: <result>
- **Date**: <start> (duration <duration>)
- **Commit**: <commit>
- **Machine**: <processor>, <n> logical processors, <n> GiB RAM, <system>
- **CSV**: `<path>`, <bytes> bytes
- CSV: the reference extract (SHA-256 <hash>).
- **CanopyDB**: <version> (fork <fork commit or none>)
- **CanopyDB configuration**: generator: <settings>; query app: <settings>
- **Options**: <R> repetitions, compression <snappy or none> (SSTables) and <lz4 or none> (CanopyDB), temp folder `<path>`
- **Build**: <release build or skipped (-SkipBuild)>
- **Baseline**: `<path as passed>` or none
- **Work folder**: `<path>`
```

- `<result>`: `PASS: <n> runs, same results in all 11 queries`; with some failure or difference,
  `FAIL: <f> of <n> runs failed, <d> of 11 queries returned different results`; and if no queries were run
  (step 4 of the flow), `FAIL: <f> of <n> runs failed, queries not run`. `<n>` counts all the application runs
  that the benchmark launched, runs 0 included; `<d>`, the `DIFFERENT` queries.
- `Date`, `Commit`, `Machine` and `CSV` have exactly the format of the acceptance checks (date
  `YYYY-MM-DD hh:mm`, duration, CSV bytes with thousands separator, resolved paths); the common rules below
  apply to the sections. When deciding whether there are uncommitted changes, `docs/canopy-db/comparisons/` does
  not count, so that a freshly written report does not mark the next one.
- The mode line changes with respect to that of the checks, because the benchmark does not require figures: it
  is the one above with the reference extract and `CSV: not the reference extract (SHA-256 <hash>).` with
  another CSV. The hash is that of the manifest of the kept original set; if there is none, the benchmark
  computes it over the CSV.
- `CanopyDB` comes from the manifest of the kept CanopyDB set; if there is none, the line is `- **CanopyDB**: unknown`.
- `<settings>`: each CanopyDB setting that the application changes with respect to the CanopyDB default, as
  `<name>=<value>` separated by `, `, with the name CanopyDB gives it, sizes in MiB and the page cache size
  always first.

Then, these sections:

- `## Generation`: table `| Pair | Runs | Duration median | Duration min | Duration max | Peak median | Size | Valid | Rejected | Out of tolerance |`,
  one `SSTables` row and one `CanopyDB` row. `Runs` is `<finished well> of <R>`. `Size`, `Valid` and `Rejected`
  are those of the kept generation.
- `## Results`: table `| Query | Line | SSTables single | CanopyDB single | SSTables batch | CanopyDB batch | Check |`,
  one row per query F1–F11; `Line` is the line that was run, in backticks, with the case number already
  substituted; the numbers of results are those of run 0; `Check` is `SAME`, `DIFFERENT`,
  `INCOMPLETE` or `NOT RUN` (KM6).
- `## Single queries`: table `| Query | Pair | Runs | Query time median | Process time median | Process time first run | Peak median | Out of tolerance |`,
  two rows per query (`SSTables` and `CanopyDB`). `Runs` is `<finished well> of <R>`, not counting run 0.
- `## Batch`: table `| Query | SSTables query time median | CanopyDB query time median |` with one row per
  query, and table `| Measure | SSTables | CanopyDB |` with the rows `Runs` (as in `Single queries`),
  `Batch total median`, `Process time median`, `Process time first run`, `Peak median`, `Peak first run` and
  `Out of tolerance`.
- If no queries were run (step 4 of the flow), `Single queries` and `Batch` have all their rows, with
  `0 of <R>` in `Runs` and `—` in the measures.
- `## Baseline`, only with `-Baseline`: table `| Section | Row | Measure | Baseline | This run | Change | Status |`,
  with one row for each median of `Generation`, `Single queries` and `Batch` and for `Size`, `Valid` and
  `Rejected` of `Generation`, in the order in which they appear in the report: in the tables with one row per
  pair, row by row and, in each, column by column; in the tables with one column per pair (the two of the
  batch), row by row and, in each, `SSTables` and then `CanopyDB`. `Section` is the title of the section; `Row`,
  the pair (`SSTables` or `CanopyDB`), preceded by the query in `Single queries` and in the per-query table of
  the batch (`F3 CanopyDB`); `Measure`, the name of the column or of the row. `Change` is the difference with
  respect to the baseline as a percentage, with sign and one decimal (`+12.3 %`); `Status` is `inside tolerance`
  or `outside tolerance` (KM5).
- `## Notes`: first the figures that exceed an inherited limit (KM7), in the order of the list and with
  `SSTables` before `CanopyDB`; then the records that do not match; then the failed runs, in the order in which
  they were run; or `none`:
  - `- AC-M4: <pair> generation peak median <x> MiB exceeds 512 MiB.`
  - `- AC-C7: <pair> F1 process time median <x> s exceeds 1 s.`
  - `- AC-M8: <pair> peak ratio <x> exceeds 3 (F3 <a> MiB, F4 <b> MiB, batch <c> MiB).`, with the medians; the
    ratio is the largest of the three medians divided by the smallest, with two decimals.
  - `- Records differ: SSTables <valid> valid and <rejected> rejected, CanopyDB <valid> valid and <rejected> rejected.`,
    comparing the kept generations; it does not appear if some pair has no kept generation.
  - `- Failed run: <phase> <pair> run <r>: <reason>.`, with the query after `<pair>` in the single queries
    (`single CanopyDB F7 run 3`); `<phase>` is `generation`, `single` or `batch`; `<reason>` is
    `exit code <c>: <last non-empty line of its error output, or no error output>`, or, if it ended with 0,
    `missing <measure>` with the first measure that is missing in the order of the data table, as its `metric`
    preceded by the query in the batch (`missing F7 query_s`).

`<pair>` is `SSTables` or `CanopyDB` throughout the report.

Common rules:

- **Medians**: of runs 1 to `R` that finished well; with an even number, the mean of the two central ones.
  Run 0 never enters a median. They are computed in decimal, not in binary floating point, with the values of
  the data table, and rounded to the report format with ties away from zero (0.0415 gives 0.042); the tolerance
  and `Change` use the already rounded medians.
- **`Out of tolerance`**: how many of the runs 1 to `R` that finished well of that row have any of the measures
  that the row summarizes in medians (times and peaks) outside the KM5 tolerance with respect to the median of
  their series; in `Batch`, those of the whole batch (total, process and peak). Differences in results and
  records do not count here: they go to `Check` and to `Records differ`. With `R` = 1 it is 0.
- **Formats**: times in seconds with three decimals (`0.041 s`); memory in MiB with one decimal
  (`14.5 MiB`); sizes in bytes and MiB (`1177432100 bytes (1122.9 MiB)`); integers without thousands separator;
  decimal point. A cell with no data shows `—`; in particular, a series with no runs that finished well, a run 0
  that did not finish well, a pair with no queries or with no kept generation, and in `Baseline` a measure that
  is missing in the baseline or a baseline with value 0 (in `Change` and `Status`).

### Data table

Next to the report, with the same name and extension `.csv`: UTF-8 without BOM, comma-separated, without
quotes, with decimal point, LF line endings, a final line break and one row per measure, in the order in which
the runs were run and, within each, in the order of the measures in the list; in the batch, from F1 to F11
with `query_s` and `results` each, and then those of the whole batch. When reading a `-Baseline`, a carriage
return at the end of a line is ignored. Header:

```text
phase,pair,query,run,exit_code,metric,value
```

- `phase`: `generation`, `single` or `batch`. `pair`: `sstable` or `canopydb`.
- `query`: `F1`–`F11`; empty in the generation and in the measures of the whole batch.
- `run`: from `1` to `R` in the generation; from `0` to `R` in the queries.
- `exit_code`: the code with which the run ended.
- `metric` and its unit: in `generation`, `duration_s`, `peak_mib`, `size_bytes`, `records_valid` and
  `records_rejected`; in `single`, `query_s`, `process_s`, `peak_mib` and `results`; in `batch` per query,
  `query_s` and `results`; in `batch` of the whole batch, `batch_total_s`, `process_s` and `peak_mib`.
- `value`: seconds with three decimals, MiB with one decimal and integers without separator; empty if the run
  did not get to give that measure.

Each launched run has all the rows of its phase, failed ones too; the queries that were not run (step 4 of the
flow) have no rows. The medians of the report come from this table, and it is what `-Baseline` reads.

## Documentation

- **Script help**, like that of the acceptance checks.
- **README**: a section on the benchmark: what it is for, how it is run, its parameters, its measured duration,
  where it leaves the report, how to compare with a previous one and that it needs a work folder of its own and
  the machine under no other load.
- **Linux guides**: the benchmark, like the checks, only runs on Windows.
- **Findings log**: H5 links the first report as its controlled measurement.

## Reuse

- **Acceptance checks**: their preliminary checks and messages, the build, the memory measurement from outside,
  the format of their header and the collection of the commit, the machine and the CSV hash.
- **Fixed set of queries** and the substitution of `<número>` from the CanopyDB query app check, without
  changing its file.
- **AC-M2 check** of that verification: how it compares counts and sets of records.
- **Memory peak measurement** of the root document on Windows: the maximum private committed memory.
- **Final summary of the generators** and **statistics line and final line of the batch** of the query apps,
  as the source of the duration, the records, the peak of the generators, the query times and the results.

## Out of scope

- Running the benchmark on Linux (KM9).
- Measuring cold, with the operating system file cache emptied, rebooting the machine or with external tools
  (KM2). Run 0 is reported separately, but it is not a cold measurement.
- Performance thresholds of its own or failing because of a figure: the benchmark only fails because of failed
  runs or different results (KM6, KM7). Treating a criterion as "not met by CanopyDB" (K10) belongs to the
  closing report, not to the benchmark.
- Measuring the H5 patch or any fork: there is no approved fork (K4). When there is one, the benchmark
  measures it unchanged, because the report notes the version and the fork commit of the manifest.
- Checking that a `-Baseline` was measured with the same CSV, the same options or the same machine: the
  developer chooses what to compare with.
- Checking that the applications built before `-SkipBuild` correspond to the commit of the report.
- Running the benchmark at the same time as another benchmark, an acceptance check or another process on the
  same work folder, and a maximum time for an application that does not finish.
- Choosing which queries are run, a different `R` per phase, stopping midway and resuming, merging several
  reports and continuous integration.
- Changing the applications, their options or their messages to measure better, and tuning CanopyDB or the
  applications so that they come out better in the benchmark.
- From flow E, everything except registering in the findings log.

## Left to the implementation

- Where the standard output of the queries goes while they are measured, as long as it is the same for both
  query apps in all the runs and the benchmark can compare the results of AC-M2.
- How the benchmark obtains the CanopyDB settings that it shows, as long as they are those of the measured
  commit.
- The organization of the work folder, what it leaves in it when it finishes and the free space it requires,
  computed from a measured run.
- The way of showing progress on the console, except the `Result` line and the path of the report at the end.
- How the tests provoke a failed run, different results and the failures of the preliminary checks (AC-KM1 to
  AC-KM3), as long as it adds no visible option to the benchmark or to the applications and does not change a
  normal run.

## Acceptance criteria

The figures correspond to the reference CSV, with the same caveat as in the parent. AC-M2 and AC-M10 are the
module criteria that the parent assigns to this child; they are checked on closing, running the benchmark on
the reference machine under no other load, and noted in the closing report. The benchmark tests run on Windows.

1. **AC-M2** (with the benchmark) — Given the reference CSV, when the benchmark is run, then the `Results`
   section of the report says `SAME` in the eleven queries and the benchmark ends with 0.
2. **AC-M10** (K3, KM5) — Given the benchmark, when it is run on the reference CSV, then the report has the
   header and all the sections of [Report](#report), with no `—` in the `Generation`, `Single queries` and
   `Batch` sections; and when it is run again, with no changes in the code between the two, with `-Baseline`
   pointing to the data table of the first, then no row of `Baseline` says `outside tolerance`, with the KM5
   tolerance or the one set by its widening for machine noise.
3. **AC-KM1** (KM6) — Given a benchmark in which one query app returns in one query a number of results
   different from the other, when it ends, then that query says `DIFFERENT`, the `Result` line says
   `FAIL: 0 of <n> runs failed, 1 of 11 queries returned different results`, the other sections have their
   figures and the code is 1.
4. **AC-KM2** (KM6) — Given a run of an application that ends with a code other than 0, when the benchmark
   ends, then `Notes` shows it with its code, the data table has its rows with that code, it does not count in
   the medians, `Result` says `FAIL` and the code is 1; and when all the generations of a pair fail, the
   benchmark does not run queries, `Results` says `NOT RUN` in the eleven and `Result` ends in
   `queries not run`.
5. **AC-KM3** — Given each case of the preliminary checks of
   [Validations and messages](#validations-and-messages), when the benchmark is run, then it shows its message,
   ends with 1 and does not run any application or write a report; and with two cases at once, it shows the
   one with the lower order.
6. **AC-KM4** (KM8) — Given a report, when its data table is read, then it has the header and one row per
   measure of each run, including 0 and the failed ones, with the values of [Data table](#data-table), and each
   median of the report matches the one computed from it; with `-Repetitions 1` and `-Repetitions 2`, the medians
   follow the common rules.
7. **AC-KM5** — Given the reference CSV on the reference machine, when this child is closed, then the first
   report and its data table are committed to the repository with `Result: PASS`, and the README, the script
   help, the Linux guides and H5 have what [Documentation](#documentation) asks for.
8. **AC-KM6** — Given the repository on Windows and on Linux (Ubuntu 24.04 on WSL2), when the tests are run,
   then all of them pass, and on Windows the four acceptance checks still pass.

## Decisions

All were taken with the user on 2026-10-22.

- **KM1 — Shape of the benchmark**: a PowerShell script, `run-canopy-comparison.ps1`, next to the acceptance
  checks, with `-WorkDir` (required), `-Csv` (the same default as them), `-Repetitions` (default 5),
  `-NoCompression`, `-Baseline <data table>` and `-SkipBuild`. Reason: it reuses the memory measurement, the
  collection of machine data and the preliminary checks of the checks. A Rust benchmark harness was discarded,
  because it measures functions and not whole processes.
- **KM2 — First run and warm**: each query and each batch is run a first time (run 0), which is reported
  separately, and then `R` more times, whose medians are the figures of the report. None is a cold
  measurement: the operating system cache holds the freshly generated files. It narrows "cold and warm" of the
  parent. Reason: emptying that cache on Windows requires administrator permissions; run 0 shows what the first
  query costs after generating, which is not in the medians.
- **KM3 — Repetitions and generation options**: `R` (5 by default) generations of each pair, alternating, each
  in an empty folder, with the temp folder in the work folder and the default compression of each generator;
  with `-NoCompression`, both without compression. Reason: to measure what the operator gets with the default
  options, on the same disk for both, and so that the drift of the machine affects both pairs equally.
- **KM4 — Memory peak, size and process time**: in the generators, the peak shown by their final summary (that
  of AC-M4); in the query apps, the maximum private committed memory that the system records for the process,
  read when it ends and not by sampling, so that a query of a few milliseconds has its peak, as in AC-M8; in the
  batch, that of the whole process. The size of a set is the sum of the file sizes shown by the final summary of
  its generator, so it is that of the publication, before querying it (K6 allows the CanopyDB query app to
  change the size of the internal files). The process time goes from the start to the end of the process, with
  no waits by the benchmark in between. Reason: it is the same measure throughout the module, and the 0.050 s
  minimum of KM5 requires that the benchmark add no delay of its own.
- **KM5 — Tolerance**: a measure is inside the tolerance of another one used as reference if it meets at least
  one of its two conditions: in times, that the difference does not exceed 20 % of the reference or does not
  exceed 0.050 s; in memory, 10 % or 2 MiB; in size, 1 %. The number of results and the records must be equal.
  It applies to each repetition against the median of its series and, with `-Baseline`, to each median against
  that of the baseline. Reason: short times vary a lot in proportion, so they need an absolute minimum. The
  variation between runs had never been measured when these figures were set; the first report measures it. If
  on closing AC-M10 fails because the real variation of the reference machine under no other load exceeds
  these figures, and not because of a defect, the run widens the tolerance of the affected measures until it
  covers the measured variation, notes it in its log and in the closing report, and notes the change in this
  document.
- **KM6 — Different results and failures**: the benchmark keeps measuring, marks the query as `DIFFERENT` or
  the run as failed, and ends with 1. For each query, `Check` is:
  - `NOT RUN` if no queries were run (step 4 of the flow);
  - `INCOMPLETE` if in run 0 of some query app, single or in a batch, the number of results of that query is
    missing (a batch 0 that fails at F7 leaves `INCOMPLETE` from F7 to F11, not the earlier ones);
  - `DIFFERENT` if not, when the number of results is not the same in all the runs that finished well of the
    two query apps, single and in a batch, or runs 0 do not meet the rest of the AC-M2 rule (step 7 of the
    flow);
  - `SAME` otherwise.

  It only stops querying if a pair has no successfully finished generation. Reason: the figures of the other
  queries are still useful for investigating, and a code 1 prevents taking the report as good.
- **KM7 — Inherited limits**: the benchmark does not check acceptance criteria; if a median exceeds the peak
  of AC-M4 (512 MiB, in either generator), the second of AC-C7 (process time of F1, in either query app) or
  the ratio of AC-M8 (the largest of the medians of F3, F4 and the batch of each query app divided by the
  smallest), it notes it in `Notes` without changing the result. Reason: the parent says the report has no
  thresholds; the acceptance checks already verify them.
- **KM8 — Report**: Markdown in English, like those of the checks, in `docs/canopy-db/comparisons/`, with the
  date, the time with seconds and the commit in the name, plus a CSV data table with each measure. Reason: two
  runs of the same commit do not collide (AC-M10), and the table allows comparing without rereading the
  Markdown.
- **KM9 — Systems**: Windows only. Reason: the acceptance checks, whose measurement the benchmark reuses, are
  Windows-only, and K3 asks to compare the two pairs on the same machine, not on several systems.
- **KM10 — Comparison with a previous report**: `-Baseline` receives the data table of another report and adds
  the `Baseline` section, without changing the result or the exit code. Reason: the goal of the benchmark is to
  see what changes with a new version of CanopyDB or a fix of our own, and AC-M10 needs to compare two runs.
  Comparing two reports by hand was discarded.

## References

- Parent document: [canopy-db.prd.en.md](canopy-db.prd.en.md)
- Siblings: [canopy-db-generador.prd.en.md](canopy-db-generador.prd.en.md),
  [canopy-db-lote.prd.en.md](canopy-db-lote.prd.en.md) and [canopy-db-consultor.prd.en.md](canopy-db-consultor.prd.en.md)
- Root document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
- [Linux support PRD](soporte-linux.prd.md) and [acceptance with any CSV
  PRD](aceptacion-cualquier-csv.prd.en.md)
- Findings log: `docs/canopy-db/hallazgos.md`

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 70d459ca | 2026-10-22 10:31 UTC+01:00 | 32 min | 0 min | 32 min | 26 min | 7 min | 2 |

<!-- prd-time: [{"session": "70d459ca-5e0b-4e1d-886c-fe43c945c851", "start": "2026-10-22T09:31:12.503000+00:00", "wall": 1942, "discarded": 0, "agent": 1536, "human": 406, "interventions": 2, "idle_minutes": 10}] -->
