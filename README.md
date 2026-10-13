# Chicago crime dataset on SSTables

Rust tools that turn the City of Chicago crime history (≈8.7 million records, ≈2.4 GB of CSV) into a
sorted, immutable dataset of LevelDB-format tables that can be queried without a database. Keys and
values are encoded as FoundationDB tuples. The project is also a hands-on exercise in Rust, external
sorting under a fixed memory budget, and product-driven development.

| Application | Status | What it does |
|---|---|---|
| Generator (`crime-generator`) | Done and verified | Reads the CSV once and publishes a primary table keyed by ID, five secondary indexes and a manifest, with a peak memory of about 93 MiB (limit 512 MiB). |
| Query app (`crime-query`) | Planned | Answers lookups by ID, case number, indexed values and date ranges. |

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
Parent PRD (app or large module) → child PRD (one per change) → ODD feature document
  → tasks: test first, one commit each, automated review → acceptance on real data → merge into main
```

- **PRD first.** The parent PRD describes the whole product. Each change gets a child PRD that names its
  parent and adds only what that change needs. A PRD is written in business language: objective,
  starting state verified against the code and data, flow, validations and messages, decisions with
  their reasons, explicit out-of-scope items and acceptance criteria. The project skill in
  [`.claude/skills/prd`](.claude/skills/prd/SKILL.md) holds the template.
  - [Parent PRD](docs/prd/chicago-crime/chicago-crime.prd.en.md) ([Spanish original](docs/prd/chicago-crime/chicago-crime.prd.md)): the generator and the query app.
  - [Generator PRD](docs/prd/chicago-crime/generador.prd.en.md) ([Spanish original](docs/prd/chicago-crime/generador.prd.md)): this delivery, acceptance criteria AC-G1 to AC-G10.
- **ODD (Organic Driven Development).** The PRD becomes a feature document with an objective,
  implementation decisions and a task checklist. That document is the single place for progress and
  evidence: [`odd/tasks/crime-generator.md`](odd/tasks/crime-generator.md). Each task:
  - explores the code first;
  - writes a failing test before the implementation (RED → GREEN);
  - closes with one Conventional Commit on the feature branch;
  - records its evidence in the feature document.

  An automated review (Gentle AI's receipt-driven review) checks the commits. Its findings become new
  tasks, for example T2a and T5a in the generator. The feature is merged into `main` only after the full
  acceptance passes.

## Build

You need a Rust toolchain. The project uses edition 2024, which requires at least Rust 1.85, and it was
built and verified with Rust 1.99 on Windows 11, the reference platform. Other systems are not verified, because the run lock and the
memory measurement use Windows APIs.

```powershell
cargo build --release
cargo test --workspace
```

The binaries end up in `target\release\`.

## Run the generator

```powershell
target\release\crime-generator.exe --input chicago-crime-data.csv --output D:\datasets\chicago
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

## Verify

[`scripts/acceptance/run-acceptance.ps1`](scripts/acceptance/run-acceptance.ps1) checks AC-G1 to AC-G10
on the reference CSV and writes a Markdown report. It takes about 6 minutes and needs about 6 GB of
free disk.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-acceptance.ps1 -WorkDir D:\acceptance
```

The independent verifier can also run on its own. It reads the dataset without any generator code,
compares every CSV row with the primary table, rebuilds every index from the CSV and checks that the
text indexes are in lowercase:

```powershell
target\release\crime-acceptance.exe --csv chicago-crime-data.csv --dataset D:\datasets\chicago
```

Add `--reference` to also require the exact figures of the reference extract.

## Repository layout

| Path | Contents |
|---|---|
| `crates/crime-dataset` | Library with the dataset contract: record schema, tuple encoding, indexes, layout, manifest. |
| `crates/crime-generator` | The generator. |
| `crates/crime-acceptance` | Independent verifier of a published dataset. |
| `scripts/acceptance` | Acceptance script for the reference CSV. |
| `docs/prd` | Product requirements: Spanish originals with English translations. |
| `odd/tasks` | ODD feature documents with tasks and evidence. |
