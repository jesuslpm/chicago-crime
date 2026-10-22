# CanopyDB query app — PRD

**Version**: 1.0
**Date**: 2026-10-22
**Parent document**: [Chicago crime store on CanopyDB — PRD](canopy-db.prd.en.md) (v1.0)

> English translation of [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Deliver the third child of the CanopyDB module (order 3 of [Split into child PRDs](canopy-db.prd.en.md#split-into-child-prds)):

- the **CanopyDB query app**, a new application that does the same as the original query app, query batch
  included, but reads the dataset published by the CanopyDB generator;
- the module's **fixed set of queries**, which the comparison benchmark will reuse (K3);
- its **acceptance check**, which also verifies the module criteria that the parent assigns to this child with
  both query apps (AC-M2 and AC-M8).

The CanopyDB query app inherits the contract of the [query app PRD](consultor.prd.md), of the ["starts with"
PRD](startswith.prd.md) and of the [query batch PRD](canopy-db-lote.prd.en.md) (parent decision K1): the same
options, validations, messages, outputs and exit codes, and the rules of the root document, of the [Linux support
PRD](soporte-linux.prd.md) and of the [acceptance with any CSV PRD](aceptacion-cualquier-csv.prd.en.md). This
document does not repeat them. It only sets what the parent assigns to it ("Sets" column of the split): the
name, the fixed set of queries, the wait when a dataset is open by another process (K6), the messages of the
engine errors (K9), the rejection of a dataset from the other engine and of one written with another version of
CanopyDB, and how memory is limited (K8).

It also **extends the list of K1 exceptions** with two changes to the public output that only apply to the
CanopyDB query app: the help (KC1) and the `schema_version` of the JSON (KC6). The original query app does not
change. The annotations it leaves are in [Annotations on closing](#annotations-on-closing).

**Why now**: the comparison benchmark (`canopy-db-comparacion`) needs both query apps and the fixed set of
queries, and the bug hunt (`canopy-db-robustez`) needs a CanopyDB query app to block, cancel and kill.

### Starting state

Verified on 2026-10-22 against the repository and the source code of CanopyDB 0.2.5:

| Aspect | Situation |
|---|---|
| CanopyDB generator | Implemented and accepted (`canopy-db-generador`, 17 of 17 checks). It publishes `manifest.json` and `canopydb/` in the version folder. AC-M4 is met: peak of 219.5 MiB on Windows and 212.2 MiB on Linux, with the CanopyDB memory limited well below its default value of 1 GiB. |
| CanopyDB manifest | `schema_version` is the text `canopydb-2`; `engine` records `canopydb`, the version `0.2.5` and `fork_commit` `null`; `files` lists the files of `canopydb/`. It has no `primary` or `indexes`. |
| Original query app | Implemented and accepted (query app, "starts with" and batch). Options: `--dataset`, `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format`, `--output`, `--batch` and `-h`/`--help`. Faced with a CanopyDB dataset it answers `No se encontró un conjunto de datos válido en <carpeta>.` with code 1 (AC-KG3). |
| Open errors of the original | No pointer, missing version, unreadable manifest or one without the expected shape, or a missing table: `No se encontró un conjunto de datos válido en <carpeta>.` A file that exists but cannot be read: `No se pudo leer el conjunto de datos en <carpeta>.` The technical detail is not shown. |
| Times of the original | The time on the statistics line of a query starts with the dataset already open. That of the final line of the batch covers the whole execution, opening included. |
| JSON output of the original | The `dataset` member has `version` (name of the version folder), `generated_at` and `schema_version`, the latter as a number (`2`). |
| Help of the original | It ends with the exit codes and a paragraph about the batch codes. |
| CanopyDB query app | It does not exist. No document gives it a name. |
| Opening in CanopyDB 0.2.5 | Only one process at a time can have a database open. The time the second one waits is configurable (5 s by default); when it runs out, CanopyDB gives an error. There is no read-only open (H2 and H3). |
| Format version in CanopyDB | The files of the database do not indicate which version of CanopyDB wrote them. The only record is `engine` in the manifest. |
| Reading without changes | The CanopyDB verifier already reads a database without adding files or changing the manifest or the logical content (AC-KG4). |
| Findings log | `docs/canopy-db/hallazgos.md`, with H1–H5 in state "Detectado". H5 (performance): CanopyDB has no bulk load of sorted entries. |
| Acceptance check of the original query app | Windows script that checks AC-C1 to AC-C18, AC-S1 to AC-S12 and the batch criteria against direct filtering of the CSV, and measures the memory peak from outside. Recorded figures: counting `THEFT`, 14.5 MiB; counting a rare type, 7.5 MiB; CSV file with all the thefts, 16.6 MiB. **The peak of a batch has never been measured.** |
| Fixed set of queries | Only the K3 sketch exists. There is no concrete list. |

**Product gaps**: none open. Each point of the parent's "Sets" column remains as a rule in this document or in
[Left to the implementation](#left-to-the-implementation).

## Actors

- **Analyst**: the one from the query app. Uses the CanopyDB query app just like the original.
- **Developer**: runs the acceptance check and registers the findings that appear.

## Flow

### Query

It follows flow B of the parent and, by K1, the flows of the query app, of "starts with" and of the batch. This
delivery specifies:

1. The analyst runs `crime-canopy-query` with the same options as the original, on their own or with `--batch`.
   The help is that of the original with three changes (KC1): the name of the application; the description,
   which says `Consulta el conjunto de datos de delitos de Chicago (base CanopyDB) publicado por el generador CanopyDB.`;
   and one more paragraph at the end of the help, after the one about the batch codes:
   `Si otro proceso tiene abierto el conjunto de datos, la consulta espera hasta 30 s antes de rechazarse con 1.`
2. It checks the command line, the conditions and, with `--batch`, the batch file, like the original and in the
   same order, without opening the dataset.
3. It locates the active version through the pointer, like the original, and checks, in this order and without
   opening the database: the `schema_version` of the manifest (KC3), the shape of the manifest (KC3), the
   CanopyDB version (KC4) and that all the files listed by `files` exist. The first failure rejects with its
   message.
4. It opens the database. If any other process has it open (another CanopyDB query app, the CanopyDB verifier or
   another program), it warns as soon as it detects it and waits up to 30 s (KC2): if it is released, it
   continues normally; if not, it rejects without results with code 1.
5. It checks that the database has the six tables of the manifest; it never creates a database, a table or a
   file.
6. It runs the query, or those of the batch one after another, with the same rules, output and statistics as the
   original. As in the original, the time on the statistics line of each query does not include the opening
   (nor, therefore, the wait), and that of the final line of the batch does. A batch keeps the database open
   until it finishes.
7. Ctrl+C at any moment, also during the wait or the opening of the database, ends like any cancellation of the
   original: `Consulta cancelada.` and code 130.
8. Faced with any other engine error, it shows the closest existing message and a line with the detail on the
   error output (KC5).
9. It does not change the queryable content or the manifest, and does not create files in the dataset folder; it
   may change the content or the size of the engine's internal files (K6).
10. Its memory peak does not grow with the number of results or with the number of queries in the batch (K8).

### Acceptance check

1. The developer runs `run-canopy-query-acceptance.ps1` (KC8) on a CSV and a work folder.
2. The check generates the dataset with both generators and runs with the CanopyDB query app the checks of the
   original query app's check, with the substitutions of AC-M1, plus those of this document. Where the
   original's uses the independent verifier, this one uses the CanopyDB verifier.
3. It runs the fixed set of queries (KC7) with both query apps, on their own and in a batch, compares their
   results (AC-M2) and measures the memory peak of each query app (AC-M8).
4. It writes a report in the format of the original and ends with 0 or 1.

## Fixed set of queries

Decision KC7. Eleven queries, written as lines of a batch file of the [query batch PRD](canopy-db-lote.prd.en.md),
in this order:

| Id | What it measures | Line |
|---|---|---|
| F1 | By ID | `--where ID=634 --format json` |
| F2 | By shared case number | `--where "Case Number=<número>" --format json` |
| F3 | Count of a large primary type | `--where "Primary Type=THEFT" --count` |
| F4 | Count of a rare primary type | `--where "Primary Type=RITUALISM" --count` |
| F5 | Date range of one month | `--range Date=2019-08-01..2019-08-31 --count` |
| F6 | Eleven-year range with a primary type | `--where "Primary Type=THEFT" --range Date=2005-01-01..2015-12-31 --count` |
| F7 | Two indexed conditions | `--where "Primary Type=THEFT" --where District=008 --count` |
| F8 | Two indexed and one unindexed filter | `--where "Primary Type=THEFT" --range Date=2019-08-01..2019-08-31 --where Arrest=true --count` |
| F9 | Prefix | `--startswith "Case Number=JA" --count` |
| F10 | Listing with limit | `--where "Primary Type=THEFT" --limit 1000 --format csv` |
| F11 | Full listing | `--where "Primary Type=THEFT" --format csv` |

`<número>` is the first shared case number of the CSV, the same one that the original query app's check uses for
AC-C6. The other values are those that check already uses (ID 634 is the first of the reference extract; August
2019, `RITUALISM`, `008` and `Arrest=true`, its queries of AC-C4, AC-C5, AC-C9 and AC-C13) and, as the starting
state of the acceptance with any CSV PRD says, a later download almost surely keeps them.

## Validations and messages

All the validations and messages of the root document, of the query app, of "starts with", of the batch and of
the Linux support PRD apply, without changing a word. This delivery only sets the cases specific to CanopyDB, in
the order in which they are checked (step 3 onwards of the flow), after the command line, the conditions and the
batch file:

| Situation | Rule | Message to the user | Code |
|---|---|---|---|
| The manifest exists but cannot be read | Rejects without opening the database, like the original, with no detail line | `No se pudo leer el conjunto de datos en <carpeta>.` | 1 |
| No pointer, missing version, manifest that is not valid JSON, or a `schema_version` that is not a text of the form `canopydb-` followed only by digits; includes any SSTable dataset, of any schema (KC3) | Rejects without opening the database | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| `schema_version` `canopydb-<digits>` different from `canopydb-2` (KC3) | Rejects without opening the database | `El conjunto usa el esquema v<cifras> y este consultor admite v2. Regenera el conjunto.` | 1 |
| `canopydb-2` manifest without the shape of the CanopyDB generator (KC3) | Rejects without opening the database | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| `engine.version` or `engine.fork_commit` different from those of the query app (KC4) | Rejects without opening the database | `El conjunto se escribió con CanopyDB <versión del conjunto> y este consultor usa CanopyDB <versión propia>. Regenera el conjunto.` | 1 |
| A file listed by `files` is missing (or is a folder), or `canopydb/` is missing | Rejects without opening the database or creating anything | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| Another process has the database open (KC2) | Warns once on the error output, as soon as the first attempt finds it open, even if it is released right away, and waits; if it is released before 30 s, continues with no further messages | `Aviso: otro proceso tiene abierto el conjunto de datos en <carpeta>; se esperará hasta 30 s.` | — |
| The database is still open by another process 30 s after the warning (KC2) | Rejects without results, between 30 and 35 s after the warning | `El conjunto de datos en <carpeta> sigue abierto por otro proceso. Inténtalo de nuevo cuando termine.` | 1 |
| Ctrl+C during the wait or the opening | Ends like any cancellation, in less than 1 s | `Consulta cancelada.` | 130 |
| The open database lacks any of the six tables | Rejects; never creates a table | `No se encontró un conjunto de datos válido en <carpeta>.` | 1 |
| Any other engine error when opening or reading the database, including an unexpected engine failure and CanopyDB being unable to write to its internal files (KC5) | Rejects or aborts like the original faced with a dataset that can no longer be read: no misleading partial output or half-written output file, with code 1 and never with an abnormal termination code | `No se pudo leer el conjunto de datos en <carpeta>.` | 1 |
| Detail of that engine error (KC5) | A single line on the error output, right before the message; every carriage return and every line feed of the engine text is replaced by a space, and the rest is left as is, even if empty. It only accompanies that `No se pudo leer…` | `Detalle del error de CanopyDB: <texto del error tal como lo da CanopyDB>` | — |

Clarifications:

- **Shape of the manifest** (KC3): as in the original, all the fields of the CanopyDB manifest of the [CanopyDB
  generator PRD](canopy-db-generador.prd.en.md#manifest) are mandatory and with their type, and unknown ones
  are tolerated. In particular, `engine.name` is `canopydb`; `engine.version` is a non-empty text;
  `engine.fork_commit` exists and is `null` or a non-empty text; and `tables` has exactly the six tables, with
  their names and in their order; each entry of `files` is a path relative to the version folder that starts
  with `canopydb/`, separated with `/`, without `.` or `..` parts. `canopydb/` may have files that `files` does
  not list: they are tolerated.
- **`<digits>`** is shown as it is written: `canopydb-02` gives `v02`. The comparison with `canopydb-2` is
  exact, with uppercase and lowercase as they are, so `CANOPYDB-2` and `canopydb-` are invalid datasets. The
  message is the incompatible schema one of the root document, with the version that the query app admits today
  (v2, decision C1 of the query app).
- **Versions**: `<versión del conjunto>` and `<versión propia>` are `engine.version` as written, followed by
  ` (fork <commit>)` with the full commit when `engine.fork_commit` is not `null`. The comparison is exact.
- **`<carpeta>`** is that of `--dataset`, as in the messages of the original.
- **Output file**: all the rejections in the table, except an engine error in the middle of a query, happen
  before the `--output` file is prepared, so a file that already existed stays as it was, as in the original.
- **With `--batch`**, all these cases happen when opening the dataset, before the header of the first query, so
  they come out without the `Línea <n>: ` prefix and without a header (step 5 of the order of the batch checks),
  and Ctrl+C during the wait only leaves `Consulta cancelada.`. An engine error during a query comes out under
  its header, like any failure of a batch query.

## Output

- **Console, error output, CSV, output file and batch**: those of the original, unchanged, with the messages of
  [Validations and messages](#validations-and-messages).
- **JSON**: that of the original, with a single difference (KC6): in the `dataset` member, `schema_version` is
  the text of the manifest, `"canopydb-2"`, instead of a number. `version` and `generated_at` do not change.
  Abbreviated example:

  ```json
  {"query":{…},"dataset":{"version":"dataset-20261022-101500-4242","generated_at":"2026-10-22T10:15:00+02:00","schema_version":"canopydb-2"},"results":[…]}
  ```

### Acceptance check report

`run-canopy-query-acceptance.ps1` (KC8) accepts the parameters of the original query app's check and, in
addition, `-NotMetByCanopyDb`, with the form, the preliminary checks and the treatment of `NOT MET` of the
CanopyDB generator check (KG10), with these differences:

- `AC-C7`, `AC-S8` and `AC-M8` can be associated, each one only once; associating another criterion gives the
  reason `only AC-C7, AC-S8 and AC-M8 can be associated`, and repeating one, `<criterio> is already associated`;
- an associated row gives `NOT MET` only when all its runs ended with 0 and with the expected result, and the
  only thing that fails is the figure it measures: the time of AC-C7 or the memory ratio of AC-S8 and of AC-M8,
  all of the CanopyDB query app; any other failure gives `FAIL`. The row
  `AC-M8 (SSTables)` cannot be associated: if it fails, it gives `FAIL` (K8: it is a failure of the project).

Its report is a Markdown file like that of the original, in English, titled
`Acceptance of the CanopyDB crime query app`, and ends with 0 if the global result is `PASS` and with 1 if it
is `FAIL`. Its rows:

- those of the original, with their labels, except `AC-C13`, which becomes `AC-M8`, and the second half of
  `AC-C18` (dataset with schema 1), which now checks the rejection of an SSTable dataset of schema 2;
- `AC-M2-F1` to `AC-M2-F11`, one per query of the fixed set, with the number of results of each query app;
- `AC-M5`, `AC-M6` and `AC-M7`, the latter with the CanopyDB query app (the original is covered by the batch);
- `AC-M8` for the CanopyDB query app and `AC-M8 (SSTables)` for the original, each with its three peaks and its
  ratio;
- `AC-KC2` to `AC-KC7`.

The global result follows the rule of the CanopyDB generator check, generalized to several `NOT MET` rows:
with no `FAIL`, `PASS: all <n> checks passed` if there is no `NOT MET`, and
`PASS: <p> of <n> checks passed, <k> not met by CanopyDB (<hallazgos>)` if there are, with the findings of the
`NOT MET` rows in the order of the rows, separated by `, ` and without repeating; with any `FAIL`,
`FAIL: <f> of <n> checks failed`.

## Documentation

- **Help** of the CanopyDB query app, with the changes of step 1 of the flow.
- **README**: a section on the CanopyDB query app (what it is for, how it is run, how it differs from the
  original: the wait when a dataset is open, the rejection of datasets from another CanopyDB version and the
  `schema_version` of the JSON), on its acceptance check and on the fixed set of queries, with the CanopyDB
  memory configuration used.
- **Linux guides**: how to build and run the CanopyDB query app.

## Reuse

- **Original query app**: options, validations, messages, query resolution (C10), formats and output file,
  batch, channels and exit codes, cancellation. Only where the tables are read from changes. Its code can be
  reorganized to share it, without changing its behavior (KC9).
- **Shared dataset contract**: key and value encoding, indexes and pointer to the active version.
- **CanopyDB verifier**: its way of reading the database without changing it, its reading of the CanopyDB
  manifest and its comparison mode (AC-M6, AC-KC4).
- **Acceptance check of the original query app**: its cases and its memory measurement from outside, with the
  changes of this document.
- **Acceptance check of the CanopyDB generator**: its check that reading adds no files (AC-KG4) and its
  treatment of `-NotMetByCanopyDb`.

## Out of scope

- The comparison benchmark and its report (`canopy-db-comparacion`) and the targeted bug hunt
  (`canopy-db-robustez`): among others, killing a query app or a batch halfway, several query apps waiting at
  the same time and the order in which they get in, queries during publication beyond AC-M7, and deleting a
  version that a CanopyDB query app has open.
- An option to change the wait time: the contract has no new options (K1).
- Reading a dataset written with another CanopyDB version or converting it: it is regenerated (KC4).
- A dataset taken to a machine with another byte order, and datasets in network folders or on Windows disks
  mounted in Linux: neither the wait of KC2 nor a specific message is guaranteed.
- Detecting a damaged database that CanopyDB reads without giving an error.
- A read-only mode in CanopyDB and reading by several processes at once (H2 and H3).
- Querying datasets in read-only folders or media: CanopyDB needs to write to its internal files, so they are
  rejected as an engine error (KC5).
- Changing the CanopyDB generator, its manifest or the CanopyDB verifier.
- Changing the behavior of the original query app, except fixing it if it does not meet AC-M8 (K8).
- From flow E of the parent, everything except registering in the findings log.
- A time target for the queries, except the inherited AC-C7.
- Associating criteria other than AC-C7, AC-S8 and AC-M8 with `-NotMetByCanopyDb`.
- Running the acceptance check and measuring AC-M8 on Linux.

## Left to the implementation

- The CanopyDB configuration in the query app, including how its memory is limited (K8), as long as the
  contract, K6 and AC-M8 are met. The README, the closing report and, later, the comparison report state it.
- How the opening is waited for and retried, as long as the times in
  [Validations and messages](#validations-and-messages) are met.
- The text of the evidence of each row of the report, as long as `AC-M2-F<n>` shows the number of results of
  each query app and the `AC-M8` rows their three peaks in MiB and the ratio.
- How the tests provoke the engine errors of AC-KC5, as long as it adds no visible option or changes a normal
  run.
- Where the file of the fixed set of queries lives, as long as the comparison benchmark can reuse it.
- Where the standard output of the queries goes while the check measures their memory.

## Acceptance criteria

The figures correspond to the reference CSV, with the same caveat as in the parent. An inherited criterion that
CanopyDB cannot meet is treated according to K10, with the association of KC8. AC-KC1 is checked with the rows
of the original; AC-KC2 to AC-KC7 and the module criteria are rows of their own in the acceptance report;
AC-KC8 and AC-KC9 are checked on closing and recorded in the closing report. A query with `--count` only gives
a number, so in it "the same result" is the same count; the set of records is compared in those that list.

1. **AC-KC1** (AC-M1, query app part) — Given the reference CSV generated with the CanopyDB generator, when the
   acceptance check is run, then the CanopyDB query app meets AC-C1 to AC-C18, AC-S1 to AC-S12 and the batch
   criteria that the original's check verifies, with the same figures; AC-M8 replaces AC-C13, and the
   rejection of an SSTable dataset of schema 2 replaces the second half of AC-C18. With the reference extract
   AC-A1 is met; with a later download, AC-A2.
2. **AC-M2** (KC7) — Given the same CSV generated with both generators, when each query of the fixed set is
   run with both query apps, then the number of results is equal in all eleven; in F1, F2 and F11 the set of
   returned records is the same, regardless of order; and F10 returns with each query app 1,000 records, all
   from the result of F11.
3. **AC-M5** (K6, KC2) — Given a CanopyDB query app batch in progress that lasts more than 45 s (repeating F11
   as many times as needed), when a second CanopyDB query app opens the same dataset, then it shows the wait
   warning on the error output, is rejected with its message and code 1 between 30 and 35 s after the warning,
   and the batch ends normally. With a batch that ends before 30 s, the second one shows the warning and
   answers normally. If the analyst presses Ctrl+C during the wait, the second one ends with
   `Consulta cancelada.` and code 130 in less than 1 s.
4. **AC-M6** (K6) — Given a freshly published CanopyDB dataset and a copy of its output folder made before
   opening it, when all the queries of the fixed set are run, on their own and in a batch, then the version
   folder has the same file names as the copy, with no new one, the manifest is identical and the comparison
   mode of the CanopyDB verifier between the folder and the copy gives the six tables equal.
5. **AC-M7** (K7) — Given the fixed set of queries, when it is run in a batch with the CanopyDB query app, then
   each query returns the same number of results as on its own, F1, F2 and F11 the same set of records, and F10
   1,000 records of the result of F11. Given a CanopyDB query app batch with the fixed set that has already run
   F1, when the CanopyDB generator publishes a new version, then the JSON of F2 indicates the same version as
   that of F1, the one the batch opened, and the batch ends normally.
6. **AC-M8** (K8) — Given, with each query app separately, the count F3, the count F4 and a batch with the whole
   fixed set, when the memory peak of each run is measured, then the highest of the three peaks does not exceed
   3 times the lowest. The peaks of one query app are not compared with those of the other. The report records
   the six peaks.
7. **AC-KC2** (KC3) — Given an SSTable dataset of schema 2 and another of schema 1, and CanopyDB datasets with
   the manifest changed to `schema_version` `canopydb-3`, `canopydb-02`, `canopydb-x`, `canopydb-`,
   `CANOPYDB-2`, the number 2, the text `"2"` and without `schema_version`;
   without `tables`; with five tables; with the tables in another order; with a different `engine.name`;
   without `engine.fork_commit`; and with an unknown field added, when they are queried with the CanopyDB query
   app, then each one gives the message of its row in [Validations and messages](#validations-and-messages) and
   code 1, except the one with the unknown field, which answers normally; and no version folder changes.
8. **AC-KC3** (KC4) — Given a CanopyDB dataset with `engine.version` changed to `0.2.4` in the manifest, and
   another with a 40-character `fork_commit`, when they are queried, then they are rejected with the message of
   another version, with the versions written as [Validations and messages](#validations-and-messages) says,
   and code 1, without opening the database: `canopydb/` does not change.
9. **AC-KC4** — Given a CanopyDB dataset without `canopydb/`, another without one of the files listed by
   `files`, another in which one of those paths is a folder, and another whose database lacks one of the six
   tables, when they are queried, then they are rejected with `No se encontró un conjunto de datos válido en <carpeta>.`,
   with no detail line, and code 1; the version folder has no new file, and in the last one the database still
   lacks that table. With one more file in `canopydb/` that `files` does not list, the query answers normally.
10. **AC-KC5** (KC5) — Given an engine error provoked in a test (when opening, halfway through a CSV listing
    with `--output` and without it, an unexpected engine failure and an error text with line breaks), when
    queried, on its own and in a batch, then the message is `No se pudo leer el conjunto de datos en <carpeta>.`,
    the error output has right before it a single line `Detalle del error de CanopyDB:` with the error text on
    one line, the code is 1, no half-written output file remains and the standard output is that of the
    original faced with a dataset that can no longer be read. Given on Windows a dataset with the files of
    `canopydb/` marked read-only, when queried, then it is rejected with that same message, its detail and
    code 1, and the files do not change.
11. **AC-KC6** (KC6) — Given a JSON query, when it is run with the CanopyDB query app, then the output has the
    shape of the original and `dataset.schema_version` is `"canopydb-2"`.
12. **AC-KC7** (KC8) — Given the check with `-NotMetByCanopyDb AC-M8=<hallazgo>` and a CanopyDB query app whose
    ratio exceeds 3, when it ends, then the `AC-M8` row says `NOT MET` with its finding, the global result is
    `PASS: <p> of <n> checks passed, 1 not met by CanopyDB (<hallazgo>)` if there is no other `FAIL` and the
    code is 0; with two `NOT MET` rows the result says `2 not met` and their two findings; if the one that
    exceeds the ratio is the original query app, the `AC-M8 (SSTables)` row says `FAIL` and the code is 1; and
    an association with another criterion ends with 1 without running anything, with the error in
    [Acceptance check report](#acceptance-check-report).
13. **AC-KC8** (KC9) — Given the repository on Windows and on Linux (Ubuntu 24.04 on WSL2), when the tests are
    run, then all pass, including those of the CanopyDB query app; the tests of the original applications that
    existed before still exist and pass without changing what they check; and on Windows the acceptance check
    of the original query app still passes.
14. **AC-KC9** (KC1) — Given the help and the README, when they are read, then the help of the CanopyDB query
    app is that of the original with the three changes of step 1 of the flow, and the README has the section
    in [Documentation](#documentation), with the fixed set of queries.

## Decisions

KC2, KC4, KC6, KC7 and KC8 were taken with the user on 2026-10-22; the others derive from the ancestry and
from the decisions of the CanopyDB generator.

- **KC1 — Name and help**: `crime-canopy-query`, with the pattern of KG1. The help changes the description and
  adds the wait paragraph; it is an exception to K1, like the help of the CanopyDB generator.
- **KC2 — Wait when a dataset is open** (K6): fixed, 30 s, with no option to change it; a warning as soon as it
  is detected and its own message when it runs out, with code 1. Reason: a single query usually takes a few
  seconds, so 30 s almost always covers the other process's query without leaving the analyst waiting blindly;
  the warning explains the pause. The 5 s of CanopyDB (they fail with any large foreign batch or listing) and
  60 s were discarded.
- **KC3 — Dataset from another engine or another schema**: only `schema_version` `canopydb-2` with the shape of
  the CanopyDB generator manifest is accepted. A `canopydb-<digits>` with another number uses the original's
  incompatible schema message with those digits; any other value, including a number (an SSTable dataset), is
  an invalid dataset, as the parent sets.
- **KC4 — Another CanopyDB version**: it is rejected before opening the database, with a new message, if
  `engine.version` or `engine.fork_commit` are not exactly those of the query app. Reason: CanopyDB files do
  not indicate their version, so opening a database from another version could give false results with no
  error, and the message tells the analyst that regenerating is enough. The parent leaves to this child how it
  is rejected. The existing invalid dataset message, which does not say what to do, and trying to open the
  database were discarded.
- **KC5 — Engine errors** (K9): any CanopyDB error when opening or reading, except the wait of KC2, uses
  `No se pudo leer el conjunto de datos en <carpeta>.`, the original's message for a file that exists and
  cannot be read, and carries a detail line with the text of KG7. What the query app checks by itself (manifest,
  files of `files` and tables) uses `No se encontró…` with no detail, like the original. Reason: the query app
  has no log, so the detail goes through the warnings channel (C5).
- **KC6 — `schema_version` in the JSON**: the text of the manifest, `"canopydb-2"`. It is an exception to K1
  that only applies to the CanopyDB query app. Reason: C9 asks that the result document which data was
  queried, and the text also says the engine. Showing `2`, which kept the output identical with no exception,
  was discarded.
- **KC7 — Fixed set of queries**: the eleven in [Fixed set of queries](#fixed-set-of-queries), which cover the
  classes of K3, with a large listing (F11), and the memory cases of AC-M8, with values that the original query
  app's check already uses. Leaving F11 out was discarded.
- **KC8 — Acceptance check**: `run-canopy-query-acceptance.ps1`, with the pattern of KG12, and the `NOT MET`
  state of KG10 for the three inherited criteria that K10 foresees CanopyDB may prevent: the time by ID
  (AC-C7) and the memory (AC-S8 and AC-M8), only of the CanopyDB query app. It extends the list of KG10, which
  only admitted AC-M4. Admitting only AC-M8 was discarded.
- **KC9 — Original applications**: like KG11, the code of the original query app can be reorganized to share
  it, without changing its behavior, its messages or its outputs. If the original does not meet AC-M8, it is
  fixed (K8).

## Annotations on closing

On closing this child, these are noted, without rewriting its history: the [query app PRD](consultor.prd.md)
(which has no translation) and the [CanopyDB PRD](canopy-db.prd.en.md) with its translation. The first two are
those required by [Criteria and annotations per child](canopy-db.prd.en.md#criteria-and-annotations-per-child);
the other two are added because of KC6:

- **Query app**, in "Query app caches or auxiliary indexes: the query app never writes to the dataset folder"
  (Out of scope): *Annotation of <date>: the CanopyDB query app may write to the engine's internal files,
  without changing the queryable content or the manifest or creating files (decision K6 of
  [canopy-db.prd.md](canopy-db.prd.en.md)); the original query app does not change.*
- **Query app**, in C7: *Annotation of <date>: it also applies to the CanopyDB query app and to the batch, and
  is checked with the maximum ratio of 3 of AC-M8 (decision K8 of [canopy-db.prd.md](canopy-db.prd.en.md),
  [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*
- **Query app**, in C9: *Annotation of <date>: in the CanopyDB query app, `schema_version` is the text of the
  manifest, `"canopydb-2"` (decision KC6 of [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)); the
  original query app does not change.*
- **CanopyDB**, in the common contract (the list of K1 exceptions): *Annotation of <date>: the CanopyDB query
  app adds two exceptions, its help and the `schema_version` of its JSON output (decisions KC1 and KC6 of
  [canopy-db-consultor.prd.md](canopy-db-consultor.prd.md)).*

## References

- Parent document: [canopy-db.prd.en.md](canopy-db.prd.en.md)
- Documents whose contract the new application inherits (K1): [consultor.prd.md](consultor.prd.md),
  [startswith.prd.md](startswith.prd.md) and [canopy-db-lote.prd.en.md](canopy-db-lote.prd.en.md)
- Sibling: [canopy-db-generador.prd.en.md](canopy-db-generador.prd.en.md)
- Root document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
- [Linux support PRD](soporte-linux.prd.md) and [acceptance with any CSV
  PRD](aceptacion-cualquier-csv.prd.en.md)
- CanopyDB: https://github.com/arthurprs/canopydb and https://docs.rs/canopydb

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 71fbd401 | 2026-10-22 06:10 UTC+01:00 | 23 min | 0 min | 23 min | 19 min | 4 min | 5 |

<!-- prd-time: [{"session": "71fbd401-ac30-467d-8555-d070e19606b9", "start": "2026-10-22T05:10:44.656000+00:00", "wall": 1399, "discarded": 0, "agent": 1156, "human": 242, "interventions": 5, "idle_minutes": 10}] -->
