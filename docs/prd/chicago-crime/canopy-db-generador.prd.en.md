# CanopyDB generator — PRD

**Version**: 1.0
**Date**: 2026-10-21
**Parent document**: [Chicago crime store on CanopyDB — PRD](canopy-db.prd.en.md) (v1.0)

> English translation of [canopy-db-generador.prd.md](canopy-db-generador.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Deliver the first child of the CanopyDB module (order 1 of [Split into child PRDs](canopy-db.prd.en.md#split-into-child-prds)):

- the **CanopyDB generator**, a new application that does the same as the original generator but keeps the
  dataset in a CanopyDB database instead of in SSTables;
- a **CanopyDB verifier**, independent of the generator, that reads the database and compares it with the CSV
  and with another dataset from either of the two engines;
- its **acceptance check** with the reference figures;
- the module's **findings log**, created with H1–H4 and their reproductions.

The CanopyDB generator inherits the contract of the [generator PRD](generador.prd.en.md) (parent decision K1):
the same options, messages, outputs and exit codes, and the rules of the root document, of the [Linux support
PRD](soporte-linux.prd.md) and of the [acceptance with any CSV PRD](aceptacion-cualquier-csv.prd.en.md). This
document does not repeat them. It only sets what the parent assigns to it: the exceptions that depend on the
format (name, shape of the published folder and of the manifest, list of files in the summary and the
no-compression option) and decisions K2, K4, K9 and K10 applied to the generator.

**Why now**: it is the base of the module. The CanopyDB query app, the comparison benchmark and the bug hunt
need a published CanopyDB dataset with the same logical content as the original.

### Starting state

Verified on 2026-10-21 on the repository and on the source code of CanopyDB 0.2.5:

| Aspect | Situation |
|---|---|
| Original generator | Implemented and accepted. Options: input CSV, output folder, temporary folder, log, overwrite and no compression, plus three hidden tuning options: size of the record-sorting and index-sorting segments, and threshold of the memory warning (512 MiB by default). The help says it generates "SSTables y manifiesto". |
| Folder published by the original | In the output folder, a pointer to the active version and one folder per version named `dataset-<AAAAMMDD>-<hhmmss>-<proceso>` (with a suffix `-<n>` if it already exists). The version folder contains the main table, the five indexes and `manifest.json`. It publishes by changing the pointer in a single step and then deletes the previous version; if it cannot, it notes it in the log and a later generation cleans it up. While working, it locks the output folder; a second generation on the same folder aborts with `Otra generación está usando la carpeta de salida <carpeta>.` |
| Classification of the output folder in the original | If the pointer exists, the folder "already contains a dataset", even if the pointer does not lead to a valid version. A version folder that the pointer does not name and the temporary pointer are leftovers. Any other entry is foreign content and is always rejected, with or without overwrite. It only looks at the first level of the output folder, not at the content of the version folders. |
| Manifest of the original | JSON with indentation and a final line break, with `schema_version` (today 2), `generated_at`, `source` (name, size and SHA-256 of the CSV), `records` (valid and rejected), `compression` (`snappy` or `none`), `primary` and `indexes` (file, entries and size of each table). All fields are required when reading it; unknown ones are tolerated. It does not state the engine. |
| Original query app facing a foreign manifest | It locates the active version through the pointer and reads from the manifest **only `schema_version`** first, as an unsigned integer. If it is an integer other than 2, it rejects with `El conjunto usa el esquema v<N> y este consultor admite v2. Regenera el conjunto.` In any other case of an invalid manifest (`schema_version` missing or not an unsigned integer, such as a text; malformed JSON; or a manifest leftover without the original's shape, for example without `primary` or `indexes`), it rejects with `No se encontró un conjunto de datos válido en <carpeta>.` Both with code 1. Only afterwards does it open the tables. |
| Final summary of the original | Lines in this order: valid and rejected records, duration, memory peak against the limit, published folder, one line `Archivo <nombre>: <bytes> bytes (<MiB>).` for each table and for the manifest, and location of the log. The log is also shown on the error output. |
| Write errors in the original | Out of space: `Espacio insuficiente en <carpeta>.`; any other error writing in the output folder: `No se puede escribir en la carpeta de salida <carpeta>.`; in the temporary folder, `No se puede usar la carpeta temporal <carpeta>.`. Then, `Generación cancelada. No se ha modificado el conjunto existente.` and code 1. The technical detail of the error is not shown. |
| Leftovers of interrupted generations in the original | It discards the orphan version folders and the temporary pointer (`Se han descartado restos de una generación interrumpida anterior.`, once even if there are several) and, in the temporary folder, its sort folders of a process that is no longer running (`Se han descartado carpetas temporales de una generación interrumpida anterior.`). |
| Original verifier | Reads the SSTables with an independent LevelDB table reader and compares with the CSV. It writes one row per check (`PASS`, `FAIL` or `SKIP`, its identifier and a summary) and a final `RESULT:` line; it ends with 0, 1 if any fails, or 2 if the command line is not valid. For it, a `SKIP` does not count as a failure; the acceptance checks do count it (decision A3). It requires exactly the seven files of the original and `compression` `snappy` or `none`, so it is of no use for a CanopyDB database. |
| Acceptance check of the original | Windows script that verifies AC-G1 to AC-G10: memory measured from outside and in the summary, two byte-identical generations, interruption, cancellation, overwrite and test CSV. It requires about 6 GB free in the working folder. |
| CanopyDB in the project | It is not a dependency yet. The findings log does not exist, nor does any reproduction of H1–H4. |
| Files of a CanopyDB 0.2.5 database | A database is a folder with a lock file, the files of the write-ahead log and a subfolder for each internal database, with its data file and its options. The name of that subfolder carries a random identifier, so it changes with each generation. When opening the database, CanopyDB treats each subfolder as an internal database; opening it creates the lock file if it is missing and may change the size of the internal files. |
| Compression in CanopyDB 0.2.5 | It only compresses with LZ4 the values larger than a threshold (12 KiB by default); values of up to 4 KiB are never compressed, nor are the tree pages. The records of this dataset take about 138 bytes, so none is compressed, whether the option is enabled or not. |
| CanopyDB memory | The page cache grows on demand up to 1 GiB by default (parent, H4); it is configurable. |
| Parent findings | H1 (CanopyDB README, third example) and H4 (the default cache size only appears in the documentation of that option, not on the front page) are documentation findings. H2 and H3 were checked on the reference machine on 2026-10-20, without a saved reproduction program. |

**Product gaps**: none open. Each point of the parent's "Sets" column remains as a rule in this document or in
[Left to the implementation](#left-to-the-implementation).

## Actors

- **Generator operator**: the one from the parent. Runs the CanopyDB generator just like the original.
- **Developer**: runs the verifier and the acceptance check, and creates the findings.

## Flow

### Generation

It follows flow A of the parent and, by K1, the flow of the generator PRD. This delivery specifies:

1. The operator runs `crime-canopy-generator` (KG1) with the same options as the original, hidden ones
   included, with the same default values and the same meaning (K1); the memory threshold only decides the
   warning, as in the original. The help is the original's with three changes: the name of the application, the
   description, which says `Genera el conjunto de datos de delitos de Chicago (base CanopyDB y manifiesto) a
   partir del CSV.`, and the text of the no-compression option (KG4).
2. Before processing any row it makes the same checks as the original on the CSV, the log, the temporary
   folder and the output folder. It locks the output folder just like the original, so the original generator
   and the CanopyDB one cannot work at the same time on the same folder: the second one aborts with the
   original's message. The output folder is classified as in the original and a dataset from the other engine
   counts as an existing dataset (KG5). It discards the leftovers of interrupted generations (KG6).
3. It validates, converts, rejects, sorts the records and sorts the index entries with the same rules,
   messages, progress lines and memory budget as the original.
4. It writes a CanopyDB database in a new version folder, with the shape of KG2: the main table and the five
   indexes, with the same keys and the same values as the SSTables (K2), including the lowercase text
   indexes. With a CSV that only has the header, it warns like the original and publishes the database with the
   six tables empty.
5. It leaves the database ready to read: opening it afterwards to read does not add files to the version
   folder, nor change the manifest or the logical content (parent K6). What may change is the content or the
   size of the engine's internal files.
6. It writes the CanopyDB manifest (KG3) and publishes atomically, like the original: it changes the pointer
   in a single step and then deletes the previous version; if it cannot, it notes it in the log with the
   original's warning and a later generation cleans it up.
7. It shows the original's final summary, with the list of files of the version folder (see
   [Output](#output)). The memory peak is measured and compared with 512 MiB just like in the original (K2); if
   CanopyDB prevents meeting it, K10 applies (KG10).
8. A cancellation with Ctrl+C or an abrupt interruption behave as in the original, also while CanopyDB writes
   or flushes the database: the cancellation deletes the new version folder and ends with 130; an abrupt
   interruption leaves an orphan version folder that the next generation discards (KG6).
9. On an engine error, it uses the closest existing message and notes the detail in the log (KG7).

### CanopyDB verifier

1. The developer runs the CanopyDB verifier, `crime-canopy-acceptance` (KG12), on the CSV and the output
   folder. The verifier reads the database without any generator code and makes on the content the same checks
   as the original verifier (AC-G1, AC-G3 and AC-G4, with and without the reference figures), besides checking
   the CanopyDB manifest.
2. In its comparison mode it receives two output folders, each from either of the two engines, and compares
   table by table and entry by entry the keys and values of their active versions (KG8).
3. It writes the report in the original verifier's format and ends with the same codes (see
   [Output](#output)). On a CanopyDB database it runs one at a time (K6).

### Findings log

1. The developer creates the module's findings log in the place and with the shape of KG9, with the fields
   set by the parent, and creates H1–H4 with the date on which the parent noted them (2026-10-20), version
   0.2.5 and the status "Detectado". A new finding carries the date of the day it is created. Fields without
   content carry `—`.
2. Each finding has its minimal reproduction:
   - H1: the quote from the CanopyDB 0.2.5 README with the affected example;
   - H4: the quote from the documentation of the cache size option and the observation that the front page
     does not mention it;
   - H2: a program that opens the database in one process and, while it has it open, tries to open it from a
     second process, which fails after 5 seconds;
   - H3: a program that shows that a database in a read-only folder cannot be opened for reading, and that
     opening and closing without writing a database that was closed without flushing its write-ahead log
     changes its files.
3. The reproductions of H2 and H3 are run with a single command, without the rest of the project.
4. A new finding that appears during this execution is created in the same way, with the next identifier.

## Validations and messages

All the validations and messages of the root document, of the generator PRD and of the Linux support PRD
apply, without changing a word. This delivery only sets how they apply to the cases specific to CanopyDB:

| Situation | Rule | Message to the user |
|---|---|---|
| The output folder has an SSTable dataset, without overwrite | It is rejected without touching anything, like an own dataset (KG5) | `La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.` |
| The output folder has an SSTable dataset, with overwrite | It is replaced like an own dataset; the SSTable version is deleted after publishing (KG5) | (usual summary) |
| Orphan version folders, from either engine | They are discarded as leftovers, with a single message (KG6) | `Se han descartado restos de una generación interrumpida anterior.` |
| Sort folders of the CanopyDB generator from a process that is no longer running | They are discarded (KG6) | `Se han descartado carpetas temporales de una generación interrumpida anterior.` |
| The original generator works on the same output folder | It aborts without touching anything | `Otra generación está usando la carpeta de salida <carpeta>.` |
| Engine error due to lack of space | It aborts; the previous dataset remains intact (KG7) | `Espacio insuficiente en <carpeta de salida>.`, or `Espacio insuficiente en <carpeta temporal>.` if the error comes from the engine's temporary files |
| Any other engine error when creating, writing, flushing or closing the database, including an internal failure or an engine panic | It aborts with code 1, never with that of a panic; the previous dataset remains intact (KG7) | `No se puede escribir en la carpeta de salida <carpeta>.` |
| Any other engine error in its temporary files | It aborts as a temporary folder error (KG7) | `No se puede usar la carpeta temporal <carpeta>.` |
| Detail of an engine error | A single line in the log, which also goes to the error output, right before the message of the cause; line breaks in the engine's text are replaced by spaces (KG7) | `Detalle del error de CanopyDB: <texto del error tal como lo da CanopyDB>` |
| The memory peak exceeds the limit (512 MiB by default) | As in the original: warning in the log, the summary indicates it and the generation ends with 0 (KG10) | `Aviso: el pico de memoria privada comprometida (<cifra> MiB) supera el límite de <límite> MiB.` |
| The original query app receives a CanopyDB dataset | It rejects it with code 1, with no change in the original query app (KG3) | `No se encontró un conjunto de datos válido en <carpeta>.` |

As in the original, any abort ends with `Generación cancelada. No se ha modificado el conjunto
existente.`, except cancellation with Ctrl+C, which only shows `Generación cancelada por el operador. No se ha
modificado el conjunto existente.` Exit codes: those of the original (0, 1, 2 and 130).

## Output

### Published folder

Same shape as the original (KG2): in the output folder, the pointer to the active version and the version
folder `dataset-<AAAAMMDD>-<hhmmss>-<proceso>[-<n>]`. The version folder contains only:

- `manifest.json`, the CanopyDB manifest;
- `canopydb/`, the CanopyDB database with its internal files.

### Manifest

It is written like the original's, with indentation and a final line break, with these fields and in this order
(KG3). Illustrative and abbreviated example (`tables` always has six entries); the names in `files` depend on
CanopyDB and its configuration:

```json
{
  "schema_version": "canopydb-2",
  "generated_at": "2026-10-21T10:15:00+02:00",
  "engine": {
    "name": "canopydb",
    "version": "0.2.5",
    "fork_commit": null
  },
  "source": {
    "file_name": "chicago-crime-data.csv",
    "size_bytes": 2406401982,
    "sha256": "<huella en hexadecimal, minúsculas>"
  },
  "records": {
    "valid": 8656904,
    "rejected": 0
  },
  "compression": "lz4",
  "tables": [
    {
      "name": "primary",
      "entries": 8656904
    },
    {
      "name": "date",
      "entries": 8656904
    }
  ],
  "files": [
    {
      "path": "canopydb/LOCK",
      "size_bytes": 0
    },
    {
      "path": "canopydb/WAL-0000000001",
      "size_bytes": 4096
    },
    {
      "path": "canopydb/crime_1F3A9C0B2D4E6F70_l/DATA",
      "size_bytes": 1534066688
    }
  ]
}
```

- `schema_version` is the text `canopydb-2`: the engine and the version of the logical schema, which is the
  same as that of the SSTables (K2). Since it is not a number, the original query app rejects the dataset on
  its first reading of the manifest, with the message in the table (KG3). It does not have `primary` or
  `indexes` either.
- `engine.version` is the CanopyDB version the generator was compiled with, as declared by the library used;
  `engine.fork_commit` is the full commit of the fork if one is used (flow E of the parent) and `null` if not.
  In this delivery it is always `null`.
- `compression` is `lz4` by default and `none` with the no-compression option (KG4).
- `tables` always has the six tables, in this order: `primary`, `date`, `primary_type`, `district`,
  `community_area` and `case_number`.
- `files` lists each file of `canopydb/`, with its path relative to the version folder, separated with `/` on
  all systems, in byte order of the path, and its size in bytes at the time of publishing. After opening the
  database the names do not change, but the sizes may change (step 5 of the flow).
- With a CSV that only has the header, `records.valid` and each `entries` are 0, and `files` still lists the
  files of the database.

### Final summary

The same lines and in the same order as the original. The `Archivo` lines are one for each entry of `files`,
in the same order and with the same path, followed by that of `manifest.json`:

```text
Archivo canopydb/LOCK: 0 bytes (0,0 MiB).
Archivo canopydb/WAL-0000000001: 4.096 bytes (0,1 MiB).
Archivo canopydb/crime_1F3A9C0B2D4E6F70_l/DATA: 1.534.066.688 bytes (1.463,0 MiB).
Archivo manifest.json: 1.234 bytes (0,1 MiB).
```

### Verifier report

`crime-canopy-acceptance` (KG12) accepts the options of the original verifier (`--csv <archivo>`,
`--dataset <carpeta de salida>` and `--reference`) and the comparison mode `--compare <carpeta A> <carpeta B>`,
which cannot be combined with the former. An invalid combination ends with 2, as in the original.

Same format and same language (English) as the original verifier: one row per check with `PASS`, `FAIL` or
`SKIP`, its identifier and a summary, and the final `RESULT:` line. The exit codes and the treatment of `SKIP`
are those of the original.

- **With the CSV** (`--csv` and `--dataset`): it only accepts a CanopyDB dataset. The original's content checks
  keep their identifiers (`CSV`, `AC-G1`, `AC-G3`, `AC-G3-SAMPLE`, `AC-G3-DST`, `AC-G4-<índice>`,
  `AC-G4-THEFT`, `AC-G4-COMMUNITY-NIL`, `AC-G4-SHARED-CASES` and `AC-C18-LOWERCASE`). The shape and manifest
  ones also keep theirs, but check the CanopyDB shape:
  - `LAYOUT`: the active version folder has only `manifest.json` and `canopydb/`;
  - `MANIFEST-SCHEMA`: `schema_version` `canopydb-2`, the fields and their order, `engine.name` `canopydb`,
    `compression` `lz4` or `none`, and the six tables of `tables` with their names and their order;
  - `MANIFEST-SOURCE`: name, size and SHA-256 of the CSV, as in the original;
  - `MANIFEST-TABLES`: each `entries` against the entries read from its table;
  - `MANIFEST-FILES`: the paths in `files` against the files of `canopydb/`, without comparing sizes, which
    may change when the database is opened (step 5 of the flow).

  If the folder does not have a valid CanopyDB dataset, `LAYOUT` is the only row, gives `FAIL` and no other
  check is run: with an SSTable dataset, its summary is `not a CanopyDB dataset`.
- **In comparison mode** (`--compare`): each folder is a CanopyDB dataset if the manifest of its active version
  has `schema_version` `canopydb-2`, and an SSTable dataset if it has `schema_version` 2 and the original's
  shape. If both open, it gives one `COMPARE-<tabla>` row for each of the six tables. If they match, the
  summary gives the number of entries. If not, it gives the number of entries on each side and the first
  difference: its position, counting from 1, and the key on each side as a decoded tuple, in the format with
  which the original verifier shows values (`none` if one side ended earlier); or the key and `values differ`
  if the keys match and the values do not.
- If one of the folders does not have a valid dataset from either engine, or its database cannot be opened
  (for example, because another process has it open), the report has a single `FAIL` row with the identifier
  `COMPARE` and the reason, and ends with 1.
- The CanopyDB verifier does not have the original's `select` subcommand: direct filtering of the CSV remains
  that of the original verifier.

### Acceptance check report

`run-canopy-acceptance.ps1` (KG12) accepts the parameters of the original's check (`-WorkDir`, `-Csv`,
`-KillTimeoutSeconds`, `-CtrlCAfterSeconds` and `-SkipBuild`) and, in addition, `-NotMetByCanopyDb`, a
PowerShell list of `<criterio>=<hallazgo>` associations (KG10), with the criterion written exactly as the label
of its row. Its report is a Markdown file like the original's, in English, titled
`Acceptance of the CanopyDB crime generator`, with these changes:

- The rows are the original's, with their labels, except `AC-G2`, which becomes `AC-M4`, and `AC-G10`, which
  becomes `AC-M3`. `AC-M3` compares two CanopyDB generations of the same CSV, the first with compression and
  the second without it; `AC-M3 (SSTables)`, the comparison of the first with the dataset of the original
  generator, and `AC-KG4` are added.
- It ends with 0 if the overall result is `PASS` and with 1 if it is `FAIL`.
- Only `AC-M4` can be associated. Its row gives `NOT MET`, with the usual evidence followed by
  `(not met by CanopyDB, <hallazgo>)`, only when the generation ended with 0, the summary has the peak line and
  the only thing failing is that the peak, measured from outside or in the summary, exceeds 512 MiB. Any other
  failure of that row gives `FAIL`, with the association or without it; if it is met, it gives `PASS`, as
  always.
- The overall result does not count `NOT MET` rows as failures. With no `FAIL`, it says
  `PASS: all <n> checks passed` if there is no `NOT MET`, and `PASS: <p> of <n> checks passed, 1 not met by
  CanopyDB (<hallazgo>)` if `AC-M4` gives `NOT MET`; with any `FAIL`, `FAIL: <f> of <n> checks failed`, like the original.
- Before starting, the check validates the associations. One without the shape `<criterio>=<hallazgo>`, with a
  criterion other than `AC-M4`, with a finding without its section in the register, or a second association of
  `AC-M4`, makes it end with 1 without running anything, with the error
  `Invalid -NotMetByCanopyDb association '<asociación>': <motivo>.`, where the reason is
  `expected <criterion>=<finding>`, `only AC-M4 can be associated`, `<hallazgo> is not in the findings
  register` or `AC-M4 is already associated`.

## Documentation

- **Help** of the CanopyDB generator and of the CanopyDB verifier.
- **README**: a section on the CanopyDB generator (what it is for, how it is run and how it differs from the
  original: published folder, manifest and no-compression option), on the verifier and on its acceptance
  check, with a link to the findings log.
- **Linux guides**: how to build and run the CanopyDB generator and check AC-M4 by hand.

## Reuse

- **Shared dataset contract**: record schema, key and value encoding, the five indexes, the pointer to the
  active version and the shape of the output folder.
- **From the original generator**: CSV reading and validation, external sorting, atomic publication, locking
  and classification of the output folder, discarding of leftovers, log, cancellation, memory measurement,
  messages and summary format. Only where the tables and the manifest are written changes. Its code can be
  reorganized to share it, without changing its behavior (KG11).
- **Original verifier**: its content checks against the CSV, its report format and its independent LevelDB
  table reader, which the comparison mode uses to read an SSTable dataset.
- **Acceptance check of the original**: its cases, with the changes of the criteria in this document.

## Out of scope

- The CanopyDB query app, the query batch, the comparison benchmark and the bug hunt: they are children 2 to 5
  of the parent.
- Changing the behavior of the original generator, query app or verifier.
- From flow E, everything except the entry in the log: preparing or publishing issues and PRs, commenting,
  creating the fork or depending on it.
- Reading or converting datasets between engines, except the content comparison of the verifier.
- Guaranteeing that a database is readable with another CanopyDB version, and the rejection of such datasets:
  set by `canopy-db-consultor`.
- Checking what happens when deleting a version that a CanopyDB query app has open: there is no CanopyDB query
  app until `canopy-db-consultor`, and the concurrency scenarios belong to `canopy-db-robustez`.
- Having the original generator discard the sort folders of the CanopyDB generator, or the other way around:
  each one discards only its own.
- A generation time target and tuning CanopyDB to generate faster: the performance comparison belongs to
  `canopy-db-comparacion`.
- Running the acceptance check on Linux: AC-M4 is checked there by hand.
- Durability against power cuts, as in the parent.
- Compressing the values in the application before storing them (KG4).

## Left to the implementation

- The internal organization of the database (one internal database with six trees or several) and the
  configuration of CanopyDB, including how its memory is limited, as long as the contract and the limits are
  met. It determines the names in `files`; the README and the closing report show the ones that result.
- The CanopyDB version: the latest published one at the start, today 0.2.5.
- The free space required by the acceptance check, calculated from a measured generation.
- The language of the reproduction programs of H2 and H3 and where they live in the repository, as long as they
  are run with a single command and without the rest of the project.
- How the tests trigger the engine errors of AC-KG7, as long as it adds no visible option or changes the
  behavior of a normal run.

## Acceptance criteria

The figures correspond to the reference CSV, with the same caveat as in the parent. An inherited criterion that
CanopyDB cannot meet is handled according to K10 (KG10).

1. **AC-KG1** (AC-M1, generator part) — Given the reference CSV, when the CanopyDB generator and its
   acceptance check are run, then AC-G1 and AC-G3 to AC-G9 are met with the same figures, checked with the
   CanopyDB verifier; AC-M3 replaces AC-G10, and AC-M4, AC-G2 and AC-L8. With the reference extract AC-A1 is
   met; with a later download from the portal, made during the verification as decision A5 sets, AC-A2 and
   AC-A3.
2. **AC-M3** (decision K2, KG8) — Given the same CSV, when it is generated twice with the CanopyDB generator
   and once with the original, then the verifier's comparison mode confirms that the two CanopyDB datasets
   have exactly the same keys and values in each table, and that they are the same as those of the SSTables;
   the same between a CanopyDB dataset generated with compression and another without it.
3. **AC-M4** (decision K2, KG10) — Given the complete reference CSV, when the CanopyDB generator is run, then
   the measured memory peak does not exceed 512 MiB and the final summary shows it. On Windows the acceptance
   check verifies it, from outside and in the summary; on Linux it is checked by hand and noted in the closing
   report. If CanopyDB prevents it, it is handled according to KG10.
4. **AC-KG2** (KG2, KG3) — Given a freshly published and unopened CanopyDB dataset, when the output folder is
   inspected, then it has the pointer and a version folder with only `manifest.json` and `canopydb/`; the
   manifest has the fields and the order of [Manifest](#manifest); `files` matches the files of `canopydb/`
   and their sizes; and the final summary lists those same files in the same order, followed by
   `manifest.json`. Afterwards, the verifier confirms that `tables` matches the entries it reads. With a CSV
   that only has the header, the original's warning is shown, each `entries` is 0 and `files` lists the files
   of the database.
5. **AC-KG3** (KG3) — Given a published CanopyDB dataset, when it is queried with the original query app, then
   it is rejected with `No se encontró un conjunto de datos válido en <carpeta>.` and code 1.
6. **AC-KG4** (K6, step 5 of the flow) — Given a freshly published CanopyDB dataset and a copy of its whole
   output folder made before opening it, when the verifier reads the whole database twice, then the version
   folder has the same file names as the copy's, with no new one, the manifest is identical and the comparison
   mode between the output folder and the copy gives the six tables equal.
7. **AC-KG5** (KG5) — Given an output folder with an SSTable dataset, when the CanopyDB generator is run
   without overwrite, then it is rejected with the existing dataset message and the folder does not change;
   with overwrite, it publishes the CanopyDB dataset and the SSTable version folder disappears. Given a folder
   with a CanopyDB dataset, the original generator behaves the same in both cases.
8. **AC-KG6** (KG6) — Given an output folder with a published dataset and an orphan version folder from each
   engine, when the CanopyDB generator is run, then it discards both with a single leftovers message and the
   result is that of a normal generation. Given, in the temporary folder, some sort folders of a CanopyDB
   generator that is no longer running, others of one that is still running and others of the original
   generator that is no longer running, when the CanopyDB generator starts, then it discards only the first
   ones, with its message, on Windows and on Linux.
9. **AC-KG7** (KG7) — Given an engine error triggered in a test (lack of space, a write failure, an internal
   failure, an engine panic and an error in its temporary files), when the generation aborts, then the message
   is the one in the table, the log contains a single line `Detalle del error de CanopyDB:` followed by the
   error text, the exit code is 1 and the previous dataset remains intact and the verifier reads it without
   failures.
10. **AC-KG8** (KG4) — Given a CSV, when it is generated with and without the no-compression option, then the
    manifest says `lz4` and `none` respectively, and the help describes the option with the text of KG4.
11. **AC-KG9** (KG8) — Given two datasets with a known difference in one table (one extra entry, a different
    key and a different value), when they are compared with the verifier, then that table gives `FAIL` with the
    difference described in [Verifier report](#verifier-report), the others give `PASS` and the exit code is
    1; with two equal datasets, all give `PASS` and the code is 0; with a database that another process has
    open or a folder without a dataset, it gives a single `FAIL` row with the reason and code 1; with an
    SSTable dataset in `--dataset`, `LAYOUT` gives `FAIL` with `not a CanopyDB dataset`; and with an invalid
    command line, the code is 2.
12. **AC-KG10** (KG9) — Given the findings log at closing, when it is reviewed, then it is in the place and with
    the shape of KG9, with all the parent's fields for H1–H4 and for any new finding; the quotes of H1 and H4
    correspond to the CanopyDB 0.2.5 documentation; and when the reproductions of H2 and H3 are run with the
    noted version, they show what step 2 of [Findings log](#findings-log) describes.
13. **AC-KG11** (KG11) — Given the repository on Windows and on Linux (Ubuntu 24.04 on WSL2), when the tests
    are run, then all of them pass, including those of the CanopyDB generator and verifier. The tests of the
    original applications that existed before the change still exist and pass, none disappears and their
    checks do not change; only the paths or names with which they reach the code they test may change. On
    Linux, the CanopyDB generator tests cover AC-L3 to AC-L6 and AC-L9. And on Windows, the acceptance check of
    the original generator still passes.
14. **AC-KG12** — Given the help and the README, when they are read, then the CanopyDB generator's help is the
    original's with the three changes of step 1 of the flow, and the README has the section of
    [Documentation](#documentation) with the link to the findings log.
15. **AC-KG13** (KG10) — Given the acceptance check with `-NotMetByCanopyDb AC-M4=<hallazgo>` and a generation
    that ends with 0 with a peak of more than 512 MiB, when it ends, then the `AC-M4` row says `NOT MET` with
    its finding and the overall result is `PASS` if there is no other `FAIL` row; with the same association
    and a generation that fails in another way (without the peak line or with a code other than 0), the row
    says `FAIL`; and given an association with a criterion other than `AC-M4` or a finding without its section
    in the register, when it starts, then it ends with 1 without running anything, with the error in
    [Acceptance check report](#acceptance-check-report).
16. **AC-KG14** (step 2 of the flow) — Given a generation of the original generator in progress on an output
    folder, when the CanopyDB generator starts on the same folder, then it aborts with `Otra generación
    está usando la carpeta de salida <carpeta>.` without touching anything, and the first one ends normally;
    and the same the other way around, on Windows and on Linux.

## Decisions

All were made with the user on 2026-10-21.

- **KG1 — Name**: `crime-canopy-generator`; its sort folders are named like the original's with that name. The
  CanopyDB query app sets its own name in its PRD.
- **KG2 — Folder shape**: the same as the original (pointer and version folders with the same name pattern);
  inside the version folder, the manifest and the CanopyDB database in its own `canopydb/` subfolder. Reason:
  it reuses as is the atomic publication, the locking and the classification of the output folder, and the
  original query app reaches the manifest, where the rejection is set (KG3). The subfolder is necessary
  because CanopyDB treats each subfolder of the database as an internal database, and it separates the
  engine's files from the manifest.
- **KG3 — Manifest**: `schema_version` is the text `canopydb-2`, with `engine`, `tables` and `files` instead of
  `primary` and `indexes`. The original query app rejects it on its first reading of the manifest with `No se
  encontró un conjunto de datos válido en <carpeta>.`, the same message with which the CanopyDB query app will
  reject an SSTable dataset (parent, Validations and messages). Reason: no tool that reads only
  `schema_version` can confuse a CanopyDB dataset with an SSTable one, and the text keeps the number of the
  logical schema. Discarded: `schema_version` 2 without `primary` or `indexes` (a tool that read only the
  version would take it for an SSTable dataset) and a different number, which would give `El conjunto usa el
  esquema v<N> y este consultor admite v2. Regenera el conjunto.`, a misleading message: the analyst does not
  need to regenerate, but to use the CanopyDB query app.
- **KG4 — No-compression option**: it is accepted with the same name; it disables the LZ4 compression that
  CanopyDB applies to large values, enabled by default. Help: `Escribe la base sin compresión. Por defecto,
  CanopyDB comprime con LZ4 los valores grandes.` `compression` describes the configuration, not that any value
  was compressed. Accepted consequence: with this dataset no value is large enough, so the database takes the
  same space with and without the option; the README explains it. Discarded: compressing the values with
  Snappy in the application, because it would change the stored values and break K2.
- **KG5 — Dataset from the other engine**: the CanopyDB generator treats an SSTable dataset as an existing
  dataset, just as the original already treats a CanopyDB dataset with the shape of KG2. Reason: one contract
  and no change in the original.
- **KG6 — Leftovers**: as in the original. Orphan version folders are discarded whatever their engine; in the
  temporary folder, each generator discards only its own sort folders. The temporary files that CanopyDB uses
  go inside the run's sort folder and are discarded with it.
- **KG7 — Engine errors** (K9): a CanopyDB error, including an internal failure or a panic, uses the original's
  write messages: those of the temporary folder if it comes from the engine's temporary files, which are
  configured inside the run's sort folder, and those of the output folder in any other case. The detail goes
  on a line of its own in the log. Reason: the original does not show the technical detail and K9 asks to keep
  it to investigate findings.
- **KG8 — Same logical content** (K2): the CanopyDB verifier compares entry by entry two datasets from either
  engine. Reason: a fingerprint of the files is of no use between engines, and the direct comparison says where
  the first difference is.
- **KG9 — Findings log**: `docs/canopy-db/hallazgos.md`, in Spanish, common to the whole module. It starts with
  a summary table (identifier, type, status and title) and continues with one section per finding, with all the
  parent's fields in list form. Reason: reproductions, quotes and approvals do not fit well in a cell; the
  module's folder can also host the reproductions.
- **KG10 — Criteria that CanopyDB cannot meet** (K2 and K10): the generator configures CanopyDB to meet 512
  MiB. If CanopyDB prevents it, the generation ends anyway, with the original's warning. The acceptance check
  has a status of its own for K10, only for AC-M4: when the developer runs it associating AC-M4 with its
  finding in the log (`-NotMetByCanopyDb AC-M4=H5`, for example) and the only thing failing is that the peak
  exceeds 512 MiB, the row says `NOT MET` instead of `FAIL` and does not count as a failure in the overall
  result (see [Output](#output)). The execution log and the closing report note it as "no cumplido por
  CanopyDB" (not met by CanopyDB). Reason: the acceptance report separates what fails in the project from what
  CanopyDB prevents, the association with an existing finding forces the exception to be justified with its
  reproduction, and limiting it to the peak keeps it from hiding a project failure, which K10 requires to be
  fixed. Discarded: marking it `FAIL` and classifying it only in the closing report, and admitting any
  criterion or any failure.
- **KG11 — Original applications**: their code can be extracted or reorganized to share it with the CanopyDB
  applications, without changing their behavior, their messages or their outputs; their tests still pass
  without changing what they check, and their acceptance check still passes. Reason: almost the whole original
  generator is usable as is, and duplicating it would cost more than the risk, which their tests and their
  acceptance check cover. Discarded: not touching a single line of the originals.
- **KG12 — Verification tools**: the CanopyDB verifier is called `crime-canopy-acceptance` and the acceptance
  check `run-canopy-acceptance.ps1`, with the naming pattern of KG1 (see [Output](#output)).

## Annotations on closing

On closing this child, the [generator PRD](generador.prd.en.md) and its translation are annotated, without
rewriting their history, as asked by [Criteria and annotations per child](canopy-db.prd.en.md#criteria-and-annotations-per-child):

- In G5: *Annotation of <date>: the CanopyDB generator does not produce byte-identical files; two generations
  of the same CSV store the same keys and values (decision K2 of [canopy-db.prd.en.md](canopy-db.prd.en.md),
  criterion AC-M3 of [canopy-db-generador.prd.en.md](canopy-db-generador.prd.en.md)). The original generator
  keeps G5.*
- In AC-G2: *Annotation of <date>: the CanopyDB generator must meet the same 512 MiB (K2); if CanopyDB prevents
  it, K10 applies (criterion AC-M4 of [canopy-db-generador.prd.en.md](canopy-db-generador.prd.en.md)).*

## References

- Parent document: [canopy-db.prd.en.md](canopy-db.prd.en.md)
- General parent document: [generador.prd.en.md](generador.prd.en.md), whose contract the new application
  inherits (K1)
- Root document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
- [Linux support PRD](soporte-linux.prd.md) and [acceptance with any CSV
  PRD](aceptacion-cualquier-csv.prd.en.md)
- CanopyDB: https://github.com/arthurprs/canopydb and https://docs.rs/canopydb

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 982e14bf | 2026-10-21 07:29 UTC+01:00 | 43 min | 0 min | 43 min | 31 min | 12 min | 9 |

<!-- prd-time: [{"session": "982e14bf-027d-4852-9540-64cd2aeea746", "start": "2026-10-21T06:29:14.532000+00:00", "wall": 2607, "discarded": 0, "agent": 1869, "human": 738, "interventions": 9, "idle_minutes": 10}] -->
