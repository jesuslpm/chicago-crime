# crime-query — ODD feature document

- **Locator**: `odd/tasks/crime-query.md` · Engram mirror topic `odd/crime-query/tasks`
- **Branch**: `feat/crime-query` from `main` at `4c74bfe`. Delivery: local work-unit commits, no PRs, merge into
  `main` after full verification (user rule). Test runner: `cargo test --workspace` (baseline 230 green).
- **Timing**: estimate **4–7 h** of agent work (no calibration reference; generator times deliberately not
  used). Real start **2026-10-13 16:06:57 +0100**. Waits for the user: none so far (logged in `## Log`).

## Specs

Product source: `docs/prd/chicago-crime/consultor.prd.md` **v1.1** (C1–C11, AC-C1…AC-C18), parent
`docs/prd/chicago-crime/chicago-crime.prd.md` v1.2 (flow B, query validations, field rules, D1–D6). The PRDs
win on any product question; the specs below quote their rules and messages verbatim and do not repeat
everything else.

- **S1 — C1 in the generator (schema 2).** "el generador guarda en minúsculas los valores de los índices
  secundarios de texto (tipo principal, distrito y número de caso), y el consultor convierte a minúsculas el
  valor buscado antes de usar el índice"; "la tabla principal conserva cada campo exactamente como viene en el
  CSV"; "sube la versión del esquema a 2". The independent verifier "confirma que los índices de texto están
  en minúsculas y que la tabla principal coincide con el CSV tal cual" (AC-C18).
- **S2 — Open the dataset.** The analyst gives the generator's output folder. The query app resolves the
  active version, reads its manifest and checks the schema version; "Si el generador publica una versión
  nueva mientras tanto, la consulta en curso termina sobre la versión que abrió" (AC-C12). Messages:
  `No se encontró un conjunto de datos válido en <carpeta>.` ·
  `El conjunto usa el esquema v2 y este consultor admite v1. Regenera el conjunto.` (filled with the real
  versions; a schema-1 dataset is rejected, AC-C18) ·
  `No se pudo leer el conjunto de datos en <carpeta>.` ("Aborta sin salida parcial engañosa"). The query app
  never writes in the dataset folder.
- **S3 — Conditions (flow step 3, C1–C3, C10).** ID (one or several); case number (one or several, all
  records sharing it); equality on primary type, district or community area with alternative values (OR),
  including "sin valor" on optional fields; date range with inclusive from/to, either end optional; equality
  on a non-indexed field "solo se admite combinada con al menos una condición indexada". All conditions AND.
  Text compared case-insensitively ("`theft`, `Theft` y `THEFT` son equivalentes"); "Los códigos conservan
  los ceros a la izquierda y no se completan (`8` no equivale a `008`)"; non-indexed filters lowercase both
  sides; non-indexed equality on "cualquier campo almacenado salvo los de texto largo (Block y
  Description)". C2: "formato `AAAA-MM-DD [hh:mm]` … en hora civil local (D3). Un "desde" sin hora empieza a
  las 00:00:00; un "hasta" sin hora incluye el día entero, y con hora incluye el minuto entero. Se admite
  solo "desde" o solo "hasta"."
- **S4 — Validations (parent + child tables, AC-C10), verbatim:**
  `Indica al menos una condición sobre un campo indexado: ID, número de caso, fecha, tipo principal, distrito o área comunitaria.` ·
  `No existe ningún delito con ID 123.` (several IDs: return the existing ones, warn for each missing one;
  single missing ID: empty result, not an error) ·
  `No existe ningún delito con número de caso JA000000.` (empty result, not an error) ·
  `El campo "<nombre>" es obligatorio; no admite "sin valor".` ·
  `"<valor>" no es válido para "<campo>": se esperaba un número entero.` ·
  `Solo se admiten rangos sobre "Date".` · `El límite debe ser un número entero mayor que 0.` ·
  `Consulta cancelada.` (Ctrl+C, no half-written output file) ·
  `"Solo conteo" no se combina con un formato de salida.` ·
  `El archivo de salida solo admite los formatos CSV o JSON.` ·
  `No se puede escribir el archivo de salida <ruta>.` ·
  failure after the file was started: "No queda ningún archivo a medias con el nombre pedido" + the cause ·
  parent: `El campo "<nombre>" no existe. Campos disponibles: ...` ·
  `"<valor>" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].` ·
  `La fecha inicial es posterior a la final.`
- **S5 — Resolution (C10).** "cada condición indexada … se resuelve con su índice y da un conjunto de ID; con
  varios valores alternativos, el conjunto es la unión; con varias condiciones indexadas, el resultado es la
  intersección de sus conjuntos de ID; las condiciones sin índice se aplican después, como filtro sobre los
  registros de esa intersección. El consultor nunca recorre la tabla principal entera." No cost-based plan
  (out of scope).
- **S6 — Output (C4, C5, C9, C11).** Limit, count only, format table/CSV/JSON. "CSV y JSON incluyen todos los
  campos almacenados en el orden del CSV … con Location reconstruida … para que el CSV tenga la misma
  cabecera que el de origen. La tabla legible muestra una fila por registro con todos los campos en
  columnas". "los resultados van a la salida estándar; avisos, errores, número de resultados, tiempo e
  índices usados van a la salida de errores" (plus non-indexed filters applied, flow step 7). JSON contains
  "la consulta interpretada … cada condición tal como se aplicó (con los valores y los rangos de fecha
  resueltos a instantes exactos según C1 y C2), el límite si lo hubo y la versión del conjunto consultada".
  Order unspecified; "Con un límite N se devuelven N registros cualesquiera de ese conjunto".
- **S7 — Output file (C8).** With CSV or JSON a file can be given; results go there instead of stdout; "Si el
  archivo ya existe, se sobrescribe siempre, sin pedir confirmación. Una consulta rechazada o fallida no deja
  un archivo a medias con el nombre pedido."
- **S8 — Exit codes (C6).** "0 si la consulta se ejecuta, aunque no haya resultados; 1 si se rechaza por el
  conjunto o por los datos de la consulta, o si aborta; 2 si la línea de comandos no es válida; 130 si el
  analista la cancela con Ctrl+C."
- **S9 — Memory and performance (C7, AC-C7).** "el consumo no crece con el tamaño del resultado. Contar
  `THEFT` (1,8 millones) usa una memoria comparable a la de contar un tipo raro." An existing ID answers "en
  menos de 1 segundo en el equipo de referencia".
- **S10 — Acceptance.** AC-C1…AC-C18 on the reference CSV, counts checked against "un recuento independiente
  sobre el CSV que no usa código del consultor".

## Implementation decisions (defaults chosen without asking, per the user's standing rule)

| Topic | Decision | Rationale |
|---|---|---|
| Crate | `crates/crime-query` (lib + bin `crime-query`), depends on `crime-dataset`, `sstable`, `clap`, `csv`, `serde`, `serde_json`, `ctrlc`, `tempfile` (all already workspace deps). | Same shape as the generator; the contract crate is the only shared code. |
| Lowercase | One function in `crime-dataset`, `index_text(&str) -> String` = `str::to_lowercase`, used by `index_key` and by the query app; the verifier re-implements it. | One definition of the index form (C1). |
| CLI | `crime-query --dataset <CARPETA> [--where CAMPO=VALOR]... [--range Date=DESDE..HASTA]... [--limit N] [--count] [--format table\|csv\|json] [--output ARCHIVO]`. Field names are the CSV header names, matched case-insensitively. Repeating `--where` on the same field = alternatives (OR); different fields = AND. `CAMPO=` (empty value) = "sin valor". Either range end may be empty (`Date=2020-01-01..`). Several `--range` intersect (AND). | PRD fixes semantics, not syntax. One generic syntax makes every PRD validation reachable (unknown field, range on a non-date field, wrong type). |
| Non-conditionable fields | Block, Description, Location: `El campo "<nombre>" no admite condiciones. Campos disponibles: ...` (new message; same list as the unknown-field message). | C3 excludes them; "no existe" would be false for real CSV columns. |
| Equality on dates | `--where Date=V` / `--where "Updated On=V"` match the whole day or minute of V (C2 granularity). Date equality is part of the date condition (alternatives OR, merged with ranges by AND). | Consistent with C2; no new date semantics. |
| Booleans | Arrest/Domestic accept `true`/`false` (CSV form) and `sí`/`si`/`no`, case-insensitive. Wrong value: `"<valor>" no es válido para "<campo>": se esperaba true o false.`; decimals: `… se esperaba un número decimal.` | Mirrors the PRD's integer message. |
| Exit 2 vs 1 | 2: unknown/missing option, malformed `--where`/`--range` (no `=`/`..`), non-numeric or non-positive limit, count + format, output file with table/count. 1: field/value/date problems, no indexed condition, dataset errors, I/O. | C6: "2 si la línea de comandos no es válida"; condition content is "datos de la consulta". |
| Plan (C10, no cost model) | Fixed rule. **With a date condition**: stream the date index over its range(s) (unsorted IDs) and test every other indexed condition per ID by an index point lookup of `(value, id)` (membership in that index's ID set); ID condition = membership in the requested list. **Without a date condition**: each alternative of each equality condition is an ID-ascending stream of its index (ID list: requested IDs that exist, sorted); alternatives merge by union, conditions by sorted-merge intersection. Non-indexed filters on the decoded primary record afterwards. Primary records are read only when the output or a filter needs them. | Every indexed condition is resolved through its own index; memory is O(number of conditions + alternatives), never O(result) (C7). THEFT is read completely in the no-date case, the PRD's known worst case. |
| Snapshot | Resolve `CURRENT`, read the manifest, then open every table file handle before reading any data; if a file vanished between `CURRENT` and the open, re-resolve (3 attempts). Windows file handles opened by Rust allow delete, so a new publication cannot disturb the open snapshot. | AC-C12. |
| Read errors | Table reads go through a `RandomAccess` wrapper that records I/O errors; the `sstable` iterator swallows block errors and skips to the next block, so the run checks the wrapper after every scan and aborts with S2's read message. Known limit: a block with a bad checksum is still skipped silently by the crate. | Avoids misleading partial output (PRD row). |
| Count only | Prints the bare number on stdout. Limit caps the count too. | Machine-friendly; "limit N returns at most N". |
| Table | Fixed column widths per field (header or typical max), values never truncated (a longer value shifts that row). | Streaming output; C7 forbids buffering all rows to size columns. |
| CSV | Header = the 22 source columns; every field quoted like the source; dates in the source text; booleans `true`/`false`; Location rebuilt; "sin valor" empty. | C4. |
| JSON | One object `{"query": …, "dataset": {"version": <folder>, "generated_at": …, "schema_version": 2}, "results": [ … ]}` written as a stream; records keyed by CSV header in CSV order, typed (ints, floats, booleans, null); query dates resolved as `YYYY-MM-DDTHH:MM:SS` (civil, no zone). | C9; streaming keeps memory flat. |
| Output file | Written to a temp file in the target folder, renamed over the target on success; dropped on rejection, failure or Ctrl+C. | C8. |
| Stats (stderr) | `<n> resultados en <t> s. Índices usados: <names>. Filtros sin índice: <fields \| ninguno>.` with Spanish thousands. | C5, flow step 7. |
| Acceptance independence | CSV-side counts come from a new `count` mode of `crime-acceptance` (csv crate only, no `crime-dataset`/`crime-query` code), driven by an extended PowerShell acceptance script. | "recuento independiente sobre el CSV". |

Carried-over advisory from the generator feature: `R3-manifest-schema-version-unchecked` is solved by S2.
`R3-decoder-accepts-empty-and-nonfinite` stays out of scope (not in the PRD); reported at close.

## Tasks

Checks for every code task: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`. Test-first: observe RED on a relevant deterministic test before GREEN. Route: the
parent context is past the ~150k advisory backstop, so each task goes to **one bounded delegated writer**
(context trigger); tasks run sequentially (each builds on the previous one).

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 — C1: lowercase text indexes, `index_text`, schema 2 in `crime-dataset`, generator tests, verifier (lowercase keys, schema 2) | S1 | delegated writer | pending | — |
| T2 — `crime-query` dataset layer: snapshot open with retries, manifest + schema check, error-tracking reader, record get, index streams (ID-ascending per value, date range), membership probe, case-number existence | S2, S5 (access), S9 | delegated writer | pending | — |
| T3 — Query model + CLI: field catalog, `--where`/`--range` parsing, value typing, C1 lowercasing, C2 date resolution, every S4 validation message, clap in Spanish, exit codes 1/2 | S3, S4, S8 | delegated writer | pending | — |
| T4 — Execution: fixed plan, union/intersection streams, probes, filters, limit, count, missing-ID/case warnings, stderr stats | S5, S6 (stats), S9 | delegated writer | pending | — |
| T5 — Output: table, CSV, JSON with the interpreted query, output file (temp + rename), Ctrl+C → 130 | S6, S7, S8 | delegated writer | pending | — |
| T6 — Acceptance: regenerate the reference dataset (schema 2), verifier run (AC-C18), `crime-acceptance count`, acceptance script for AC-C1…AC-C18, README | S10 | delegated writer + verification run | pending | — |

Forecast: ~3,500 authored changed lines. RDD: on (global). After each commit run
`gentle-ai review assess --committed-only` against the last reviewed boundary (first boundary `4c74bfe`);
consent always granted (standing user instruction); at feature close review whatever stayed under budget.

## Log

- **L1 (2026-10-13, user, verbatim excerpt)**: "Empezamos el flujo ODD para implementar el consultor (query app)
  de chicago-crime. Fuente de verdad: PRD: docs/prd/chicago-crime/consultor.prd.md v1.1 (decisiones C1–C11,
  criterios AC-C1…AC-C18). … Cambios de alcance de la v1.1 (Engram puede tener versiones viejas; manda el
  PRD): C11: el orden de los resultados NO se garantiza y puede variar; el analista no puede pedir orden; un
  límite N devuelve N registros cualesquiera del resultado. Ordenar queda para un cambio posterior. C7: sin
  ordenación externa ni carpeta temporal; solo rige que la memoria no crece con el tamaño del resultado. Fuera
  de alcance: estadísticas y elección de plan por costes. Peor caso conocido: THEFT (1.838.502 registros,
  21 %), cuyos ID se leen completos aunque otra condición sea más selectiva. C1 implica un cambio en el
  generador: índices de texto en minúsculas, esquema 2. … Sin preguntas a mitad de proceso … Flujo ODD por
  defecto: rama de feature, un commit local por tarea, assess de riesgo tras cada commit, revisión cuando
  toque … Consentimiento de revisión SIEMPRE concedido. Al cerrar el feature, revisá también lo pendiente por
  debajo del umbral. Sin push hasta que te lo pida. Al terminar, merge de la rama del feature a main. … Al
  cerrar: tiempo real vs estimado; informe de archivos tocados con líneas añadidas y eliminadas por área …
  Pendiente menor: en .claude/settings.local.json la regla "ask" de git push no pidió confirmación … No
  confíes en esa regla"
- **L2 (2026-10-13 16:07–16:40)**: Engram updated: #839 (C11), #840 (C7) rewritten to v1.1; #845 saved (no cost
  planner). Baseline: `main` 4c74bfe, 230 tests green, RDD on (global). No `git push` will be run at all.
  `sstable` 0.11.1 reader facts: `TableIterator::advance` swallows block read errors and jumps to the next
  block; `seek` past the end resets the iterator; `Table::get` returns errors.
