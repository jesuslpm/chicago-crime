# <Outcome in one line, e.g. "Query app finished: verified and merged into main">

Closing report of `<feature or change>` (<YYYY-MM-DD>). Execution approach: <ODD | SDD | vibe coding>.
Product source: <link to the PRD>. Execution document: <link, or "none">.

<One or two lines with the delivery state: branch, merge, commit, whether anything was pushed.>

## Result

- **Specification:** <which specs or acceptance criteria were delivered; tests passing, with the count>.
- **Acceptance:** <acceptance run on real data, with its figures; or "not run" and why>.
- **Reviews and verification:** <independent verifiers, automated reviews and their outcome>.
- **Findings fixed along the way:** <defects found by tests, verifiers or reviews, and how they were fixed>.

## Acceptance criteria coverage

<Required for vibe coding; optional when the execution document already traces criteria to evidence.>

| Criterion | Test or check | Result |
|---|---|---|
| <AC id> | <test name or command> | <PASS / FAIL / not run, with the figure when there is one> |

## Reuse

<Required for vibe coding; optional otherwise. What the change reused, extended or extracted from existing
code, and what had to be written new, with file references. One line for any similar-looking code left
separate on purpose, and why.>

## Time

- Estimate: <range given before starting>. Real start: <date>. End: <date>.
- **Active duration: <h min>**, against the estimate: <deviation and its main cause>.
- Waits for the user: <none | each wait with start, end and reason>.

## Change report (`<base>..<head>`)

<Paste the table produced by `scripts/numstat_by_area.py`, then the lockfiles/generated line.>

<One line comparing with the forecast, if a line forecast existed.>

## Summary

<One paragraph: what the change does for the user, in product terms, and the key technical choices.>

## Defaults chosen without asking

<Decisions the agent took with a reasonable default instead of interrupting the user; or "none".>

## Pending (non-blocking)

<Known limits, advisory findings left open, follow-up work. Each item states why it is not blocking.>

**Risk:** <item numbers and reason, or "none">.

## Manual cleanup

<Temporary folders or files the user may delete, with sizes; or remove this section if there are none.>
