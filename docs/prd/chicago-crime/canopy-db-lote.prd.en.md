# Query batch in the query app — PRD

**Version**: 1.0
**Date**: 2026-10-21
**Parent document**: [Chicago crime store on CanopyDB — PRD](canopy-db.prd.en.md) (v1.0)

> English translation of [canopy-db-lote.prd.md](canopy-db-lote.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Let the analyst run **several queries in a single run** of the original query app: they write a file with one
query per line, the query app opens the dataset once, runs the queries one after another in the order of the
file and finishes. This is decision K7 of the parent and the second child of its
[split](canopy-db.prd.en.md#split-into-child-prds).

**Why now**: the comparison benchmark of the CanopyDB module (K3) also measures queries in a batch, so as not
to measure mostly startup and cold opening. The CanopyDB query app will be born with the batch
(`canopy-db-consultor`) and the original needs it earlier, so that the two answer the same way to the same
thing (K1). This child does not depend on `canopy-db-generador` and touches nothing of the CanopyDB query app.

This document does not repeat the rules of the query app (C1–C11), of "starts with" (P1–P6) or of the
ancestry, which remain in force for each query of the batch. It only adds how the batch is requested, how the
file is written, what is output and how errors end. It replaces or refines these rules of the ancestry:

- **D6** of the [root parent document](chicago-crime.prd.en.md) (and step 1 of its flow B) and step 1 of the
  flow of the [query app PRD](consultor.prd.md): a run answers one query **or a batch of queries**. It is still
  a run that starts, answers and finishes; there is no interactive mode.
- **C6** of the query app: exit code 2 ("the command line is not valid") also applies to a badly formed batch
  line, which is the command line of its query (decision B4).
- **K8** of the parent: in the batch, what does not grow with the number of queries is the memory of each
  query; the already checked queries of the file may be kept (decision B7).

The annotations it leaves in those documents are in [Annotations on closing](#annotations-on-closing).

### Starting state

Verified on 2026-10-21 on the repository:

| Aspect | Situation |
|---|---|
| Original query app | Implemented and accepted (query app v1.1, AC-C1 to AC-C18; "starts with", AC-S1 to AC-S12). Answers a single query per run (D6). There is no batch option. |
| Checks | The shape of the command line is checked first (code 2); then the conditions (code 1), without opening the dataset; then the dataset is opened (code 1). An output file that cannot be prepared is rejected right before running the query (code 1). |
| Output | Results on standard output or in the `--output` file; warnings, errors and a statistics line on standard error (C5), with the form `1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.` With `--count`, only the number on standard output. If standard output is closed (for example, `head`), it ends with no message or statistics and with code 0. |
| Acceptance check | A Windows script runs the queries of the query app and "starts with" criteria against direct filtering of the CSV, checks the messages and cancellation with Ctrl+C, and measures the memory peak (AC-C13, AC-S8). It does not know the batch. |

## Actors

- **Analyst**: the one from the query app. The only actor of this delivery.

## Flow

It follows the flow of the query app; this delivery details flow C of the parent:

1. The analyst writes a batch file with one query per line (see [Batch file](#batch-file)).
2. They run the query app with `--dataset <CARPETA> --batch <ARCHIVO>` (decision B1).
3. The query app checks the command line, reads the file and checks each line, in order, with the same rules as
   a single query, and then the rules specific to the batch. All before opening the dataset: if anything is
   wrong, it rejects the batch with the first error and runs no query (decision B4).
4. It opens the dataset once and runs the queries one after another, in the order of the file, on the version
   it opened (flow C of the parent).
5. Before each query it writes its header on standard error. Then, the query does exactly what it would do on
   its own: it prepares its output file if it has one, and writes its warnings, its results and its statistics
   line (decision B3).
6. If a query fails or the analyst cancels, the batch stops there; what the previous queries already wrote is
   kept (decision B6).
7. When all have finished, it writes the final line of the batch with the number of queries and the total time.

### Batch file

Decision B2:

- **Encoding**: UTF-8, with or without a byte order mark at the start.
- **Line endings**: LF or CRLF, mixed or not; the last line may not end in a line break. Lines are numbered
  from 1 as they are in the file, counting empty lines and comments.
- **Ignored lines**: those that are empty or have only spaces and tabs, and those that start with `#` after
  optional spaces. A `#` anywhere else is just another character. A line with anything else, even if it is
  only empty quotes (`""`), is a query.
- **Words**: only spaces and tabs separate words, as on the command line. Double or single quotes group a text
  with spaces and may be opened in the middle of a word (`--where="Primary Type=THEFT"`); inside quotes, the
  other kind of quote is just another character (`"Location Description=O'HARE"`). There are no escape
  sequences: `\` is just another character. Any other character (a stray carriage return, a null character, a
  no-break space, a byte order mark that is not at the start) is just another character of its word.
- **Options**: each line accepts the options of a single query except `--dataset`, `--batch`, `--help` and
  `-h`, that is `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format` and `--output`, also in
  the form `--option=value` (decision B1).
- **Paths**: a relative `--output` path is resolved from the folder where the query app is run, as in a single
  query. To compare two paths (decision B5) they are resolved lexically: absolute path from the current
  folder, without `.` or `..`, without consulting the disk and, on Windows, ignoring case. So `crim.csv`,
  `./crim.csv` and `sub\..\CRIM.csv` are the same path, even if the file does not exist yet.

Example:

```text
# Robos y daños de enero de 2020
--where "Primary Type=THEFT" --range Date=2020-01-01..2020-01-31 --count
--startswith "Primary Type=CRIM" --range Date=2020-01-01..2020-01-31 --format csv --output crim.csv

--where ID=13209277 --format json
```

## Validations and messages

Each line of the batch has the validations, messages and codes of a single query (query app, "starts with" and
root parent), with the prefix `Línea <n>: ` before the message, without changing it (including the final hint
` Usa --help para ver las opciones.`). `<archivo>` is the `--batch` path and `<ruta>` the `--output` one, as
written. This delivery adds:

| Situation | Rule | Message to the user | Code |
|---|---|---|---|
| `--batch` together with `--where`, `--startswith`, `--range`, `--limit`, `--count`, `--format` or `--output` on the command line | Reject; those options go on each batch line. With several, the first one written is named | `Con --batch, la opción <opción> va en cada línea del lote, no en la línea de comandos.` | 2 |
| The batch file does not exist, is a folder or cannot be read (also `--batch ""` and `--batch -`) | Reject | `No se puede leer el lote <archivo>.` | 1 |
| The file is not valid UTF-8 | Reject | `El lote <archivo> no es un texto UTF-8 válido.` | 1 |
| The file has no query (empty, or only ignored lines) | Reject | `El lote <archivo> no contiene ninguna consulta.` | 1 |
| A line with unclosed quotes | Reject | `Línea <n>: falta cerrar unas comillas.` | 2 |
| A line with `--dataset`, `--batch`, `--help` or `-h` | Reject | `Línea <n>: la opción <opción> no se admite dentro del lote.` | 2 |
| A line that, on its own, would be rejected for its shape or its conditions | Reject | `Línea <n>: ` and the message of the single query | That of the single query (2 or 1) |
| A line with the same output file as an earlier line, comparing the paths as [Batch file](#batch-file) says | Reject | `Línea <n>: el archivo de salida <ruta> ya lo usa la línea <m>.` | 1 |
| A line whose output file is the batch file itself, with the same comparison | Reject | `Línea <n>: el archivo de salida <ruta> es el propio lote.` | 1 |
| A query fails during the batch: the dataset can no longer be read, its output file cannot be prepared or written, or standard output fails | Stop the batch (B6) | That of the single query, below the query header | 1 |
| Ctrl+C during the batch, also before the first query or between two queries | Stop the batch (B6). Between two queries, the next one does not start and its header is not written | `Consulta cancelada.` | 130 |
| Standard output is closed | Stop the batch with no message or final line, like a single query; what was already written on standard error (headers and statistics) stays | — | 0 |
| All queries run, with or without results or warnings | — | The final line of [Output](#output) | 0 |

**Order of the checks** (decision B4); only the first error is reported and its code is that of the result:

1. The command line, with the shape errors that already exist, which win: for example, `--batch` without
   `--dataset` gives the required-options message, a repeated `--batch` the invalid-arguments one and
   `--limit abc` the limit one, all with code 2. Then, the options that do not go with `--batch`. With
   `--batch`, the command line needs no condition.
2. Reading the file and its encoding.
3. Each line, in order: quotes, options not admitted in the batch, shape and conditions of the single query,
   and repeated output file or one equal to the batch.
4. That there is at least one query.
5. Opening the dataset, with the messages of the single query and without prefix.

None of these checks creates or modifies an output file.

## Output

Decision B3. A single query, without `--batch`, does not change its output.

- **Standard output**: what each query writes, one after another, exactly as it would write it on its own
  (table, CSV, JSON or the count number), with no added separators or blank lines. A query with `--output`
  writes nothing to it. So the output of a batch is the concatenation of the outputs of its single queries.
- **Standard error**: for each query, the header `--- Consulta <i> de <total> (línea <n>) ---`, and below it
  its warnings, its error if it fails and its statistics line, exactly as on its own. In the header and in the
  prefix `Línea <n>: ` the numbers have no thousands separator (`Consulta 1234 de 2000`). When all have
  finished, `Lote completado: <total> consultas en <t> s.` (`1 consulta` in the singular), with the numbers
  and the time in the format of the statistics line. The time covers the whole run: reading and checking the
  file, opening the dataset and all the queries. The final line is not written if the batch is rejected,
  fails, is cancelled or standard output is closed.
- **Output files**: each one has exactly the content it would have with the single query on the same version;
  in the JSON (C9), the dataset version is the same in all the queries of the batch.

Example with the file from [Batch file](#batch-file). Standard output:

```text
1234
{"query": …, "dataset": …, "results": [
…
]}
```

Standard error (times vary):

```text
--- Consulta 1 de 3 (línea 2) ---
1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 2 de 3 (línea 3) ---
2.345 resultados en 0,087 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 3 de 3 (línea 5) ---
1 resultado en 0,002 s. Índices usados: ID. Filtros sin índice: ninguno.
Lote completado: 3 consultas en 0,152 s.
```

If the second query could not write `crim.csv`, standard error would end like this, with code 1:

```text
--- Consulta 1 de 3 (línea 2) ---
1.234 resultados en 0,041 s. Índices usados: Primary Type, Date. Filtros sin índice: ninguno.
--- Consulta 2 de 3 (línea 3) ---
No se puede escribir el archivo de salida crim.csv.
```

## Reuse

- **Query app and "starts with"**: options, validations and messages, query resolution (C10), formats and
  output file (C4, C8, C9), output channels and codes (C5, C6), cancellation and memory (C7). Each query of
  the batch is checked and run with them, without parallel rules.
- **Statistics line** of the single query, with its number and time format, also for the final line of the
  batch.
- **Acceptance check of the query app**: extended with the batch checks and it keeps passing in full (AC-M9).

## Out of scope

- What the parent leaves out of the batch (parallel queries, batch from standard input, interactive mode). `-`
  as a batch file has no special meaning.
- The CanopyDB query app and its batch: delivered by `canopy-db-consultor`.
- Measuring the memory peak of a batch (AC-M8): done by `canopy-db-consultor` with both query apps (K8).
- A batch output that is a single valid CSV or JSON document: to separate clean results, each line uses its
  `--output`.
- Queries over several datasets in the same batch.
- Continuing the batch after a query that fails, resuming it from a line or running only some lines.
- Undoing the output files of the queries already finished when the batch fails or is cancelled.
- Checking before starting that the output files can be written: each one is checked on reaching its query, as
  in the single query.
- Detecting as equal two paths that lead to the same file through symbolic or hard links.
- Preventing an `--output` inside the dataset folder: the batch treats it like the single query, which does
  not prevent it today.
- Options common to all lines, variables, expansion of `~`, `%VAR%` or wildcards, line continuation,
  inclusion of other files and comments at the end of a line.
- Escape sequences in quotes, and therefore a value that contains both double and single quotes.
- Other file encodings, such as UTF-16.
- A limit on lines or on batch file size, and a time target for the batch.
- Changes to the generator, to the dataset format or to the independent verifier.

## Left to the implementation

- The text of the help, including the usage line, as long as it describes `--batch`, the shape of the file, the
  options each line accepts and the exit codes of the batch.
- How the file is read and kept, within B7.
- A Ctrl+C that arrives when the last query has already finished may end with the final line and code 0 or
  with `Consulta cancelada.` and code 130.

## Acceptance criteria

The figures correspond to the reference CSV. This child also verifies AC-M7 with the original query app and
AC-M9 of the parent; the memory of the batch (flow C, K8) is verified by `canopy-db-consultor` in AC-M8. The
times in the output are checked by their format, not by their value. The moments that cannot be provoked
deterministically from outside the process (a batch stopped between two queries or during one, a dataset that
can no longer be read, a standard output that fails) may be checked with tests that control that moment
without a real process; cancellation with a real Ctrl+C is checked at least once in the acceptance check.

1. **AC-B1** (AC-M7) — Given a batch with all the queries that the acceptance check of the query app and of
   "starts with" runs successfully on their own, when it is run, then each query returns the same count and
   the same set of records as on its own; those with a limit return as many records as the limit, all from the
   result without limit.
2. **AC-B2** (AC-M7) — Given a batch that has already opened the dataset and run its first query, when the
   generator publishes a new version before the second starts, then all its queries return results from the
   version it opened and the JSON of each one states that version.
3. **AC-B3** (B2) — Given a batch with empty lines, comments, mixed LF and CRLF line endings, a byte order
   mark, a last line without a line break, double and single quotes, quotes in the middle of a word,
   `--option=value` and a `#` inside a value, when it is run, then each query gives the same as on its own.
4. **AC-B4** (B4) — Given a batch whose third query, on line 5 after an empty line and a comment, would be
   rejected on its own for its shape, and another like it that would be rejected for its conditions, when they
   are run, then no query is run, no output file is created, the message is `Línea 5: ` and that of the
   single query, and the code is 2 and 1. Given a batch with two badly written lines, then only the first one
   is reported.
5. **AC-B5** — Given each row of [Validations and messages](#validations-and-messages), when it is provoked,
   then the indicated message is shown with its code.
6. **AC-B6** (B3) — Given a batch with a query in table format, one in CSV with `--output` and a count, when
   it is run, then standard output is exactly the concatenation of the standard outputs of the three single
   queries, the file is identical to that of the single query, and standard error has the form of
   [Output](#output): one header per query, its statistics line and the final line. With a batch of a single
   query, the final line says `1 consulta`.
7. **AC-B7** (B6) — Given a batch of three queries whose second writes to a file that cannot be created (in a
   folder that does not exist, and in another test, a path that is a folder), when it is run, then the first
   keeps its results, standard error ends with the header of the second and its message, the third is not
   run, there is no final line and the code is 1.
8. **AC-B8** (B6) — Given a batch of three queries with an output file, stopped during the second, when the
   analyst presses Ctrl+C, then it ends with `Consulta cancelada.` and code 130, the file of the first exists
   complete, that of the second does not exist and no temporary file of it remains, that of the third does
   not exist and there is no final line.
9. **AC-B9** — Given `--help`, when it is shown, then it describes `--batch`, the shape of the file, the
   options each line accepts and the exit codes of the batch; and the README and the query app guide explain
   the batch with an example.
10. **AC-B10** (AC-M9) — Given the tests and the acceptance check of the original applications, when they are
    run, then all pass without changing what they check, except the help text, plus the new ones of the
    batch.

## Decisions

All were made with the user on 2026-10-21.

- **B1 — How the batch is requested**: `--batch <ARCHIVO>` together with `--dataset`. Each line accepts all
  the options of a single query except `--dataset`, `--batch`, `--help` and `-h`; with `--batch`, those
  options are not accepted on the command line. Reason: K7 says each line is written with the same options as
  a single query, and the style follows that of `--where` and `--startswith`. Discarded: a positional
  argument, a subcommand, and global `--format` and `--output` or as a default value for the lines, which add
  combination rules and would force gathering results into one file.
- **B2 — Shape of the file**: that of [Batch file](#batch-file). Words as in a shell without escapes, to copy
  queries from a console as they are and to write Windows paths without doubling `\`; comments only on their
  own lines, so as not to break a value with `#`; UTF-8 with or without a byte order mark and LF or CRLF, to
  accept what Notepad and PowerShell 5.1 save. Discarded: quotes only at the start of a word, escapes with
  `\`, end-of-line comments, strict UTF-8 without mark and UTF-16.
- **B3 — Output and separation**: standard output receives only what each single query would write; the
  identification of each query, as a header, and the final line with the total time go to standard error.
  Reason: it respects C5, which exists to save or chain CSV and JSON as they are, and reuses the warnings and
  the statistics line without touching them. Discarded: the separator on standard output (breaks C5 and
  AC-C8), the separator only in table format, a prefix on each line of standard error, the time as the sum of
  the queries and having no final line.
- **B4 — Prior check and codes**: the whole file is checked before opening the dataset, in the order of
  [Validations and messages](#validations-and-messages), and any error rejects the batch without running
  anything. Only the first error is reported and its code is that of the result: a badly formed line gives 2,
  like the command line of its query (extends C6); an unreadable file, one without queries or with a repeated
  output file gives 1, like a rejected datum. The messages of the single query are reused as they are after
  `Línea <n>: `. Reason: an error that can be seen beforehand must not leave half results. Discarded:
  reporting all the errors, failing on reaching the line, and giving the whole file a single code.
- **B5 — Repeated output file**: two lines with the same `--output`, or an `--output` that is the batch file
  itself, reject the batch before starting, comparing the paths lexically (see [Batch file](#batch-file)),
  without consulting the disk. An `--output` inside the dataset folder is treated as in the single query.
  Whether an output file can be written is checked on reaching its query, as in the single query. Reason:
  avoid losing results or the batch itself without notice, without creating anything too early. Discarded:
  overwriting without a new rule, comparing only the text of the path, resolving the real path on disk
  (requires the folder to exist), preparing all the files before starting and rejecting an `--output` inside
  the dataset folder only in the batch.
- **B6 — Failures and cancellation during the batch**: the batch stops at the query that fails (code 1) or at
  the one in progress when Ctrl+C is pressed (`Consulta cancelada.`, code 130); that query leaves no
  half-written output file, the previous ones keep their results and their files, the next ones are not run
  and the final line is not written. Each query thus complies with C8 separately. Reason: what finished is
  valid and an unreadable dataset would make all the following ones fail. Discarded: continuing with the
  rest, undoing the previous files and a cancellation message of the batch's own.
- **B7 — Memory**: refines K8 for the batch. What does not grow with the number of queries is the memory each
  one uses: none keeps its results or anything derived from its query for the next; the only thing in common
  is the open dataset. The already checked queries of the file may be kept, and that grows with the size of
  the file, negligible next to the peak of a query. Reason: the prior check of B4 needs the whole file.
  Discarded: a peak independent of the file size, which would force reading it twice and rejecting it if it
  changes between the two readings.

## Annotations on closing

On closing this child, these are noted, without rewriting its history:

- **Root parent document**, in decision D6 and in step 1 of flow B: *Annotation of <date>: partly replaced by
  the [query batch PRD](canopy-db-lote.prd.md) (decision K7 of the [CanopyDB PRD](canopy-db.prd.en.md)); a
  run of the query app answers one query or a batch of queries, and it still starts, answers and
  finishes.*
- **Query app PRD**, in step 1 of the flow ("Cada ejecución responde una consulta y termina (D6)") and in C6:
  the same annotation, and in C6 also *exit code 2 also applies to a badly formed batch line (decision B4 of
  the [query batch PRD](canopy-db-lote.prd.md)).*
- **CanopyDB PRD**, in K8: *Annotation of <date>: in the batch, the already checked queries of the file may be
  kept; what does not grow is the memory of each query (decision B7 of the
  [query batch PRD](canopy-db-lote.prd.md)).*

The first two are those required by the table
[Criteria and annotations per child](canopy-db.prd.en.md#criteria-and-annotations-per-child) of the parent;
the one in C6 and the one in K8 are added because of decisions B4 and B7.

## References

- Parent document: [canopy-db.prd.en.md](canopy-db.prd.en.md)
- General parent document: [consultor.prd.md](consultor.prd.md)
- Root parent document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
- ["Starts with" PRD](startswith.prd.md)

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| ea0ef110 | 2026-10-21 07:24 UTC+01:00 | 24 min | 0 min | 24 min | 15 min | 8 min | 18 |

<!-- prd-time: [{"session": "ea0ef110-7116-4e15-bcfe-d1c9b40cd59a", "start": "2026-10-21T06:24:22.769000+00:00", "wall": 1428, "discarded": 0, "agent": 921, "human": 507, "interventions": 18, "idle_minutes": 10}] -->
