# Robustness of the CanopyDB applications and module closing — PRD

**Version**: 1.0
**Date**: 2026-10-22
**Parent document**: [Chicago crime store on CanopyDB — PRD](canopy-db.prd.en.md) (v1.0)

> English translation of [canopy-db-robustez.prd.md](canopy-db-robustez.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Hunt for bugs in CanopyDB, and in the project applications that use it, in the situations that a normal run
does not touch: processes interrupted or killed halfway, several processes on the same dataset, queries while a
new version is being published, a volume larger than the reference one, and Linux. It is the fifth and last
child of the CanopyDB module (decision K5 of the parent). The module is closed by this child or by
`canopy-db-comparacion`, whichever closes last (decision KR5).

**What it delivers**:

1. A catalog of scenarios, each with what must happen (see [Scenarios](#scenarios)).
2. A repeatable way to run them on Windows and on Linux, to pass them again with each new version of CanopyDB
   (decision KR2), and a tool that builds the large CSV of R13 (decision KR3).
3. The scenario log, with the result of each scenario on each system (see [Scenario log](#scenario-log)), and
   an entry in the findings log for each CanopyDB bug that appears.
4. The issue draft of each finding, from H1 onwards, ready to publish (decision KR8).
5. Its part of the module closing and, if it closes last, the whole closing (see
   [Module closing](#module-closing)).

**Why**: objective 2 of the parent is to detect CanopyDB bugs under the demands of the contract. The previous
children check the normal path and a few interruptions; what remains untested is exactly where a young engine
usually fails: the lock of a process that dies, the deletion of files that another process has open, and the
races between publishing and reading.

This child does not change the options, messages or outputs of any application. It adds a rule to the CanopyDB
query app for a version that gets deleted (decision KR7) and, where the inherited contract does not say what
must happen in a scenario, the scenario row sets it (decision KR1).

### Starting state

Verified on 2026-10-22 against the repository and the closing reports of children 1 to 3:

| Aspect | Situation |
|---|---|
| Children of the module | `canopy-db-generador`, `canopy-db-lote` and `canopy-db-consultor` closed. `canopy-db-comparacion` has no PRD yet. |
| CanopyDB version | 0.2.5 published, no fork. |
| Cancellation of the CanopyDB generator | It honors Ctrl+C while reading the CSV, while loading the main table and the indexes, and right before the version switch. A Ctrl+C during the final dump of the database is honored when the dump finishes. A Ctrl+C after the version switch changes nothing: the generation has already finished successfully. |
| Abrupt interruption of the CanopyDB generator | The next generation discards the leftovers (KG6). The acceptance check kills the generator at a single moment, during the load into CanopyDB; no test kills it in the other phases. |
| Simultaneous generators | Two generators on the same folder, of any engine and in any order, are already checked on Windows and on Linux (AC-KG14). |
| CanopyDB query app killed or cancelled halfway | No test kills or cancels a query app or a batch halfway through a query and checks that the next open does not wait. |
| Wait on an open dataset (KC2) | Tested with a single query app waiting; never with several at once, nor with the one holding the dataset killed while another waits. |
| Version that disappears during the wait | The CanopyDB query app re-reads the pointer and opens the new version, but no PRD says so and it has not been tested: on Windows it cannot be provoked, because the generator cannot delete an open version. |
| Deleting an open version | On Windows the generator cannot and treats it as the parent sets (flow A, step 5). On Linux the deletion works even if a query app has the version open; what happens to that query app has not been tested nor does any PRD say. |
| Publishing during a batch | AC-M7 is checked on Windows with a held batch: the JSON of each query states the version that the batch opened. There are no tests with query apps in parallel during several consecutive publications. |
| Large data | No test exceeds the records of the reference CSV, and there is no tool to build a larger CSV. |
| Linux | The tests pass on Ubuntu 24.04 over WSL2. The acceptance checks of the previous children only run on Windows (out of scope in the parent). The Linux guides ask for the code and the data to be on the Linux file system. |
| Findings log | H1 to H5, all "Detectado", with no public action. |
| Root PRD | It does not mention the CanopyDB module yet. |

**Product gaps**: none open.

## Actors

- **Developer**: runs the scenarios, records their results, fixes the project bugs, registers the CanopyDB ones
  and writes their issues.
- **Project owner** (the user): the one from the parent; also declares the module closed.
- **Analyst**: only notices KR7: a CanopyDB query app whose version gets deleted answers instead of failing. The
  operator sees no change.

## Flow

1. The developer runs the scenarios on Windows 11 and on Ubuntu 24.04 over WSL2. On Linux, the code and the
   data are on the Linux file system, as its guides ask, not on a mounted Windows drive.
2. The run reports, for each scenario and variant, its state and what it observed (see [Output](#output)).
3. The developer records each result in the scenario log.
4. A failure to comply is handled according to K10 of the parent: if it belongs to the project, it is fixed and
   the scenario is repeated until it passes; if it belongs to CanopyDB, it goes into the findings log with its
   minimal reproduction and the draft of its issue.
5. If a fix changes the code of an application, the acceptance check of that application is passed again before
   closing.
6. When all the scenarios are recorded, the developer does their part of the module closing.

## Scenarios

### Common rules

- Scenarios R1 to R12 use the dataset generated from the reference CSV; R13, the large CSV.
- **Kill** means ending the application process without giving it a chance to clean up: `TerminateProcess` on
  Windows, `SIGKILL` on Linux. **Ctrl+C** is the cancellation signal that the application honors.
- A signal counts only if it arrives in the phase that the variant names; if the application had already passed
  that phase or had finished, that run does not count and is repeated, up to five attempts. If none succeeds,
  the variant is left as not run.
- "The next" process is launched when the previous one has already finished, and must open the dataset without
  the wait notice of KC2.
- "A clean run" is the same query run alone, with no interruptions or other processes; "the same result" is the
  same number of results and, in listings, the same set of records.
- "The version" of a query is the one that its JSON output states, as in AC-M7.
- Processes that are not killed end with the code that their row sets. At the end of R1 to R12, the logical
  content of the published dataset is that of a clean generation (checked as in AC-M3).
- Scenarios with races (R8, R9, R11 and R12) are repeated at least five times per variant. A variant fails if a
  single repetition fails, and the log notes how many failed.

### Generation

| Id | Scenario and variants | Expected |
|---|---|---|
| R1 | Kill the CanopyDB generator in each phase: reading and sorting the CSV, loading the main table, writing the indexes, final dump of the database, writing the manifest, between writing the temporary pointer and the version switch, and after the version switch, before deleting the previous version. Each phase with a previous dataset published and without one. | Before the version switch: the previous dataset stays published and a query gives the previous version; without a previous dataset, the CanopyDB query app rejects the folder with `No se encontró un conjunto de datos válido en <carpeta>.` and code 1. After the version switch: a query gives the new version. In all cases, the next generation (with `--overwrite` if there is a dataset) does not report another generation in progress, shows the KG6 messages for what was left (version leftovers, temporary folders or both, and none for what was not left), ends with 0 and publishes the content of a clean generation. |
| R2 | Ctrl+C to the CanopyDB generator in each phase: reading and sorting the CSV, loading the main table, writing the indexes, final dump of the database, and after the version switch. Each phase with a previous dataset published. | Up to and including the dump: it ends with 130 and the cancellation message, with no leftovers, and a query gives the previous version; in the dump, it ends when the dump finishes. After the version switch: it ends with 0, with its final summary and no cancellation message, and a query gives the new version. The log notes how long it took to end after the Ctrl+C. |

### CanopyDB query app

| Id | Scenario and variants | Expected |
|---|---|---|
| R3 | Kill a CanopyDB query app: during the database open; during the F3 count; during the F11 listing to standard output; during the F11 listing with an output file, with and without a previous file with that name. | The next query app gives the same result as a clean run. The dataset folder meets AC-M6. With an output file, the file with the requested name is the previous one, intact, or does not exist if there was none: never a half-written one (C8). A temporary file left next to it is noted and is not a failure, because a killed process cannot delete it. |
| R4 | Kill a batch with the fixed set in which F10 and F11 write to output files: after F10 finishes and during F11. | The same as R3; in addition, the F10 file is complete and is that of a clean run. |
| R5 | Ctrl+C halfway through F11, outside the wait: to a standalone query app and to a batch. | It ends with `Consulta cancelada.` and 130 in under 1 s, with no half-written output file or temporary files; the next query app opens without the wait notice. |
| R6 | A batch has the dataset open and ends between 10 and 20 s after a second query app starts waiting; the second one is cancelled with Ctrl+C or killed. Then a third arrives. | The cancelled one ends as KC2 sets (`Consulta cancelada.`, 130, under 1 s). The batch ends with 0 and the same result as a clean run. The third shows a single notice and, when the batch ends, answers with 0 and the result of a clean run. |
| R7 | A batch has the dataset open and a second query app waits; the batch is killed. | The second one opens in under 1 s from the death of the batch, without a second notice, and answers with 0 and the result of a clean run. |

### Concurrent processes and publication

| Id | Scenario and variants | Expected |
|---|---|---|
| R8 | Four query apps with the F3 count, launched at once, wait for a batch to release the dataset. Variant A: the batch ends between 5 and 15 s after the last notice. Variant B: the batch lasts at least 45 s more. | A: each one shows a single notice and answers with 0 and the result of a clean run; a rejection is a failure, because the four queries fit in the wait. B: each one shows a single notice and is rejected with the KC2 message and code 1, between 30 and 35 s after its notice. In both, the log notes the order in which they ended; no order is guaranteed (KR4). |
| R9 | Four query apps in parallel, each repeating F3 and F2 without pause, while the CanopyDB generator publishes ten consecutive versions of the reference CSV with `--overwrite`. Then, one more generation without query apps. | Each generation ends with 0 and publishes; on Windows, the one that cannot delete an open previous version treats it as the parent sets (flow A, step 5). Each query ends with 0 and the result of a clean run, with the version it opened, also if the version it was going to open is deleted beforehand (KR7); or, only if another query app had the dataset open all that time, it is rejected with the KC2 message and code 1 between 30 and 35 s after its notice (KR4). After the final generation no previous version remains. |
| R10 | Kill the CanopyDB generator during the load of the main table while a batch with the fixed set reads the previous dataset. | The batch ends with 0, with the result of a clean run and the previous version; then, what R1 expects for that phase with a previous dataset. |
| R11 | Linux only: the generator publishes and deletes the previous version while a standalone query app with F11 has it open, and in another variant a batch with the fixed set. | The query app or the batch ends with 0, with the result of a clean run, and all its queries give the version it opened (KR7). When it ends, the deleted version leaves nothing in the dataset folder. |
| R12 | Linux only: a batch has a version open and a query app with F2 waits; the generator publishes another one and deletes the one that was waited on before the batch ends. | The batch, as R11. The one that waits shows a single notice and, as soon as the version it was waiting on disappears, opens the new one without waiting for the batch and answers with 0, the result of a clean run and the new version, within 30 s of its notice (KR7). |

### Large data

| Id | Scenario and variants | Expected |
|---|---|---|
| R13 | Windows only: generate the large CSV with the CanopyDB generator and run, with the CanopyDB query app, the fixed set and the three runs of AC-M8. | The generator ends with 0, with `<registros del CSV grande> registros válidos, 0 rechazados`, and its memory peak, measured as in its acceptance check, does not exceed 512 MiB (or K10). Each query of the fixed set returns the same number of results as the direct filtering of the large CSV; in F10, 1,000. The three runs of AC-M8 meet their ratio. |

**Large CSV** (KR3): the tool builds it from the reference CSV, with its header only once, the original and then
two copies of all its records. In each copy, each record gets a new ID, numeric and greater than any ID of the
original, and a new case number with the same two initial letters and the same format, that appear neither in
the original nor in the other copy; the rest of the fields do not change. So F1 and F2 return the same as in the
reference CSV, F9 triple, and the other queries triple. Before starting, the scenario calculates the free space
it needs on the disk of the work folder; if there is not enough, it does not start, says how much it needs and
is left as not run.

### Systems

All the scenarios run on both systems, except R11 and R12, which can only be provoked on Linux, and R13, which
only runs on Windows (KR3). On the other system, those scenarios come out as not applicable.

## Scenario log

A repository document, in Spanish, next to the findings log. Each time the scenarios are passed, new rows are
added; the previous ones are not changed, to compare CanopyDB versions. One row per scenario, variant and
system, with the variants that the scenario row names.

| Field | Content |
|---|---|
| Scenario | `R1`… `R13` and its variant. |
| System | `Windows` or `Linux`. |
| Date | Date of the run, `AAAA-MM-DD`. |
| CanopyDB version | The version and, if used, the fork commit. |
| Project commit | The commit it was run with. |
| Result | `Sin bug` (met state); `Bug del proyecto` with the commit that fixes it (state not met by the project; the row of the later repetition says `Sin bug`); `Hallazgo` with the identifiers `Hn` (state `NOT MET`, or not met due to a new finding); or `No aplica en este sistema`. A variant that was not run is not recorded: it is repeated until it runs. |
| Observed | Exit codes, messages, times, order and repetitions; if it failed, how it departed from what was expected; if a scenario associated with a finding was met, that the finding may be resolved. |

## Module closing

The module is closed by the last of its children to close, this one or `canopy-db-comparacion` (KR5). This
child always does steps 1, 2 and 5; steps 3 and 4 are done by the child that closes last, which also verifies
AC-M12 and AC-M13 again on its closing.

1. AC-M11, AC-M12 and AC-M13 of the parent are verified.
2. Annotations of this child, in each PRD in Spanish and in its translation:
   - Parent, in "Split into child PRDs" (the execution order and the module closing) and in its row of
     "Criteria and annotations per child": KR5.
   - Parent, in "Findings log": the new field `Borrador del issue` (KR8).
   - PRD of `canopy-db-consultor`: in its flow, the rule of KR7; in "Out of scope", that the scenarios it left
     for this child are in this document.
3. The closing report of the child that closes the module includes a table with one row per criterion, from
   AC-M1 to AC-M13: the children that verify it, according to the "Criteria and annotations per child" table of
   the parent, and the result of each one (`Cumplido`, or `No cumplido por CanopyDB` with its finding). The
   module is not closed if a criterion lacks the result of any of its children.
4. Annotations of the module closing, in the PRD in Spanish and in its translation:
   - Root PRD: *Anotación del <fecha>: el [módulo CanopyDB](canopy-db.prd.md) añade un generador y un consultor
     que guardan el conjunto en CanopyDB, con el mismo contrato.*
   - Parent: the module is closed, with the date.
5. The README and the Linux guides say how to run the scenarios, how to build the large CSV and where the
   scenario log is.

## Validations and messages

This child adds no validations or messages to any application; all keep those of their ancestry. KR7 adds no
messages: the query app answers instead of rejecting.

## Output

- **Scenario log**: see [Scenario log](#scenario-log).
- **Findings log**: a new entry for each CanopyDB bug, limitation or documentation problem that appears, with the
  shape of the parent and the new field `Borrador del issue`, which links the draft (KR8).
- **Run of the scenarios**: for each scenario and variant, its identifier, its state and what was observed
  (KR6). The states are:
  - met;
  - not met;
  - `NOT MET` with its finding: the variant is associated with a known finding and the only thing not met is
    what that finding describes; any other difference leaves it as not met;
  - not run, with the reason (the signal did not arrive in its phase after five attempts, or there is not
    enough disk space);
  - not applicable on this system.

  An associated variant that is met comes out as met, with a notice that its finding may be resolved. An
  association that names a scenario, a variant or a finding that do not exist makes the run not start and end
  with code 1. The exit code is 1 if any variant is left not met or not run, and 0 otherwise. The exact format
  is left to the implementation.
- **Large CSV tool**: writes the large CSV and says how many records it has.
- The applications do not change their output.

## Reuse

- The contract of the ancestry and of children 1 to 3: messages, codes, cancellation, leftovers (KG6), two
  simultaneous generators (AC-KG14), wait (KC2), engine errors (KG7 and KC5) and output file (C8).
- The fixed set of queries (KC7) in R3 to R13.
- The same-logical-content check of the CanopyDB verifier (KG8).
- The direct filtering of the CSV and the memory peak measurement of the acceptance checks, for R13.
- The association of criteria with findings of KG10 and KC8 (`-NotMetByCanopyDb`), for KR6.
- The minimal reproductions of the findings log as a model for the new ones.

## Out of scope

- Changing the options, messages or outputs of any application, including the wait time of KC2.
- Scenarios on the original applications, including killing or cancelling the batch of the original query app.
- Simultaneous generators on the same folder or with the same temporary folder: already covered by AC-KG14 and
  the tests of the previous children.
- The CanopyDB verifier in any scenario.
- Other signals and ways of ending: `SIGTERM`, `SIGHUP`, Ctrl+Break and closing the console.
- Standard output closed halfway through a listing, lack of memory or file descriptors, paths with spaces or
  non-ASCII characters, and third-party programs (antivirus, indexers) that open the dataset files.
- Power cuts and durability after a machine shutdown, as in the parent.
- Real full disk, network folders, Windows drives mounted on Linux, read-only media and machines with another
  byte order.
- Databases deliberately damaged or written with another version of CanopyDB (KC4).
- Developing in CanopyDB a read-only mode or access by several processes (H2 and H3).
- Measuring or comparing performance: it belongs to `canopy-db-comparacion`.
- R13 on Linux and a large CSV of another size (KR3).
- Running the acceptance checks of the previous children on Linux.
- macOS and other systems.
- From flow E of the parent, everything except registering in the log and the issue drafts (KR8): publishing
  issues, preparing or publishing a PR, commenting, creating the fork or depending on it.

## Left to the implementation

- How the scenarios are run (automated tests, scripts or a dedicated program), whether any of them is part of
  `cargo test`, how the KR6 associations are given to it and the format of its output, as long as they meet KR2
  and [Output](#output).
- How a signal is made to arrive in each phase, as long as it adds no visible option to the applications nor
  changes a normal run.
- The names of the variants, of the scenario log and of the large CSV tool, and where the issue drafts live.
- How R13 calculates the space it needs.

## Acceptance criteria

The figures correspond to the reference CSV, with the same caveat as in the parent. An inherited criterion that
CanopyDB cannot meet is handled according to K10.

1. **AC-KR1** — Given each scenario from R1 to R13 and each variant, when it is run on each system that applies
   to it, then what its row expects is met, or it is left as `NOT MET` due to a CanopyDB finding.
2. **AC-KR2** (KR2) — Given the repository on each system, when the scenarios are run, then each variant
   reports its identifier, its state and what was observed, with no manual steps; R11 and R12 come out as not
   applicable on Windows, and R13 on Linux.
3. **AC-KR3** (KR6) — Given an association of a variant with a finding, when the variant fails to meet only what
   the finding describes, then it comes out as `NOT MET` and the code is 0 if everything else is met; when it
   additionally fails to meet something else, it comes out as not met and the code is 1; when it is met, it
   comes out as met with the notice. Given an association with a nonexistent scenario, variant or finding, the
   run does not start and the code is 1. Given a variant that was not run, the code is 1.
4. **AC-KR4** (KR7) — Given a standalone or batch CanopyDB query app that already opened a version, when the
   generator deletes it, then all its queries give that version and the result of a clean run (R11). Given one
   that had not yet opened it, when the version disappears, then it opens the new one with a single notice and
   within the 30 s of KC2 (R9 and R12).
5. **AC-KR5** (KR3) — Given the reference CSV, when the tool builds the large CSV, then it has one header,
   triple the records, no repeated ID or case number except those that were already repeated in the original,
   and the CanopyDB generator accepts it with no rejections.
6. **AC-KR6** — Given the scenario log at closing, when it is reviewed, then it has one row with all its fields
   for each scenario, variant and system, with a result other than not run; and each finding it names exists in
   the findings log with its minimal reproduction.
7. **AC-KR7** (KR8) — Given the findings log at closing, when it is reviewed, then each finding links in
   `Borrador del issue` a draft in English with title, description, minimal reproduction and version; and there
   is no new public action in the CanopyDB repository or in a fork.
8. **AC-KR8** (KR5) — Given the closing of this child, when it is reviewed, then steps 1, 2 and 5 of
   [Module closing](#module-closing) are met; and, if `canopy-db-comparacion` is already closed, steps 3 and 4
   as well.

AC-M11, AC-M12 and AC-M13 are those of the parent.

## Decisions

KR1 and KR2 derive from the ancestry; KR3 to KR8 were taken with the user, who also confirmed the common rules
of the scenarios and R7. All are from 2026-10-22.

- **KR1 — What each scenario expects**: it comes from the inherited contract. Where the contract does not say
  what happens, the scenario row sets it: the code and the summary after a Ctrl+C after the version switch
  (R2), a temporary file left by a killed query app (R3), cancellation in under 1 s also halfway through a
  query (R5) and opening in under 1 s after killing the one that held the dataset (R7). A failure to meet what
  is expected is a project bug or a CanopyDB finding.
- **KR2 — Repeatable scenarios**: they run on each system with no manual steps, to pass them again with a new
  version of CanopyDB or after fixing a finding. Reason: the parent asks to help CanopyDB mature, and a
  scenario that was run only once says nothing about the next version.
- **KR3 — Large data**: triple the reference CSV (about 26 million records), built by a tool of this child, and
  on Windows only. Reason: it is enough to see whether the memory of the generator and of the query app grows
  with the volume, with about 50 GB free; on Linux, the generator's peak was already measured by hand with the
  reference CSV (AC-M4). Discarded: double (less margin to see memory grow), quintuple (about 80 GB and much
  more time) and repeating it on Linux.
- **KR4 — Several waiting query apps**: there is no entry order or fair share. A query app that is rejected at
  30 s meets KC2 if in that time another process had the dataset open, even if it was momentarily free. Reason:
  KC2 promises a wait, not a turn, and a turn would be a new mechanism. Discarded: treating as a bug the query
  app that loses against others.
- **KR5 — Module closing**: it is closed by the last child to close, this one or `canopy-db-comparacion`; that
  child verifies AC-M12 and AC-M13 again, makes the AC-M1 to AC-M13 table and the closing annotations. It
  replaces, in the parent, the execution order between children 4 and 5, the sentence "The module is closed
  with the closing of 5" and the root PRD annotation in the row of this child of "Criteria and annotations per
  child"; the parent is annotated. Reason: AC-M2 with the benchmark and AC-M10 are verified only by
  `canopy-db-comparacion`, which has no PRD yet, and this child does not depend on it. Discarded: waiting for
  `canopy-db-comparacion` to close this child and closing the module with those two criteria pending.
- **KR6 — Known findings in the run of the scenarios**: it is told which variants are associated with which
  finding, with the `-NotMetByCanopyDb` pattern of KG10 and KC8: an associated variant comes out as `NOT MET`
  only if what is not met is what its finding describes, and it does not make the run fail. It extends that
  pattern with the notice of a finding that may be resolved. Reason: a run that cannot be green while CanopyDB
  does not fix a finding stops warning about new failures. Discarded: any failure giving code 1.
- **KR7 — Query app whose version gets deleted**: a CanopyDB query app that already opened a version ends with
  the results of that version even if the generator deletes it, as AC-M7 sets for a newly published version.
  One that has not yet opened it, whether waiting or not, re-reads the pointer as soon as the version
  disappears and answers with the new one, without a second notice and without exceeding the 30 s of KC2 from
  its notice. It holds on both systems. If the pointer has not changed and the version is incomplete, the
  rejection of AC-KC4 still holds. It is a new rule of the CanopyDB query app that does not change its options,
  messages or outputs; its PRD is annotated. Reason: on Linux the deletion of an open version works, and
  without a rule any rejection would pass as correct. Discarded: accepting a rejection with an existing
  message and code 1.
- **KR8 — Contribution to CanopyDB**: this child leaves the issue of each finding drafted in English (title,
  description, minimal reproduction and version), but publishes nothing nor prepares a PR. Each entry of the
  findings log links its draft in a new field, `Borrador del issue`, which extends the shape of the parent.
  Reason: publishing is an external action that the owner approves one by one (K4), and with the drafts ready
  that approval is immediate. Discarded: publishing with approval within this child and limiting itself to
  registering in the log.

## References

- Parent document: [canopy-db.prd.md](canopy-db.prd.en.md); root: [chicago-crime.prd.md](chicago-crime.prd.en.md)
- Siblings: [canopy-db-generador.prd.md](canopy-db-generador.prd.en.md),
  [canopy-db-lote.prd.md](canopy-db-lote.prd.en.md), [canopy-db-consultor.prd.md](canopy-db-consultor.prd.en.md)
- [Generator PRD](generador.prd.en.md), [query app PRD](consultor.prd.md),
  [Linux support PRD](soporte-linux.prd.md),
  [acceptance with any CSV PRD](aceptacion-cualquier-csv.prd.en.md)
- [Findings log](../../canopy-db/hallazgos.md)

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 6174e156 | 2026-10-22 10:32 UTC+01:00 | 1 h 3 min | 14 min | 49 min | 25 min | 24 min | 7 |

<!-- prd-time: [{"session": "6174e156-e93a-4ad3-88e8-cda74c462cb4", "start": "2026-10-22T09:32:36.433000+00:00", "wall": 3774, "discarded": 858, "agent": 1489, "human": 1425, "interventions": 7, "idle_minutes": 10}] -->
