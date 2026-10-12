# Dataset generator — PRD

**Version**: 1.0
**Date**: 2026-10-12
**Parent document**: [Queryable Chicago crime store on SSTables — PRD](chicago-crime.prd.en.md) (v1.2)

> English translation of [generador.prd.md](generador.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Deliver the **generator**, the first of the two applications in the parent document. From the Chicago
crime CSV, it produces the complete dataset: the primary SSTable, the five secondary index SSTables and
the manifest. The query app is left for a later change.

This document does not repeat the parent's rules, which remain in force: generation flow A, field rules,
generator validations, memory constraint and decisions D1–D5. It only adds what this delivery needs and
the parent leaves open:

- the boundary of the delivery;
- how the generator is verified without a query app;
- the operational details of its use by the operator: log, exit codes, cancellation and leftovers from
  interrupted runs.

### Starting state

Verified on 2026-10-12:

| Aspect | Situation |
|---|---|
| Product | The parent document v1.2 is closed, with decisions D1–D6 made. |
| Code | No Rust code or project exists. |
| Data | The reference CSV described in the parent is available: 8,656,904 records, no duplicate IDs and unsorted. |
| Tracking | There is no open implementation document. |

## Actors

- **Generator operator**: the one defined in the parent document. It is the only actor in this delivery.

## Flow

It follows flow A of the parent document. This delivery specifies the following points:

1. The operator runs the generator, specifying the input CSV and the output folder. Optionally, they
   specify the temporary folder, the log location (by default, next to the output folder; decision G1),
   whether overwriting an existing dataset is allowed and whether compression, which is enabled by
   default, is disabled (decision G2).
2. Before processing any row, the generator checks that it can read the CSV, write the log and use the
   temporary folder. If a previous generation was interrupted and left leftovers, it deletes or ignores
   them, without them affecting the new generation or the published dataset.
3. During the process it shows the progress on the console and writes the rejected records and the
   warnings to the log, according to the parent's rules. The log is a separate file for each run, outside
   the dataset folder, so that it survives even if the generation fails; its content is also shown on the
   console.
4. The operator can cancel at any time (Ctrl+C). The cancellation is treated as a failed generation: the
   temporary files are cleaned up and the previous dataset remains intact.
5. On finishing, the final summary also states where the log is. The exit code is 0 when the generation
   finishes, even if there are rejections or a memory warning, and other than 0 when it aborts or is
   cancelled (decision G3).
6. The manifest identifies the source CSV by its name, its size and a fingerprint (hash) of the content
   computed during the same read, without an additional pass (decision G4).
7. The output is deterministic: the same CSV always produces byte-for-byte identical SSTables. The
   manifest is excluded because it includes the generation date (decision G5).

## Validations and messages

All the generator validations of the parent document apply. This delivery adds:

| Situation | Rule | Message to the user (verbatim Spanish UI text / English meaning) |
|---|---|---|
| The CSV does not exist or cannot be read | Aborts before processing any row | `No se puede leer el archivo <ruta>.`<br>*The file `<path>` cannot be read.* |
| The temporary folder does not exist or is not writable | Aborts before processing any row | `No se puede usar la carpeta temporal <carpeta>.`<br>*The temporary folder `<folder>` cannot be used.* |
| The log cannot be created | Aborts before processing any row, because without a log the rejections cannot be recorded | `No se puede escribir el log en <ruta>.`<br>*The log cannot be written to `<path>`.* |
| The CSV only has the header | An empty dataset is published, consistent with the parent's decision D5, and a warning is given | `Aviso: el CSV no contiene registros; el conjunto generado está vacío.`<br>*Warning: the CSV contains no records; the generated dataset is empty.* |
| Cancellation by the operator | Treated as a failed generation | `Generación cancelada por el operador. No se ha modificado el conjunto existente.`<br>*Generation cancelled by the operator. The existing dataset has not been modified.* |
| Leftovers from a previous interrupted generation | Deleted or ignored without affecting the result | `Se han descartado restos de una generación interrumpida anterior.`<br>*Leftovers from a previous interrupted generation have been discarded.* |

## Verification without the query app

As long as the query app does not exist, the parent's criteria that say "is queried" are checked by
reading the generated tables directly with an independent reader of the LevelDB table format, without
using any code from the generator itself. The criteria that only make sense with the query app (AC6, AC7,
AC10 and AC11) are left for its change.

## Reuse

- **Parent document**: flow A, field rules, generator validations, memory constraint and
  decisions D1–D5.
- **Measurements of the reference CSV** from the parent: they are the expected figures in the
  verification.

## Out of scope

- The query app and the parent's criteria AC6, AC7, AC10 and AC11: they go in the next change.
- An inspection or query mode inside the generator.
- Choosing which indexes are generated: the set of indexes is the one fixed by the parent's decision D2.
- Regenerating a single index without regenerating the complete dataset.
- Generating from several CSVs at once.
- Running or verifying on systems other than Windows: the reference machine is Windows 11.
- Installers or distribution of the executable.

## Acceptance criteria

The exact figures correspond to the reference CSV, with the same caveat as in the parent document.

1. **AC-G1** (parent AC1) — Given the full CSV, when the generator runs, then the primary SSTable
   contains 8,656,904 entries in strictly ascending ID order, there are 0 rejected, and the manifest and
   the summary match those figures.
2. **AC-G2** (parent AC2) — Given the full CSV, when the generator runs, then the peak private committed
   memory does not exceed 512 MiB and the final summary shows it.
3. **AC-G3** (parent AC3 and AC12) — Given the ID sample from the parent's AC3 plus a record with a time in
   the spring-forward gap, when the primary table is read directly, then every field is equivalent in value
   to the one in the CSV row and the date of that record is exactly the published one.
4. **AC-G4** (parent AC4, AC5 and AC8) — Given the generated dataset, when the indexes are read directly,
   then:
   - each index has as many entries as there are valid records;
   - within each value, the IDs appear in ascending order;
   - in the primary type index, the value `THEFT` has 1,838,502 entries;
   - in the community area index, the value "no value" (*sin valor*) has 613,720 entries;
   - in the case number index, each shared number lists all the IDs that share it.
5. **AC-G5** (parent AC9 and AC13) — Given the test CSVs of the parent's AC9 and AC13, when the
   generator runs, then the result is the one those criteria describe.
6. **AC-G6** — Given an already published dataset, when the process is abruptly interrupted in the middle of
   the generation, then the previous dataset remains intact, no incomplete dataset is visible and the
   next generation finishes normally.
7. **AC-G7** — Given an already published dataset, when the operator cancels with Ctrl+C, then no
   temporary files remain, the previous dataset remains intact and the exit code indicates failure.
8. **AC-G8** — Given an output folder with a dataset, when the generator runs without the overwrite option,
   then the run is refused without modifying anything; when it runs with that option, then the dataset is
   replaced.
9. **AC-G9** — Given an aborted generation, when the operator opens the log, then the log exists and
   explains the cause.
10. **AC-G10** — Given the same CSV, when it is generated twice, then the resulting SSTables
    are byte-for-byte identical.

## Decisions

No decision remains open. All of them were made on 2026-10-12 and are already reflected in the body of
the PRD.

- **G1 — Log**: one file per run, outside the dataset folder. Its location is configurable and, by
  default, it is placed next to the output folder. It is also shown on the console. Reason: a log inside
  the dataset would be lost in a failed generation, which is precisely when it is needed most.
- **G2 — Compression**: Snappy enabled by default, with an option to disable it. Reason: the text values
  and the categories repeat a lot, so the size is expected to shrink considerably. Disabling it is useful
  for measuring and comparing.
- **G3 — Exit codes**: 0 when the generation finishes, even if there are rejections or a memory warning;
  other than 0 when it aborts or is cancelled. Reason: it allows automation without reading the log, and
  the rejections are already recorded in the summary and in the manifest.
- **G4 — Source in the manifest**: name, size and a fingerprint (hash) of the content computed during the
  read. Reason: it makes it possible to know with certainty which extract a dataset came from, without an
  additional pass.
- **G5 — Deterministic output**: the same CSV produces byte-for-byte identical SSTables; the manifest is
  excluded. Reason: it makes it easier to verify and compare generations, and it detects unintended
  changes.

## References

- Parent document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md)
