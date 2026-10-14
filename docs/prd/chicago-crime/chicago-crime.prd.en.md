# Queryable Chicago crime store on SSTables — PRD

**Version**: 1.2
**Date**: 2026-10-12
**Parent document**: none

> English translation of [chicago-crime.prd.md](chicago-crime.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Build two command-line applications in Rust that turn the Chicago crime history (8.66 million records,
2.3 GB of CSV) into a sorted dataset that can be queried without a database:

1. **Generator**: reads the CSV only once and produces a primary SSTable sorted by ID, plus one SSTable
   per secondary index.
2. **Query app**: answers questions about that dataset (by ID, by case number, by values of indexed fields
   and by date ranges) without loading it entirely into memory.

**Why**: the CSV does not allow efficient lookups; every question forces a scan of 2.3 GB. A sorted,
immutable store allows point and range queries with a fraction of that reading, and serves as a hands-on
case for learning Rust, external sorting and FoundationDB tuple encoding.

**Main constraint**: the generator must work with a peak memory of **512 MiB**, even though the dataset
does not fit in that space. It is a deliberate design constraint, not a limitation of the current machine.
The limit refers to the peak private committed memory of the generator process, including the runtime and
the libraries and excluding the operating system's file cache. On Windows it is measured objectively as
the maximum private committed bytes of the process. Exceeding the limit does not stop the generation: a
warning is written to the log and the final summary reports it. *(Note added on 2026-10-14: on Linux it
is measured as the peak resident memory of the process; decision L3 of the [Linux support
PRD](soporte-linux.prd.md), in Spanish.)*

### Starting state

Verified on 2026-10-12 against the repository and the data:

| Aspect | Situation |
|---|---|
| Code | No Rust code or project exists. There is only the CSV and the previous design notes. |
| Source | Extract of *Crimes — 2001 to Present* from the Chicago data portal, updated on 2026-10-10. |
| Volume | 8,656,904 records, 22 columns in every row; the longest row is 305 characters long. |
| Identifier | All IDs are integers (from 634 to 14,353,993) and **there are no duplicates**. |
| Order | The CSV is **not** sorted by ID (2.5 million backward jumps). Sorting is mandatory. |
| Case number | **Not unique**: 632 records repeat a case number already seen. |
| Dates | All in the same format `MM/DD/YYYY hh:mm:ss AM/PM`, Chicago local time. Years 2001–2026. |
| Empty values | Coordinates and location: 100,851 · Electoral district (Ward): 614,809 · Community area: 613,720 · Location description: 16,758 · Police district: 47. |
| Cardinalities | Primary type: 34 · District: 26 · Community area: 78 · Ward: 51 · FBI: 26 · Beat: 306 · IUCR: 418 · Location description: 219. |
| Reference machine | Windows 11, 32 threads, 64 GB of RAM. |

**Product gaps**: none; the decisions are in [Decisions](#decisions).

## Actors

- **Generator operator**: runs the generator on a downloaded CSV and obtains a dataset ready to be
  queried.
- **Analyst**: uses the query app to search for crimes and count results.
- **Chicago data portal** (external): publishes the CSV. There is no direct integration; the operator
  downloads the file by hand.

## Flow

### A. Generation

1. The operator specifies the input CSV, the output folder and, optionally, a temporary folder for
   sorting.
2. The generator validates the header before processing any row.
3. It streams through the CSV, converts each field to its type (see [Field rules](#field-rules)) and
   shows the progress (rows processed, current phase). Each invalid record is written to the log with its
   row, column, reason and original content, and is skipped without stopping the process (decision D5).
4. It sorts the records by ascending numeric ID, using temporary disk space when they do not fit in memory.
5. It writes the **primary SSTable**: key = FoundationDB tuple with the ID; value = flat FoundationDB tuple
   with all the typed fields in CSV order, including the ID and without Location (decision D4).
6. It writes **one SSTable per secondary index**: date, primary type, district, community area and case
   number (decision D2). Each entry relates a value of the indexed field to an ID. Within the same value,
   the IDs appear in ascending order. Empty values are indexed as "no value" (*sin valor*) so that they
   can also be queried. *Note of 2026-10-13: under decision C1 of the [query app PRD](consultor.prd.md),
   the values of the text indexes (primary type, district and case number) are stored in lowercase, with
   schema 2; the primary table does not change.*
7. It writes a **manifest** that describes the dataset: schema version, source, generation date, number of
   valid and rejected records, and the included indexes with their number of entries.
8. Only if everything finished successfully, it publishes the dataset to the output folder atomically: a
   half-written dataset is never visible. A failed generation, including an unexpected interruption of the
   process, neither leaves an incomplete dataset visible nor invalidates the previous dataset, which
   remains queryable. Durability against power failures is not required. The technical mechanism is
   chosen during implementation.
9. It shows a final summary: valid and rejected records, duration, peak memory and size of each file.

### B. Query

1. The analyst runs a command with the dataset folder and the query; the query app answers and exits
   (decision D6).
2. The query app checks the manifest and that the schema version is compatible.
3. Query types in the first version:
   - **By ID**: one or several.
   - **By case number**: returns all the records that share it.
   - **By equality** on an indexed field, with one or several alternative values (logical OR).
   - **By date range** (from/to, both inclusive).
   - **Combination** of several of the previous conditions with logical AND.
4. Output options: result limit, count only, and readable table, CSV or JSON format.
5. Results are returned in ascending ID order. *Note of 2026-10-13: replaced by decision C11 of the
   [query app PRD](consultor.prd.md); in that change the order of results is not guaranteed, and ordering
   is planned for a later change.*
6. The query app reports the number of results, the time taken and the indexes used.

### Field rules

| Field | Business type | Rule |
|---|---|---|
| ID | Integer | Required and unique. It is the key and also the first field of the value, so that the value is self-contained. |
| Case Number | Text | Required. Not unique. |
| Date, Updated On | Date-time | Chicago local civil time as published, without time zone or conversion (decision D3). Nonexistent or repeated times caused by the daylight-saving change are kept without error. |
| Block, Primary Type, Description | Text | Required. |
| Location Description | Text | Optional. |
| IUCR, Beat, District, FBI Code | Code | Text that keeps the leading zeros (`0314`, `003`, `08B`). District is optional. |
| Arrest, Domestic | Yes/No | Required. |
| Ward, Community Area | Integer | Optional. |
| X Coordinate, Y Coordinate | Integer | Optional. |
| Latitude, Longitude | Decimal | Optional. |
| Year | Integer | Required. |
| Location | Derived text | Redundant with the input ("(latitude, longitude)"): not stored. |

## Validations and messages

### Generator

| Situation | Rule | Message to the user (verbatim Spanish UI text / English meaning) |
|---|---|---|
| Header different from the expected one | Aborts before processing any row | `La cabecera no coincide con el esquema: columna 5 esperaba "IUCR" y encontró "<valor>".`<br>*The header does not match the schema: column 5 expected "IUCR" and found "`<value>`".* |
| Row with a number of columns other than 22 | Logged and skipped | `Fila 1234 omitida: se esperaban 22 columnas y hay 21.`<br>*Row 1234 skipped: 22 columns were expected and there are 21.* |
| Non-numeric ID | Logged and skipped | `Fila 1234 omitida: el ID "<valor>" no es un número entero.`<br>*Row 1234 skipped: the ID "`<value>`" is not an integer.* |
| Duplicate ID | The first occurrence in the file is kept; the others are logged and skipped | `Fila 5678 omitida: el ID 14348063 ya apareció en la fila 1234.`<br>*Row 5678 skipped: the ID 14348063 already appeared in row 1234.* |
| Typed field that cannot be converted | Logged and skipped | `Fila 1234 omitida, columna "Date": "<valor>" no es una fecha válida (formato MM/DD/AAAA hh:mm:ss AM/PM).`<br>*Row 1234 skipped, column "Date": "`<value>`" is not a valid date (format MM/DD/YYYY hh:mm:ss AM/PM).* |
| Empty required field | Logged and skipped | `Fila 1234 omitida: la columna "Case Number" es obligatoria y está vacía.`<br>*Row 1234 skipped: the column "Case Number" is required and is empty.* |
| Empty optional field | Stored as "no value" | — |
| Generation finished with rejections | The dataset is published with no rejection limit; the summary and the manifest state how many records were rejected | `Generación completada: 8.656.900 registros válidos, 4 rechazados (ver log).`<br>*Generation completed: 8,656,900 valid records, 4 rejected (see log).* |
| Peak memory exceeds 512 MiB | Does not stop the generation; a warning is written to the log and the final summary reports it | `Aviso: el pico de memoria privada comprometida (580 MiB) supera el límite de 512 MiB.`<br>*Warning: the peak private committed memory (580 MiB) exceeds the 512 MiB limit.* |
| The output folder already contains a dataset | Replaced only with explicit confirmation | `La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.`<br>*The output folder already contains a dataset. Use the overwrite option to replace it.* |
| Not enough space on the temporary or output disk | Aborts and cleans up the temporary files | `Espacio insuficiente en <carpeta>.`<br>*Insufficient space in `<folder>`.* |
| Any abort | Leaves no partial dataset; the previous dataset, if there was one, remains intact | `Generación cancelada. No se ha modificado el conjunto existente.`<br>*Generation cancelled. The existing dataset has not been modified.* |

### Query app

| Situation | Rule | Message to the user (verbatim Spanish UI text / English meaning) |
|---|---|---|
| Folder without a dataset or with an incomplete one | Rejects | `No se encontró un conjunto de datos válido en <carpeta>.`<br>*No valid dataset was found in `<folder>`.* |
| Incompatible schema version | Rejects | `El conjunto usa el esquema v2 y este consultor admite v1. Regenera el conjunto.`<br>*The dataset uses schema v2 and this query app supports v1. Regenerate the dataset.* |
| Nonexistent ID | Empty result, not an error | `No existe ningún delito con ID 123.`<br>*There is no crime with ID 123.* |
| Unknown field | Rejects | `El campo "<nombre>" no existe. Campos disponibles: ...`<br>*The field "`<name>`" does not exist. Available fields: ...* |
| Value with an invalid format | Rejects | `"<valor>" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].`<br>*"`<value>`" is not a valid date. Expected format: YYYY-MM-DD [hh:mm].* |
| Inverted date range | Rejects | `La fecha inicial es posterior a la final.`<br>*The start date is later than the end date.* |
| Condition on a field without an index | Allowed, with a warning. *Note of 2026-10-13: replaced by decision C10 of the [query app PRD](consultor.prd.md); it is only accepted as a filter alongside an indexed condition.* | `Aviso: "<campo>" no tiene índice; la consulta recorrerá todos los registros.`<br>*Warning: "`<field>`" has no index; the query will scan all the records.* |

## Reuse

- **Previous design notes** (summarized conversation): sorting by numeric ID, FoundationDB tuple encoding
  for key and value, flat tuple with explicit nulls, 512 MiB budget and measured sort segments.
- **FoundationDB tuple encoding**: only its serialization format is reused, because it preserves the order
  of integers, strings and dates when bytes are compared. FoundationDB is not used as a database.
- **LevelDB table format** (decision D1): all the SSTables use the LevelDB table file format, with
  byte-by-byte key order, per-block checksums and optional Snappy compression. No custom format is
  invented. It is generated with a pure Rust library, without C++ dependencies.

## Out of scope

- Incremental updates, deletions or merging of extracts: every data change means regenerating the
  complete dataset.
- Automatic download of the CSV from the portal.
- Connection to a real FoundationDB cluster.
- Opening the dataset as a LevelDB database or ingesting it into RocksDB: the files are LevelDB tables
  readable one by one, not a complete database.
- Network server, API, graphical interface or interactive mode of the query app.
- Free-text search (in Block or Description) and fuzzy search.
- Geospatial queries (radius, polygon, grid).
- Aggregations and groupings beyond counting the results.
- Composite indexes (for example, type + date) and result ordering other than by ID. *Note of
  2026-10-13: result ordering remains out of scope and is planned for a later change (decision C11 of the
  [query app PRD](consultor.prd.md)); composite indexes remain out of scope.*
- Access control: the data is public.

## Acceptance criteria

The exact figures of AC1, AC4 and AC5 correspond to the reference CSV analyzed on 2026-10-12 (see
[Starting state](#starting-state)). With another compatible CSV, those results must match the valid
records actually processed.

1. **AC1** — Given the full CSV, when the generator runs, then the primary SSTable contains
   8,656,904 entries in strictly ascending ID order.
2. **AC2** — Given the full CSV, when the generator runs, then the measured peak private committed memory
   (see Main constraint) does not exceed 512 MiB and the final summary shows it.
3. **AC3** — Given a sample of IDs (the first, the last, one without coordinates and one without Ward), when
   they are queried by ID, then every returned field is equivalent in value to the one in the CSV row.
4. **AC4** — Given the primary type index, when `THEFT` is counted, then the result is 1,838,502, the same
   as in the CSV.
5. **AC5** — Given the community area index, when the "no value" records are counted, then the result is
   613,720.
6. **AC6** — Given a date range of one month, when it is queried, then the count matches the one obtained
   by filtering the CSV directly.
7. **AC7** — Given a combination of two indexed conditions (for example `THEFT` and district `008`), when it
   is queried, then the count matches the direct filtering of the CSV.
8. **AC8** — Given a case number shared by several records, when it is queried, then all of them are
   returned.
9. **AC9** — Given a CSV with an altered header, when the generator runs, then it aborts with the message
   from the table and the previous dataset remains intact.
10. **AC10** — Given a generated dataset, when an existing ID is queried, then the response arrives in
    less than 1 second on the reference machine.
11. **AC11** — Given any query, when it is requested in CSV or JSON, then the output is valid for that
    format and contains the same records as the table output.
12. **AC12** — Given a record whose time falls in the spring-forward gap (nonexistent in Chicago), when the
    dataset is generated and the record is queried by ID, then the generation does not fail and the
    returned date is exactly the published one.
13. **AC13** — Given a CSV with a row of 21 columns, an invalid date and a repeated ID, when the generator
    runs, then it finishes successfully, the dataset excludes those three rows, the log identifies each one
    with its row and reason, and the summary and the manifest report 3 rejected.

## Decisions

No decision remains open. All of them were made on 2026-10-12 and are already reflected in the body of
the PRD.

- **D1 — SSTable format** (2026-10-12): LevelDB table format generated with pure Rust.
  Reason: it is a standard and stable format, any LevelDB table reader can read the files, and it compiles
  on Windows without a C++ toolchain. Compatibility with RocksDB was ruled out because of that build
  friction.
- **D2 — Initial secondary indexes** (2026-10-12): date, primary type, district, community area and case
  number. Reason: the date enables range queries and the others are the usual filters with a useful
  cardinality. Arrest and Domestic (Yes/No) remain without an index because each value spans millions of
  records; they can be used as a condition with a full scan. *Note of 2026-10-13: decision C10 of the
  [query app PRD](consultor.prd.md) replaces the full scan; Arrest and Domestic are only used as a filter
  alongside an indexed condition.*
- **D3 — Date policy** (2026-10-12): the Chicago local civil time is stored as published, without
  converting it to UTC, and queries are also expressed in local time. Reason: the source states neither a
  time zone nor an offset. As measured, 253 records have a nonexistent time (the spring-forward gap) and
  1,663 fall in the repeated fall-back hour. Converting to UTC would force aborting or inventing the
  instant. Accepted consequence: the dates are not absolute instants and are not compared with data from
  other time zones.
- **D4 — Value content** (2026-10-12): the value includes the ID as its first field and omits Location.
  Reason for the ID: the value becomes self-contained and follows the CSV order, which makes generating
  CSV or JSON simpler. It costs 33 MiB (2.9% of the value) and its effect on memory and CPU is negligible.
  Reason for Location: it is redundant with the input and would take 251 MiB (22% of the value).
- **D5 — Invalid records** (2026-10-12): each invalid record is written to the log and skipped; the
  generation continues. Only errors that affect the whole file abort: a different header, an unreadable
  file or a lack of space. For a duplicate ID, the first occurrence in the file is kept, however far
  apart the repetitions are. There is no rejection limit. Reason: real data always brings bad records,
  and aborting would prevent making use of the good ones. Accepted consequence: a file with the correct
  header but broken content generates an almost empty dataset that replaces the previous one; the
  summary with the number of rejected records is the signal to detect it.
- **D6 — Query app usage mode** (2026-10-12): single-run commands; each query starts, answers and exits.
  Reason: it is the simplest to automate, to chain with other tools and to test. The interactive mode is
  left for a later change.

## References

- Previous design notes: `conversacion_rust_csv_foundationdb_sstable.md` (repository root).
- Crimes — 2001 to Present: https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2
- FoundationDB, data modeling and the tuple layer: https://apple.github.io/foundationdb/data-modeling.html
