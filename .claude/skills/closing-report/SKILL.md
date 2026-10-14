---
name: closing-report
description: "Trigger: close a feature, closing report, informe de cierre, finish ODD/SDD/vibe-coding run, merge done. Write the Markdown closing report of a change and save it in the repository."
license: MIT
metadata:
  author: inercya
  version: "1.0"
---

## Activation Contract

Load when an implementation run ends (ODD, SDD or vibe coding), after the last verification and before
telling the user the work is done. The report is written to the repository, not only to the chat. It is the
record a reader opens to learn what changed, how long it took, what was verified and what is still pending.

## Hard Rules

- Facts only. Every figure comes from a command, a test run, a script report, `git log` or `date`; never
  from memory or an estimate. Mark anything not measured as not measured.
- Time: compare the real duration with the estimate given at the start. Waits for the user are reported
  apart from active time. Use only clock times (`date`, commit times), never guessed ranges.
- Change report: generate it with `scripts/numstat_by_area.py`; do not add lines by hand.
- Report failed, skipped and pending checks honestly; never present a partial result as complete.
- Language: match the project's PRDs; translate the template headings consistently.
- The report never authorizes delivery. Push, PR and merge remain the user's decisions.

## Location

| Approach | Report path |
|---|---|
| ODD | Next to the feature document: `odd/tasks/<feature>-report.md` |
| SDD | Next to the change's artifacts (the change folder) as `report.md` |
| Vibe coding or no execution document | `docs/reports/<YYYY-MM-DD>-<change-name>.md` |

Link the report from the execution document's closing section and the execution document from the report.

## Execution Steps

1. Collect the evidence: final test and check results, acceptance results, review outcomes, commits.
2. Measure time: start time recorded at the beginning, end time from `date`; compute the active duration and
   list the waits apart.
3. Generate the change table from the branch point (or the commit before the first change):
   `python -I .claude/skills/closing-report/scripts/numstat_by_area.py <base> HEAD --lang es`.
4. Fill `assets/closing-report-template.md`, section by section; delete a section only when it truly does
   not apply, and say why in one line.
5. Save it at the path from the Location table, link it as described, and commit it with the change when the
   change is being committed (`docs: add the <feature> closing report`).

## Output Contract

Return the report path, the real-versus-estimated time line and the list of pending items.

## References

- `assets/closing-report-template.md` — report skeleton with per-section guidance.
- `scripts/numstat_by_area.py` — lines added and removed by area, with totals; lockfiles and binaries apart.
- Example: `odd/tasks/crime-query-report.md`.
