# Acceptance checks with any compatible CSV — PRD

**Version**: 1.0
**Date**: 2026-10-16
**Parent document**: [Queryable Chicago crime store on SSTables — PRD](chicago-crime.prd.en.md) (v1.2)

> English translation of [aceptacion-cualquier-csv.prd.md](aceptacion-cualquier-csv.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Make the acceptance checks of the generator and of the query app work with any compatible download of the
Chicago CSV, not only with the reference extract. With the reference extract, the exact figures of the PRDs
are still checked. With another download, every expected figure comes from the CSV itself.

**Why**: the portal updates the CSV daily, and the README explains how to download it. A new download has more
records, so today the acceptance checks fail even when the generator and the query app work correctly. The
PRDs already cover this case: "Con otro CSV compatible, esos resultados deben coincidir con los registros
válidos realmente procesados" (with another compatible CSV, those results must match the valid records
actually processed; parent acceptance criteria, repeated in those of the generator and the query app). The
acceptance checks do not follow that rule.

In passing, the README examples use folders on `D:\`, which does not exist on every machine. They move to
`C:\`.

This document changes no rule of the parent or of the [generator](generador.prd.en.md) and
[query app](consultor.prd.md) PRDs. The generator and the query app do not change.

### Starting state

Verified on 2026-10-16 on the repository at commit `407d43d`:

| Aspect | Situation |
|---|---|
| Rule of the PRDs | The exact figures belong to the reference extract; with another compatible CSV they must match the records actually processed. |
| Identity of the reference extract | The README publishes its date (2026-10-10), records (8,656,904), size (2,406,401,982 bytes) and SHA-256 fingerprint. The generator computes the CSV fingerprint while reading it and stores it in the manifest. The acceptance checks do not read it. |
| Generator acceptance | Requires the summary `8.656.904 registros válidos, 0 rechazados` (AC-G1). Builds the AC-G5 test CSVs from the first row of the reference extract, with its ID and date written in the script: if the CSV starts with another row, the invalid-date row is rejected as a repeated ID and AC-G5 fails. |
| Query app acceptance | Requires `THEFT` = 1,838,502 (AC-C2 and AC-C17; AC-C11 compares the theft list of the direct filtering with that figure) and community area "no value" = 613,720 (AC-C3). Every other count is already compared with the direct filtering of the CSV. |
| Independent verifier | Both acceptance checks always run it asking for the reference figures: 8,656,904 records, 0 rejected, `THEFT`, "no value", 632 repeated case numbers and 253 records in the spring-forward gap. Without that request it compares everything with the CSV. |
| Skipped checks | Without the reference figures, the verifier skips a check when the CSV lacks the data it needs (for example, no record in the spring-forward gap). Both acceptance checks count any skip as a failure. |
| Chosen historical data | Some queries use data chosen on the reference extract: August 2019, the 2019 spring-forward gap, the rare type `RITUALISM`, a missing ID (123) and a missing case number. They are from 2019 or earlier, so a newer download almost certainly keeps them. |
| Timed tests | The interruption and cancellation of AC-G6 and AC-G7 do not depend on the CSV size: a larger CSV only gives them more margin. |
| Disk space | The acceptance checks require 6 GB and 5 GB free, fixed, unrelated to the CSV size. |
| Reports | They show the CSV path and size, but not its fingerprint, and call it "reference CSV" even when it is another one. |
| README | The examples for generating, querying, acceptance and the independent verifier use folders on `D:\`, as do the help examples of both scripts and the worktree guide, which quotes the README's acceptance folder. It says the acceptance checks verify the criteria "on the reference CSV". |

## Actors

- **Developer**: downloads the CSV from the portal, or uses the reference extract, and runs the generator or
  the query app acceptance check.

## Flow

1. The developer runs the generator or the query app acceptance check, as today, with the reference extract
   or with another download.
2. After generating the dataset, the acceptance check compares the CSV fingerprint stored in the manifest with
   the reference extract's fingerprint published in the README (decision A1).
3. **If they match**, the acceptance check runs the same checks as today, including the exact figures and the
   independent verifier with the reference figures.
4. **If they do not match**, every expected figure comes from the CSV itself (decision A2):
   - valid records: the data rows of the CSV, with 0 rejected;
   - `THEFT` and community area "no value": the direct filtering of the CSV;
   - the independent verifier runs without asking for the reference figures.
5. The AC-G5 test CSVs are built from the first row of the CSV received, whatever it is.
6. Every check must pass, as today: a check skipped for lack of data counts as a failure (decision A3).
7. The report header says whether the CSV is the reference extract and shows its fingerprint.

## Validations and messages

The acceptance reports are written in English, as before.

| Situation | Rule | Message in the report header |
|---|---|---|
| The CSV is the reference extract | The exact figures of the PRDs are required | `CSV: the reference extract (SHA-256 <fingerprint>); the exact figures of the PRDs are required.` |
| The CSV is another one | The expected figures come from the CSV itself | `CSV: not the reference extract (SHA-256 <fingerprint>); the expected figures are counted on this CSV.` |

## Documentation

- **README**, acceptance section: says that both acceptance checks work with the reference extract and with
  any later download, explains the two modes and how the reference extract is recognized (its fingerprint,
  which the README already publishes).
- **Example folders**: every example with folders on `D:\` moves to `C:\` (decision A4), in the README, in the
  help examples of both scripts and in the worktree guide.

## Reuse

- The CSV's SHA-256 fingerprint that the generator already stores in the manifest.
- The identity of the reference extract published in the README.
- The independent verifier, which already compares everything with the CSV when it is not asked for the
  reference figures.
- The direct filtering of the CSV that the query app acceptance already uses for the other counts.

## Out of scope

- Changes to the generator, the query app or the rules of the independent verifier.
- Updating the figures of the PRDs or the README to a new download: they remain those of the reference
  extract.
- CSVs with rejected rows or repeated IDs: with another CSV, 0 rejected is still required, so the acceptance
  check does not pass.
- Specific messages for a CSV that lacks the chosen historical data (August 2019, the spring-forward gap): the
  acceptance check does not pass, as today.
- Adjusting the required disk space to the CSV size.
- Downloading the CSV automatically.
- Running the acceptance checks on Linux.
- Changing the `D:\` folders that appear in the PRDs of earlier changes: they are history.

## Acceptance criteria

1. **AC-A1** — Given the reference extract, when both acceptance checks run, then they pass, check the same
   exact figures as before the change, including the independent verifier with the reference figures, and the
   report header says it is the reference extract and shows its fingerprint.
2. **AC-A2** — Given a download from the portal later than the reference extract, when both acceptance checks
   run, then they pass, the report header says it is not the reference extract and shows its fingerprint, and
   the valid records, `THEFT` and community area "no value" match the direct filtering of that download.
3. **AC-A3** — Given a CSV whose first row is not that of the reference extract, when the generator acceptance
   check runs, then AC-G5 passes.
4. **AC-A4** — Given the README, when the acceptance section is read, then it explains the two modes and how
   the reference extract is recognized; and no example in the README, in the help of both scripts or in the
   worktree guide uses a folder on `D:\`.

## Decisions

- **A1 — How the reference extract is recognized**: by the SHA-256 fingerprint the generator stores in the
  manifest, compared with the one the README publishes. Reason: it costs no extra read and nobody has to
  remember an option. A script option to choose the mode was discarded because it would allow checking the
  reference extract without its exact figures.
- **A2 — Expected figures with another CSV**: they come from the CSV itself, through the independent verifier
  without the reference figures and the direct filtering the query app acceptance already uses. Reason: it is
  the caveat the PRDs already set.
- **A3 — Skipped checks**: with another CSV, a check skipped for lack of data still counts as a failure.
  Reason: acceptance is only worth something if it proves every criterion.
- **A4 — Example folders**: `C:\` instead of `D:\`. Reason: `C:\` exists on every Windows machine; `D:\` does
  not.
- **A5 — CSV to test AC-A2 and AC-A3** (2026-10-16): a real download from the portal, made while verifying this
  change and not stored in the repository. Reason: it is the real case that motivates the change. A trimmed
  copy of the reference extract was discarded because it is synthetic. Accepted consequence: the download
  cannot be repeated, because the portal changes daily. If its first row matches that of the reference
  extract, AC-A3 is tested with a copy of the extract with another row first, generated on the spot.

## References

- Parent document: [chicago-crime.prd.en.md](chicago-crime.prd.en.md), acceptance criteria and their caveat.
- [Generator PRD](generador.prd.en.md) (AC-G1, AC-G4 and AC-G5) and [query app PRD](consultor.prd.md)
  (AC-C2, AC-C3, AC-C11, AC-C17 and AC-C18).
- README, section on getting the data: identity of the reference extract.

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| 89464d25 | 2026-10-16 06:34 UTC+01:00 | 28 min | 0 min | 28 min | 12 min | 16 min | 7 |

<!-- prd-time: [{"session": "89464d25-9c83-4b20-8557-c6611235a8df", "start": "2026-10-16T05:34:29.114000+00:00", "wall": 1650, "discarded": 0, "agent": 699, "human": 951, "interventions": 7, "idle_minutes": 10}] -->
