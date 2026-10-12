# Chicago crime dataset on SSTables

Rust tools that turn the Chicago crime CSV (8.66 million records, 2.3 GB) into a sorted, immutable
dataset of LevelDB-format tables that can be queried without a database. Product requirements:

- [Parent PRD](docs/prd/chicago-crime/chicago-crime.prd.md): the generator and the query app.
- [Generator PRD](docs/prd/chicago-crime/generador.prd.md): this delivery, with acceptance criteria
  AC-G1 to AC-G10.

The query app comes in a later change.

## Workspace

| Crate | Kind | What it does |
|---|---|---|
| `crates/crime-dataset` | library | The dataset contract: record schema, tuple encoding of keys and values, indexes, layout and manifest. |
| `crates/crime-generator` | binary | Reads the CSV once and publishes the dataset, under a 512 MiB memory peak. |
| `crates/crime-acceptance` | binary | Independent verifier: checks a published dataset against its CSV without any generator or contract code. |
| `scripts/acceptance` | PowerShell | Full acceptance run on the reference CSV, with a Markdown report. |

## Build and test

Built and verified with Rust 1.99 (edition 2024). The reference platform is Windows 11.

```powershell
cargo build --release
cargo test --workspace
```

The binaries end up in `target/release/`.

## Run the generator

```powershell
target\release\crime-generator.exe --input chicago-crime-data.csv --output D:\datasets\chicago
```

| Option | Meaning |
|---|---|
| `--input <ARCHIVO>` | The crime CSV to read. Required. |
| `--output <CARPETA>` | Where the dataset is published. Required. |
| `--temp-dir <CARPETA>` | Folder for the temporary files of the external sort. Default: the system temp folder. |
| `--log <ARCHIVO>` | The run log. Default: next to the output folder, named `<carpeta>-<AAAAMMDD-hhmmss>.log`. It must not exist yet. |
| `--overwrite` | Replace a dataset that already exists in the output folder. Without it, the run refuses. |
| `--no-compression` | Write the tables without Snappy compression. |
| `-h`, `--help` | Show the help, in Spanish. |

Every message, in the console and in the log, is in Spanish. The log is a separate file per run,
outside the dataset, so it survives a failed run. It lists rejected rows (line, column, reason and
the original row), warnings, and a final summary: valid and rejected records, duration, peak
memory, the published files with their sizes, and where the log is.

| Exit code | Meaning |
|---|---|
| 0 | The dataset was published, even with rejected rows or a memory warning. |
| 1 | The run aborted; the previous dataset, if any, is intact. |
| 2 | The command line is not valid; nothing was done. |
| 130 | The operator pressed Ctrl+C; the previous dataset, if any, is intact. |

### Dataset layout

```text
<output folder>/
    CURRENT                               name of the active version folder and a line feed
    dataset-<YYYYMMDD>-<HHMMSS>-<pid>/    the active version
        primary.sst                       key (id); value: the 21 stored fields in CSV order
        index-date.sst                    one table per index, key (value or Nil, id)
        index-primary_type.sst
        index-district.sst
        index-community_area.sst
        index-case_number.sst
        manifest.json                     schema version, source CSV with SHA-256, counts, tables
```

Keys and values are FoundationDB tuples. A run writes a complete new version folder and then
replaces `CURRENT` in one step, so a reader always finds a complete dataset. A failed, cancelled or
killed run never changes the published version. The next run deletes what an interrupted one left
behind, both in the output folder and in the temp folder.

## Run the acceptance

The script needs the reference CSV, `chicago-crime-data.csv`, at the repository root. That file is
not in Git; download it from the [Chicago data portal](https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2).

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-acceptance.ps1 -WorkDir D:\acceptance
```

It builds in release mode and checks, inside `-WorkDir`:

- two full generations: memory peak (AC-G2) and byte-identical tables (AC-G10);
- the independent verifier on the first one (AC-G1, AC-G3, AC-G4 and the manifest);
- a rerun refused without `--overwrite` (AC-G8);
- a CSV with an altered header (AC-G9, and parent AC9 in AC-G5);
- a real Ctrl+C (AC-G7);
- a run killed during the merge, then a run that recovers (AC-G6);
- a CSV with three bad rows (parent AC13 in AC-G5).

It takes about 6 minutes and 6 GB of disk on the reference machine, writes `acceptance-report.md`
to `-WorkDir`, and exits with 1 if any check fails. The CSV is only read.

To run only the verifier on a dataset:

```powershell
target\release\crime-acceptance.exe --csv chicago-crime-data.csv --dataset D:\datasets\chicago --reference
```

It prints one PASS or FAIL line per check and exits with 0 only when all pass. `--reference` also
requires the exact figures measured on the reference CSV; without it, the expected figures come from
the CSV given.
