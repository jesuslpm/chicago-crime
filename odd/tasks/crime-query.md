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
| Read errors | Table reads go through a `RandomAccess` wrapper that records I/O errors; the `sstable` iterator swallows block errors and skips to the next block, so the run checks the wrapper after every scan and aborts with S2's read message. Since T2a the wrapper also verifies each block's masked CRC32C, so a corrupt block is a read error too. | Avoids misleading partial output (PRD row). |
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
| T1 — C1: lowercase text indexes, `index_text`, schema 2 in `crime-dataset`, generator tests, verifier (lowercase keys, schema 2) | S1 | delegated writer | **done**: 236/236; verifier S1 PASS; RDD approved `review-9f2da850bb2a217a` | `c8db165` |
| T2 — `crime-query` dataset layer: snapshot open with retries, manifest + schema check, error-tracking reader, record get, index streams (ID-ascending per value, date range), membership probe, case-number existence | S2, S5 (access), S9 | delegated writer | **done**: 269/269; native assess high; verifier PASS; RDD approved `review-a365970a61ce8d44` | `7a752e3` |
| T2a — Fixes from the T2 verification and review: `TrackedFile` verifies the masked CRC32C of every table block it reads (blocks are read as content + type byte + CRC in one `read_at`) so a corrupt block becomes a `ReadError` instead of a silently short stream (`R4-001`, verifier advisory 1); the AC-C12 snapshot test asserts the old version folder is gone before reading through open handles (`R3-001`); a test where `CURRENT` moves to a new version between open attempts and a later attempt succeeds (`R3-002`) | S2 | delegated writer (parallel with T4, disjoint files) | **done**: committed tree 347/347, clippy, fmt (worktree check) | `c67f70d` |
| T3 — Query model + CLI: field catalog, `--where`/`--range` parsing, value typing, C1 lowercasing, C2 date resolution, every S4 validation message, clap in Spanish, exit codes 1/2 | S3, S4, S8 | delegated writer | **done**: 337/337 (parent re-ran + binary spot checks); assess medium `under_budget` (2,656 lines) → pending in the slice | `005e565` |
| T4 — Execution: fixed plan, union/intersection streams, probes, filters, limit, count, missing-ID/case warnings, stderr stats | S5, S6 (stats), S9 | delegated writer (parallel with T2a) | in progress | — |
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
- **L3 (16:50–17:00, T1)**: writer delivered S1 (`c8db165`): `index_text` = `to_lowercase` in `crime-dataset`,
  `index_key` lowercases primary type, district and case number, `SCHEMA_VERSION` 2; generator tests updated plus a
  mixed-case test (index lowercase, primary keeps CSV text); verifier lowercases expected keys, expects schema 2 and
  adds `AC-C18-LOWERCASE`. RED observed first (index keys held `ARSON`/`Theft`, manifest 1 vs 2, 18 criteria
  expected). fmt/clippy clean, 236/236 (parent re-ran 236/236). Native assess: medium, `under_budget` (521 lines);
  raised by the parent with `--escalate-item 1` (persisted index format change) → `review_due` high. Pending for T6:
  the acceptance script does not yet map `AC-C18-LOWERCASE` to a result row.
- **L4 (17:00–17:20, T1 verification)**: independent verifier (scratch clones under the system temp dir): S1 PASS, no
  blockers. Mixed-case fixture → index values lowercase (`theft`, `00a`, `ja123`, `jé900`), codes keep zeros (`008` ≠
  `8`), primary keeps the CSV text; manifest schema 2; verifier 18/18 PASS; baseline dataset fails the new verifier
  (7/18); `primary.sst`, `index-date.sst`, `index-community_area.sst` byte-identical baseline vs T1. Advisories (not
  caused by T1): decimals with trailing zeros (`41.80`) are stored typed, so AC-G3 cannot match their text (D4, baseline
  behavior); case numbers that differ only in case now group together (consequence of C1). RDD on a clean worktree at
  `c8db165` (the shared tree had T2's untracked files): lenses risk + reliability, consent granted (standing
  instruction) → **approved**, acknowledged (lineage `review-9f2da850bb2a217a`, authority burned). Reviewed boundary
  → `c8db165`. Suggestions: `R3-001` (empty-dataset behavior of `AC-C18-LOWERCASE` untested), `R3-002` (no non-ASCII
  `index_text` test where lowercase changes byte length) — accepted as advisory, folded into T6.
- **L5 (17:20–17:35, T2)**: writer delivered `7a752e3`: `crates/crime-query` library (dataset, index streams, tracked
  reader, messages) + 30 integration and 3 unit tests (RED against a panicking stub). New `sstable` 0.11.1 findings:
  `seek` to a target between a block's last key and its separator leaves the iterator invalid (workaround with
  `approx_offset_of` + bounded advance, pinned by a test over 2,999 range starts); block reads ignore short reads (the
  wrapper turns early EOF into an error); the reader panics on a file shorter than the footer or without the magic (checked
  before `Table::new`). Windows: the generator's overwrite deleted the old version folder while the query held handles,
  and the open snapshot kept reading old data (AC-C12 behavior confirmed in a test). Added `foundationdb-tuple` to the
  crate (key decoding). Native assess high → independent verifier launched; T3 writer launched in parallel (disjoint
  files, verifier works in a temp clone).
- **L6 (17:35–17:55, T2 verification)**: independent verifier PASS (S2, S5 access, S9), no blockers: 90 value queries,
  22,368 + 500 date-range queries (0 mismatches; disabling the seek workaround gives 9), 502 records, every open message
  verbatim, snapshot under `--overwrite` holds, truncation after open → read error, folder hashes unchanged after ~27k
  queries, peak private 1.0 MB (ARSON) vs 2.2 MB (THEFT 200k) — growth is the bounded block cache (2,048 blocks ≈ 8 MiB).
  Advisories: (1) in-place corruption keeping the size is skipped silently (CRC failures never reach the wrapper; fix:
  check the CRC in `TrackedFile`) → T2a; (2) invalid manifest JSON reports "No se encontró…" (defensible: "carpeta sin
  conjunto o incompleta"); (3) judge AC-C13 with the ≈8 MiB cache in mind (T6). RDD on a clean worktree at `7a752e3`
  (base `c8db165`), lenses resilience + reliability, consent granted → **approved**, acknowledged (lineage
  `review-a365970a61ce8d44`). Boundary → `7a752e3`. Warnings `R4-001`, `R3-001` and suggestion `R3-002` → T2a.
- **L7 (17:55–18:15, T3)**: writer delivered `005e565`: `fields.rs` (22-column catalog), `query.rs` (typed query,
  filters, C2 intervals, JSON view), `cli.rs` (clap in Spanish; derive + `ArgMatches::indices_of` to keep the order of
  `--where`/`--range`), interim `main.rs`, `CivilDateTime::{MIN, MAX, from_parts, to_iso_string}` in `crime-dataset`
  (additive; the parser now builds through `from_parts`). 66 new tests (RED: compile failures on the missing API).
  Defaults the writer chose: range messages `El rango "<texto>" no tiene la forma Date=DESDE..HASTA.` and `El rango
  "<texto>" necesita al menos una fecha: Date=DESDE.., Date=..HASTA o Date=DESDE..HASTA.`; `El formato "<x>" no es
  válido. Formatos admitidos: table, csv, json.`; `--format` case-insensitive; `NaN`/`inf` rejected as decimals; open
  range ends shown as `0001-01-01T00:00:00`/`9999-12-31T23:59:59` in the JSON view; `--range` on any non-Date known
  field (Block included) → `Solo se admiten rangos sobre "Date".`. Parent spot checks: missing dataset exit 1, Block →
  "no admite condiciones" exit 1, `--count --format csv` exit 2. Assess: medium, `under_budget`. T2a and T4 launched
  in parallel (declared disjoint surfaces; the parent owns git).
- **L8 (18:15–18:35, T2a)**: writer delivered `c67f70d`: `TrackedFile` verifies the masked CRC32C of every block read
  (in-house CRC32C + LevelDB unmask, no new dependency; reads of exactly the last 48 bytes are the footer; the
  `filter._` metaindex entry written by `new_no_filter` is never read because the reader looks up
  `filter.leveldb.BuiltinBloomFilter2`, pinned by a unit test). RED: zeroing 64 bytes in `index-primary_type.sst`
  returned IDs 852..3000 as success; now a read error. Snapshot test asserts the old folder is gone (Windows deletes it);
  retry-success test through a crate-private seam `open_observed`. The committed tree (T3 + T2a, without T4's in-progress
  files) was checked in a separate worktree: 347/347, clippy and fmt clean. Assess of the slice `7a752e3..c67f70d`
  (T4's untracked files excluded with `--untracked-scope=exclude`): **high** (`process_boundary`, 3,120 lines) →
  independent verifier launched, then RDD.
