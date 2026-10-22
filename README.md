# Chicago crime dataset on SSTables

Rust tools that turn the City of Chicago crime history (≈8.7 million records, ≈2.4 GB of CSV) into a
sorted, immutable dataset of LevelDB-format tables that can be queried without a database. Keys and
values are encoded as FoundationDB tuples. The project is also a hands-on exercise in Rust, external
sorting under a fixed memory budget, and product-driven development.

| Application | Status | What it does |
|---|---|---|
| Generator (`crime-generator`) | Done and verified | Reads the CSV once and publishes a primary table keyed by ID, five secondary indexes and a manifest, with a peak memory of about 93 MiB (limit 512 MiB). |
| CanopyDB generator (`crime-canopy-generator`) | Done | The same generator, options and messages, publishing the dataset in a [CanopyDB](https://github.com/arthurprs/canopydb) base instead of LevelDB tables, to compare the two engines ([module PRD](docs/prd/chicago-crime/canopy-db.prd.md)). See [CanopyDB generator](#canopydb-generator). |
| Query app (`crime-query`) | Done and verified | Answers lookups by ID, case number, indexed values and date ranges, with filters on the other fields, as a table, CSV or JSON, without loading the dataset into memory; one query per run, or a batch of queries from a file. |
| CanopyDB query app (`crime-canopy-query`) | Done | The same query app over the CanopyDB generator's dataset, with the same answers. See [CanopyDB query app](#canopydb-query-app). |

## Get the dataset

The data is the public **Crimes – 2001 to Present** dataset of the City of Chicago:

- Dataset page: <https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2>
- Direct CSV download: <https://data.cityofchicago.org/api/views/ijzp-q8t2/rows.csv?accessType=DOWNLOAD>

Save it as `chicago-crime-data.csv` at the repository root. That name is ignored by Git, and the
acceptance script expects it there.

```powershell
curl.exe -L -o chicago-crime-data.csv "https://data.cityofchicago.org/api/views/ijzp-q8t2/rows.csv?accessType=DOWNLOAD"
```

The portal updates the dataset daily, so a new download has more rows than the reference extract used
to write and verify this project. That extract was published on 2026-10-10 and has 8,656,904 records,
2,406,401,982 bytes, SHA-256 `5cb5fa41c930ece2e91f683d894c815d46333d2a5372b8df5ae40b6e8a6ae35d`. The
exact figures in the PRDs and the acceptance criteria refer to that extract.

## Methodology: PRD + ODD

Every change starts with a product document and is then built with Organic Driven Development:

```text
Root PRD (product) → module PRD → … → change PRD → ODD feature document
  → tasks: test first, one commit each, automated review → acceptance on real data → merge into main
```

- **PRD first.** PRDs form a hierarchy whose depth each project chooses. The root PRD describes the whole
  product; every other PRD (a module, or a change to a module or to another change) names its direct
  parent and adds only what it needs. A change is read together with its whole ancestry, up to the root:
  for example, the [startswith PRD](docs/prd/chicago-crime/startswith.prd.md) builds on the query app PRD,
  which builds on the root PRD. A PRD is written in business language: objective,
  starting state verified against the code and data, flow, validations and messages, decisions with
  their reasons, explicit out-of-scope items and acceptance criteria. The project skill in
  [`.claude/skills/prd`](.claude/skills/prd/SKILL.md) holds the template.
  - [Root PRD](docs/prd/chicago-crime/chicago-crime.prd.en.md) ([Spanish original](docs/prd/chicago-crime/chicago-crime.prd.md)): the generator and the query app.
  - [Generator PRD](docs/prd/chicago-crime/generador.prd.en.md) ([Spanish original](docs/prd/chicago-crime/generador.prd.md)): the generator, acceptance criteria AC-G1 to AC-G10.
  - [Query app PRD](docs/prd/chicago-crime/consultor.prd.md) (Spanish): the query app, acceptance criteria AC-C1 to AC-C18.
  - [Linux support PRD](docs/prd/chicago-crime/soporte-linux.prd.md) (Spanish): building, testing and running on Linux, acceptance criteria AC-L1 to AC-L9.
  - [Acceptance with any CSV PRD](docs/prd/chicago-crime/aceptacion-cualquier-csv.prd.en.md) ([Spanish original](docs/prd/chicago-crime/aceptacion-cualquier-csv.prd.md)): acceptance scripts that also work with a newer download, acceptance criteria AC-A1 to AC-A4.
- **ODD (Organic Driven Development).** The PRD becomes a feature document with an objective,
  implementation decisions and a task checklist. That document is the single place for progress and
  evidence: [`odd/tasks/crime-generator.md`](odd/tasks/crime-generator.md),
  [`odd/tasks/crime-query.md`](odd/tasks/crime-query.md) and
  [`odd/tasks/soporte-linux.md`](odd/tasks/soporte-linux.md). Each task:
  - explores the code first;
  - writes a failing test before the implementation (RED → GREEN);
  - closes with one Conventional Commit on the feature branch;
  - records its evidence in the feature document.

  An automated review (Gentle AI's receipt-driven review) checks the commits. Its findings become new
  tasks, for example T2a and T5a in the generator. The feature is merged into `main` only after the full
  acceptance passes.

## Requirements

| Tool | Needed for | Version |
|---|---|---|
| Rust toolchain | Building and testing the generator and the query app | 1.85 or later (edition 2024) |
| Python | The worktree guard hook, the scripts of the `closing-report` and `prd` skills, and the speaker-note audio generator | 3.10 or later |
| [uv](https://docs.astral.sh/uv/) | Only the [speaker-note audio](#speaker-note-audio) generator | Verified with 0.10.2 |

Python is run as `python`, or as `python3` where there is no `python` command, as on most Linux
distributions. The hook and the two skills use only the standard library, so they need nothing else; without
any Python the worktree guard does not run and says so on every tool call. uv runs the audio generator in an
isolated environment with locked dependencies, so nothing is installed in the system Python. All the Python
tests pass on Python 3.10 and 3.14.

## Build

You need the Rust toolchain from [Requirements](#requirements). The project was built and verified with Rust
1.99 on Windows 11, the reference platform, and on Ubuntu 24.04 under WSL 2 (see
[Build and test on Linux](#build-and-test-on-linux)).

```powershell
cargo build --release
cargo test --workspace
```

The binaries end up in `target\release\`.

## Build and test on Linux

The same commands build the project and pass every test on Linux, with the binaries in
`target/release/` and no `.exe` extension. Both applications behave as on Windows, with the same
options and messages; the generator reads its memory peak from the Linux kernel instead of Windows,
and the PowerShell acceptance scripts still run only on Windows. On a Windows machine, the
[WSL guide](docs/linux/wsl.md) covers getting a Linux distribution, installing the toolchain, keeping
the code and the datasets in the Linux file system rather than under `/mnt/c`, and copying the
reference CSV, which is not in the repository and is only needed to run the applications, not the
tests.

On a machine that runs Linux directly, the [native Linux guide](docs/linux/native.md) gives the
toolchain packages for the common distributions and the build, test and run commands. It also lists
what differs from Windows: how the memory figure is measured, how a run tells the leftovers of an
interrupted generator from those of a running one, and which checks still need Windows.

## Run the generator

```powershell
target\release\crime-generator.exe --input chicago-crime-data.csv --output C:\datasets\chicago
```

| Option | Meaning |
|---|---|
| `--input <ARCHIVO>` | The crime CSV to read. Required. |
| `--output <CARPETA>` | Folder where the dataset is published. Required. |
| `--temp-dir <CARPETA>` | Folder for the temporary sort files. Default: the system temp folder. |
| `--log <ARCHIVO>` | Run log. Default: next to the output folder, named `<carpeta>-<AAAAMMDD-hhmmss>.log`. It must not exist yet. |
| `--overwrite` | Replace a dataset that already exists in the output folder. Without it, the run is refused. |
| `--no-compression` | Write the tables without Snappy compression. |
| `-h`, `--help` | Show the help. |

The command line, the console and the log are in Spanish. A full run on the reference extract takes
about 65–75 seconds and writes about 1.1 GB.

- **Invalid rows** are written to the log and skipped; they never stop the run.
- **The log** is one file per run, kept outside the dataset so it survives a failed run. It ends with a
  summary: valid and rejected records, duration, peak memory, the published files and their sizes.
- **Ctrl+C** cancels the run safely.

| Exit code | Meaning |
|---|---|
| 0 | The dataset was published, even with rejected rows or a memory warning. |
| 1 | The run aborted; the previous dataset, if any, is intact. |
| 2 | The command line is not valid; nothing was done. |
| 130 | Cancelled with Ctrl+C; the previous dataset, if any, is intact. |

### What it publishes

```text
<output folder>/
    CURRENT                               name of the active version folder
    dataset-<YYYYMMDD>-<HHMMSS>-<pid>/    the active version
        primary.sst                       key (id); value: the stored fields in CSV order, as in the CSV
        index-date.sst                    one table per index; key (value or Nil, id)
        index-primary_type.sst
        index-district.sst
        index-community_area.sst
        index-case_number.sst
        manifest.json                     schema version (2), source CSV with SHA-256, counts, tables
```

The text indexes (primary type, district and case number) hold their values in lowercase, so a
lookup does not depend on how the CSV writes them; the primary table keeps every field exactly as
the CSV has it. Lowercasing never pads or strips codes: `008` stays `008`.

A run builds a complete new version folder and then switches `CURRENT` in one step. A reader therefore
always finds a complete dataset. A failed, cancelled or killed run never changes the published one,
and the next run cleans up whatever an interrupted run left behind. The same CSV always produces
byte-identical tables.

The current format is **schema 2**. A dataset published by an older generator (schema 1, whose text
indexes are not in lowercase) is refused by the query app: regenerate it.

## CanopyDB generator

`crime-canopy-generator` is the CanopyDB counterpart of the generator ([PRD](docs/prd/chicago-crime/canopy-db-generador.prd.md)).
It shares the generator's whole pipeline, so its options, console and log messages, exit codes, memory
limit, Ctrl+C handling and cleanup of interrupted runs are the ones above. Only the storage differs:

```powershell
target\release\crime-canopy-generator.exe --input chicago-crime-data.csv --output C:\datasets\chicago-canopy
```

```text
<output folder>/
    CURRENT                               name of the active version folder, as above
    dataset-<YYYYMMDD>-<HHMMSS>-<pid>/    the active version
        manifest.json                     the CanopyDB manifest
        canopydb/                         one CanopyDB 0.2.5 base: LOCK and a crime_<id>_l/ folder
                                          with DATA and OPTIONS; database "crime", six trees
```

- **Same content.** The six trees (`primary`, `date`, `primary_type`, `district`, `community_area`,
  `case_number`) hold exactly the keys and values of the LevelDB tables. The files themselves differ
  between two runs of the same CSV (the database folder name carries a random ID), so the guarantee is
  the same logical content, not identical bytes.
- **Manifest.** `schema_version` is the text `"canopydb-2"`, and the manifest lists the `engine`
  (name, CanopyDB version, fork commit or `null`), the source CSV, the counts, the `compression`, the
  entries of each table and every file of the base with its size. The original query app refuses such a
  dataset with `No se encontró un conjunto de datos válido en <carpeta>.`
- **`--no-compression`** turns off CanopyDB's LZ4 compression of large values (`compression: "none"`
  in the manifest). CanopyDB only compresses values above 12 KiB, and the records of this dataset take
  about 138 bytes, so the base has the same size either way.
- **Ready to read.** The base is written without a write-ahead log, with a 64 MiB page cache, and is
  checkpointed and closed before it is published, so opening it later adds no file to it.
- **Datasets of the other engine.** A folder with a LevelDB dataset counts as an existing dataset: it is
  refused without `--overwrite` and replaced with it. The original generator treats a CanopyDB dataset
  the same way, and the two generators share the lock of the output folder.
- **Engine failures** use the generator's messages for the output folder, with one extra log line
  `Detalle del error de CanopyDB: <error>` right before them.

What CanopyDB does differently from its documentation or from this project's needs is recorded in the
[findings register](docs/canopy-db/hallazgos.md) (in Spanish), with minimal reproductions in
[`docs/canopy-db/repro`](docs/canopy-db/repro).

## Run the query app

Each run answers one query, or a batch of queries read from a file (`--batch`, see **Batches** below),
and ends. Point `--dataset` at the generator's output folder, the one that holds `CURRENT`; the query
app reads the active version and never writes to that folder.

```powershell
# How many thefts in district 008?
target\release\crime-query.exe --dataset C:\datasets\chicago --where "Primary Type=THEFT" --where District=008 --count

# Arrests for theft in August 2019, as CSV in a file
target\release\crime-query.exe --dataset C:\datasets\chicago --where "Primary Type=theft" --range Date=2019-08-01..2019-08-31 --where Arrest=true --format csv --output thefts.csv

# Two records by ID, as JSON
target\release\crime-query.exe --dataset C:\datasets\chicago --where ID=634 --where ID=14348063 --format json

# Thefts and every primary type that starts with CRIM
target\release\crime-query.exe --dataset C:\datasets\chicago --where "Primary Type=THEFT" --startswith "Primary Type=CRIM" --count
```

| Option | Meaning |
|---|---|
| `--dataset <CARPETA>` | The generator's output folder. Required. |
| `--where <CAMPO=VALOR>` | An equality condition. Repeatable. |
| `--startswith <CAMPO=PREFIJO>` | A "starts with" condition on a text field. Repeatable. |
| `--range <Date=DESDE..HASTA>` | An inclusive date range; either end may be left out (`Date=2020-01-01..`, `Date=..2020-01-31`). Repeatable; every range must hold. |
| `--limit <N>` | Return at most N records (N > 0). Any N records of the result, not necessarily the same ones each time. |
| `--count` | Print only the number of results. It cannot be combined with `--format`. |
| `--format table\|csv\|json` | Output format; `table` by default. |
| `--output <ARCHIVO>` | Write the CSV or JSON results to this file instead of the standard output. An existing file is overwritten without asking. Not available with `table` or `--count`. |
| `--batch <ARCHIVO>` | Run the queries of this file, one per line, over the dataset opened once. With it, the query options above go on each line of the file, not on the command line. |
| `-h`, `--help` | Show the help. |

**Conditions.** `CAMPO` is a CSV column name in any case (`"Primary Type"`, `district`). Every
condition must hold (AND); repeating `--where` on the same field gives alternatives (OR):
`--where District=008 --where District=009`. Each query needs at least one condition on an indexed
field: `ID`, `Case Number`, `Date` (with `--range` or `--where`), `Primary Type`, `District` or
`Community Area`. Any other stored field, except the long texts `Block` and `Description` and the
derived `Location`, can be used as a filter on top of an indexed condition.

- `CAMPO=` with nothing after the `=` asks for records without a value (only optional fields).
- Text is compared ignoring case: `theft`, `Theft` and `THEFT` are the same. Results show the text
  exactly as the CSV has it.
- Codes keep their leading zeros and are never padded: `District=8` does not find `008`.
- Arrest and Domestic accept `true`, `false`, `sí`, `si` or `no`.
- Dates are `AAAA-MM-DD [hh:mm]` in Chicago civil time, as published. A range start without a time
  begins at 00:00:00; an end without a time takes the whole day, and with a time the whole minute.
  `--where Date=2019-08-15` is that whole day.

**Prefixes.** `--startswith CAMPO=PREFIJO` keeps the records whose field starts with the prefix
([startswith PRD](docs/prd/chicago-crime/startswith.prd.md)). It follows the same rules as text
equality: case is ignored, the prefix is taken as is, spaces included, and codes are not padded
(`District=8` finds nothing, `District=00` finds `001` to `009`). The prefix cannot be empty, and
a record without a value never starts with any prefix.

- It works on the text fields only: `Case Number`, `Primary Type` and `District`, where it is an
  indexed condition and can be the only one; and `IUCR`, `Location Description`, `Beat` and
  `FBI Code`, where it is a filter that needs an indexed condition too.
- A field's prefixes and `--where` values are alternatives (OR); a record matching several of them
  comes out once.
- Memory does not grow with the number of results or with the number of distinct values a prefix
  covers. A broad prefix such as `Case Number=H`, which covers two thirds of the dataset, uses a memory
  comparable to a narrow one.
- In JSON, the condition lists its prefixes, lowercase, under `startswith`, next to its `values`.

**Output.** CSV and JSON hold every stored field in CSV order, with `Location` rebuilt. The CSV is
written like the source (same header, non-empty fields quoted, empty fields bare, `\n` line ends), so
each record comes out as its source line byte for byte when the source writes its numbers in
canonical form, as every value of the reference CSV does. Numbers are stored typed, so one written
otherwise comes out in canonical form: `41.80` as `41.8`, a Ward `08` as `8`. JSON also holds the query as interpreted (dates resolved to exact
seconds, the limit) and the dataset version it read. The results go to the standard output;
warnings, errors and the closing line with the number of results, the time, the indexes used and the
filters applied go to the standard error, so the results can be redirected or piped as they are. **The
order of the results is not guaranteed** and may change between runs. A rejected, failed or cancelled
query never leaves a half-written output file: with `--output` the results are written to a
temporary file next to the target, named `.<ARCHIVO>.XXXXXX.crime-query.tmp` (for `thefts.csv`,
something like `.thefts.csv.a1B2c3.crime-query.tmp`), which replaces the target only once every
result is written; a rejected, failed or cancelled query deletes it. Only a run killed without a
chance to clean up (console window closed, `taskkill /F`, power loss) can leave that temporary file
behind; it holds no useful result and can be deleted. The query app never deletes such files itself,
since another run writing to the same target may own one. On the standard output, results are written as they are found, so if the dataset
becomes unreadable in the middle of a query, the results already written stay there; the exit code 1
and the error message on the standard error flag the output as incomplete.

Memory does not grow with the result: counting all 1.8 million thefts uses about as much as counting
a rare type. The command line and the messages are in Spanish.

**Batches.** `--batch <ARCHIVO>` runs several queries in one run
([canopy-db-lote PRD](docs/prd/chicago-crime/canopy-db-lote.prd.md)): the query app opens the dataset
once and runs the queries of the file one after the other, in file order, all on the version it opened
even if the generator publishes another one meanwhile.

```text
# Thefts and "criminal" types in January 2020
--where "Primary Type=THEFT" --range Date=2020-01-01..2020-01-31 --count
--startswith "Primary Type=CRIM" --range Date=2020-01-01..2020-01-31 --format csv --output crim.csv

--where ID=13209277 --format json
```

```powershell
target\release\crime-query.exe --dataset C:\datasets\chicago --batch queries.txt
```

- Each line is a query written with the options of a single query (`--where`, `--startswith`,
  `--range`, `--limit`, `--count`, `--format`, `--output`, also as `--option=value`), not
  `--dataset`, `--batch` or `--help`. Next to `--batch`, the command line takes only `--dataset`.
- The file is UTF-8, with or without a BOM, with LF or CRLF line ends. Empty lines and lines starting
  with `#` are skipped; a `#` anywhere else is a plain character. Spaces and tabs separate words;
  double or single quotes group a text with spaces and may open mid-word (`--where="Primary Type=THEFT"`);
  there are no escapes, so `\` in a Windows path is a plain character. Relative `--output` paths
  resolve from the current folder.
- The whole file is checked before the dataset is opened. The first wrong line (`Línea 5: …`, with the
  message the query would get alone), two lines writing the same output file (paths compared after
  resolving `.` and `..`, ignoring case on Windows) or an output equal to the batch file reject the
  batch, and nothing runs.
- The standard output gets exactly what each query would write alone, one after the other, with
  nothing in between; give each query its own `--output` to keep CSV or JSON results apart. The
  standard error gets, for each query, a header `--- Consulta 2 de 3 (línea 4) ---` followed by its
  warnings and its closing line, and at the end `Lote completado: 3 consultas en 0,152 s.`, with the
  time of the whole run.
- A query that fails, or Ctrl+C, stops the batch there: earlier queries keep their results and output
  files, the current one leaves no half-written file, and later ones do not run.

| Exit code | Meaning |
|---|---|
| 0 | The query ran, even with no results. With `--batch`, every query of the batch ran. |
| 1 | The query was rejected because of the dataset or its conditions, or it aborted. With `--batch`, also a batch file that cannot be read, is not UTF-8, has no query or repeats an output file, or a line rejected for its conditions. |
| 2 | The command line, or a line of the batch, is not valid; nothing was queried. |
| 130 | Cancelled with Ctrl+C; no half-written output file is left. |

## CanopyDB query app

`crime-canopy-query` is the query app over a dataset published by the
[CanopyDB generator](#canopydb-generator) ([PRD](docs/prd/chicago-crime/canopy-db-consultor.prd.md)). It
shares the query app's whole code, so its options, queries, batches, outputs, messages and exit codes are
the ones above, and it returns the same records:

```powershell
target\release\crime-canopy-query.exe --dataset C:\datasets\chicago-canopy --where "Primary Type=THEFT" --where District=008 --count
```

It differs in what only a CanopyDB dataset has:

- **One process at a time.** CanopyDB lets a single process open a base. While another one has it open
  (another query or batch, or the CanopyDB verifier), the query prints
  `Aviso: otro proceso tiene abierto el conjunto de datos en <carpeta>; se esperará hasta 30 s.` and
  waits. If the base is released in time it answers as usual; if not, it ends with
  `El conjunto de datos en <carpeta> sigue abierto por otro proceso. Inténtalo de nuevo cuando termine.`
  and exit code 1. Ctrl+C during the wait ends with `Consulta cancelada.` and 130. A batch holds the
  base until its last query ends. The wait does not count in a query's stats line; it does in the batch's
  final line.
- **Datasets it accepts.** Only a `canopydb-2` manifest with the CanopyDB generator's shape. A LevelDB
  dataset, of any schema, gives `No se encontró un conjunto de datos válido en <carpeta>.`; another
  CanopyDB schema, `El conjunto usa el esquema v<N> y este consultor admite v2. Regenera el conjunto.`
  A base written with another CanopyDB version or with a fork (`engine` in the manifest) is refused before
  it is opened, because CanopyDB's files do not record their format version:
  `El conjunto se escribió con CanopyDB <versión> y este consultor usa CanopyDB 0.2.5. Regenera el conjunto.`
  A missing file of the manifest's `files` or a missing table gives the not-found message: the app never
  creates a file, a database or a table.
- **Reading changes nothing you query.** The base is opened with the options stored in it. CanopyDB may
  rewrite its own internal files, but no file is added and the manifest and the records do not change.
  Read-only folders and media are refused, because CanopyDB opens its files for writing.
- **Engine failures** give the query app's `No se pudo leer el conjunto de datos en <carpeta>.`, exit code
  1, with one line before it on standard error: `Detalle del error de CanopyDB: <error>`.
- **JSON output.** The `dataset` member shows `"schema_version": "canopydb-2"`, the manifest's text,
  instead of the number `2`.
- **Memory.** CanopyDB reads its file into a page cache of its own, which is the process's private
  memory, so the app sets an 8 MiB cache (CanopyDB's default is 1 GiB): the peak memory then barely
  grows with what a query reads.

### Fixed query set

[`scripts/acceptance/canopy-fixed-queries.txt`](scripts/acceptance/canopy-fixed-queries.txt) holds the
eleven queries of the CanopyDB module (F1 to F11) that the acceptance check runs with both query apps,
alone and in a batch, and that the comparison benchmark reuses: by ID, by a shared case number, counts
of `THEFT` and of the rare type `RITUALISM`, a month, eleven years of thefts, two indexes, two indexes
and a filter, a prefix, a listing with a limit and the listing of every theft. Replace `<número>` with
the first case number the CSV repeats before running it with `--batch`.

## Verify

[`scripts/acceptance/run-acceptance.ps1`](scripts/acceptance/run-acceptance.ps1) checks AC-G1 to AC-G10
and writes a Markdown report. It takes about 6 minutes and needs about 6 GB of free disk.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-acceptance.ps1 -WorkDir C:\acceptance
```

[`scripts/acceptance/run-query-acceptance.ps1`](scripts/acceptance/run-query-acceptance.ps1) checks
the query app's AC-C1 to AC-C18 and its prefix conditions' AC-S1 to AC-S12
([startswith PRD](docs/prd/chicago-crime/startswith.prd.md); AC-S12 passes when every AC-C row
does): it generates the dataset, runs every criterion
through the binary, compares every count with an independent filtering of the CSV and writes
`query-acceptance-report.md`. It needs about 5 GB of free disk. Its AC-C1 takes the parent AC3's
"el primero, el último" as the first and last CSV rows, and the verifier's AC-G3-SAMPLE as the
smallest and largest IDs, so both readings are checked.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-query-acceptance.ps1 -WorkDir C:\query-acceptance
```

[`scripts/acceptance/run-canopy-acceptance.ps1`](scripts/acceptance/run-canopy-acceptance.ps1) is
the CanopyDB generator's: the same checks with the CanopyDB verifier, where AC-M4 replaces AC-G2 (the
same 512 MiB) and AC-M3 replaces AC-G10 (two generations, with and without compression, and the
original generator's tables hold the same keys and values; the files are not byte-identical). It also
checks AC-KG4: reading the base twice changes no file name nor the manifest. Its work entries are named
`canopy-*`, so it can share a work folder with the original script, and it writes
`canopy-acceptance-report.md`. It needs about 18 GB of free disk; on the reference machine it took
29 minutes (2026-10-21, all 17 checks passed, CanopyDB peak 219.5 MiB).

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-canopy-acceptance.ps1 -WorkDir C:\canopy-acceptance
```

If CanopyDB itself keeps the generator above 512 MiB, `-NotMetByCanopyDb AC-M4=<finding>` names the
finding of the [register](docs/canopy-db/hallazgos.md) that explains it: the AC-M4 row then reads
`NOT MET` instead of `FAIL` and does not fail the run, provided the generation ended normally and the
peak is the only failure.

Both scripts work with the reference extract and with any later download (`-Csv <file>`; by default
`chicago-crime-data.csv` at the repository root). After generating the dataset, they compare the CSV's
SHA-256, which the generator records in the manifest, with the reference extract's (see
[Get the dataset](#get-the-dataset)):

- **Reference extract**: they also require the exact figures of the PRDs, such as 1,838,502 `THEFT`
  records, and run the independent verifier with `--reference`.
- **Any other CSV**: every expected figure is counted on that CSV. The valid records must be its data
  rows with 0 rejected, the `THEFT` and no-community-area counts come from the independent filtering,
  and the verifier runs without `--reference`. A check skipped for lack of data still fails.

The report header says which case applied and shows the CSV's SHA-256
([PRD](docs/prd/chicago-crime/aceptacion-cualquier-csv.prd.en.md)).

Measured on the reference machine (2026-10-13, all 58 checks of the 18 criteria passed; three runs
took 6¼, 7½ and 15 minutes including the generation and the verifier, the last one with a release
rebuild while the machine was busy):

| Measure | Result |
|---|---|
| Lookup of one ID, whole process | 39–51 ms (limit 1 s) |
| Count of `THEFT` (1,838,502) / `District=008` on top | 0.4 s / 0.6 s |
| Count of a month (24,505), with `THEFT` and `Arrest=true` (514) | 0.01 s / 0.2 s |
| Peak private memory: count of `THEFT` / of a rare type (24) / CSV file of every theft | 14.5 / 7.5 / 16.6 MiB |
| Ctrl+C during a CSV file output | exit 130 in 0.05 s, no file left |
| New version published while a query on the old one runs | the query returns exactly the old version's 1,000,000 rows |

The independent verifier can also run on its own. It reads the dataset without any generator code,
compares every CSV row with the primary table, rebuilds every index from the CSV and checks that the
text indexes are in lowercase:

```powershell
target\release\crime-acceptance.exe --csv chicago-crime-data.csv --dataset C:\datasets\chicago
```

Add `--reference` to also require the exact figures of the reference extract.

The CanopyDB verifier, `crime-canopy-acceptance`, does the same on a CanopyDB dataset, with the
same options and report, and checks the CanopyDB layout and manifest instead of the LevelDB files. Its
`--compare` mode compares two datasets of either engine table by table and entry by entry, and gives
the first difference of each table that differs:

```powershell
target\release\crime-canopy-acceptance.exe --csv chicago-crime-data.csv --dataset C:\datasets\chicago-canopy
target\release\crime-canopy-acceptance.exe --compare C:\datasets\chicago-canopy C:\datasets\chicago
```

It opens the base with the options stored in it, so reading changes nothing. A CanopyDB base admits one
process at a time: while a process holds it, another one waits 5 seconds and fails.

The original verifier's `select` mode filters the CSV directly, without any query app code, and is the independent side
of the query app's acceptance: it prints the number of matching rows and can write their IDs or the
rows themselves, in the same CSV form as `crime-query --format csv`:

```powershell
target\release\crime-acceptance.exe select --csv chicago-crime-data.csv --where "Primary Type=THEFT" --where District=008
```

`--startswith FIELD=PREFIX`, repeatable, keeps the rows whose field begins with the prefix, in any
case and taken as is (`8` does not begin `008`), on Case Number, Primary Type, District, IUCR,
Location Description, Beat and FBI Code. A field's prefixes and `--where` values are alternatives,
different fields must all hold:

```powershell
target\release\crime-acceptance.exe select --csv chicago-crime-data.csv --where "Primary Type=THEFT" --startswith "Primary Type=CRIM"
```

## Work on several changes at once

Each change can live in its own copy of the repository (a git worktree) with its own agent session, so two
changes, for example the same PRD implemented with vibe coding and with ODD, can run side by side without
touching each other. A copy named `<n>` gets the branch `feat/<n>`, the folder `C:\wt\chicago-crime-<n>`, a
read-only link to the reference CSV, its own git-ignored `work` folder for acceptance runs and generated
datasets, and its own Engram project. The main copy creates, lists and removes copies; each copy merges its
branch into its base, which advances without a merge commit and is never pushed. A guard hook keeps the agent
of a copy from writing outside it or moving the base by hand.

You ask the agent with fixed Spanish phrases, such as `Crea un worktree para el cambio <n>` in the main copy or
`Fusiona el cambio en la base` in the copy; the project's `worktree` skill runs the matching script in
`scripts/worktree`. The whole flow can be rehearsed on a throwaway base and discarded without changing `main`.
The [worktree guide](docs/worktrees.md) covers every phrase, the rehearsal and what to do for each message;
the Spanish prompts are in [prompts-worktrees.md](docs/methodology/prompts-worktrees.md). Windows only.

## Speaker-note audio

The talk's slides, `docs/methodology/slides.html`, can play a recorded version of each speaker note: open the
presenter view (press P) and use the ▶ Nota button. The MP3s are committed in `docs/methodology/audio`. After
changing the notes in `docs/methodology/slides.md`, generate them again. That needs Python and uv (see
[Requirements](#requirements)); the dependency versions are locked in `scripts/slides/notes_audio.py.lock`.

```powershell
uv run scripts/slides/notes_audio.py
uv run python -m unittest discover -s scripts/slides
```

Only the notes whose text changed are synthesized again. The voice comes from Microsoft's online service
through `edge-tts`, so generating needs an internet connection.

## Repository layout

| Path | Contents |
|---|---|
| `crates/crime-dataset` | Library with the dataset contract: record schema, tuple encoding, indexes, layout, manifest. |
| `crates/crime-generator` | The generator. |
| `crates/crime-query` | The query app. |
| `crates/crime-acceptance` | Independent verifier of a published dataset and independent filtering of the CSV. |
| `crates/crime-canopy-generator` | The CanopyDB generator: the generator's pipeline with a CanopyDB storage. |
| `crates/crime-canopy-acceptance` | Independent verifier of a CanopyDB dataset, with a mode that compares two datasets of either engine. |
| `crates/crime-canopy-query` | The CanopyDB query app: the query app with a CanopyDB reader. |
| `docs/canopy-db` | The CanopyDB findings register and its reproductions. |
| `scripts/acceptance` | Acceptance scripts for the reference extract or any later download (generator and query app). |
| `scripts/worktree` | Scripts that create, list, merge and remove per-change copies (worktrees); see the [worktree guide](docs/worktrees.md). |
| `scripts/slides` | Generates one MP3 per slide from the talk's speaker notes into `docs/methodology/audio`, played with the ▶ Nota button of the presenter view; see [Speaker-note audio](#speaker-note-audio). |
| `docs/prd` | Product requirements: Spanish originals with English translations. |
| `docs/linux` | Guides to build, test and run the project on Linux, in WSL or natively. |
| `odd/tasks` | ODD feature documents with tasks and evidence. |
