# chicago-crime — project instructions

Rust workspace that turns the Chicago crime CSV into SSTables (`crime-generator`) and queries them
(`crime-query`). See `README.md` for the layout and `docs/methodology/` for the working method.

## Workflow

- **PRD first.** Every change to product behavior starts with a PRD written with the `prd` skill, in
  `docs/prd/chicago-crime/`. The Spanish PRDs are authoritative; `*.prd.en.md` are translations. A PRD does
  not authorize implementation; wait for the user to ask for it.
- **Execution after the PRD.** ODD by default. Use vibe coding or SDD only when the user asks for it. Bug
  fixes, refactors and tooling with no product change need no PRD.

## Before starting any execution (ODD, SDD or vibe coding)

1. **Questions first.** Ask every open product question before the run starts. After that, do not stop to
   ask: take a reasonable default, record it in the execution document and list it in the closing report.
2. **Estimate.** Give a time range for the whole run before the first change and record it with the real
   start time from `date`. Past runs finished in about half the estimated time; calibrate with the earlier
   closing reports in `odd/tasks/*-report.md`.
3. **Always verify.** Never ask whether to run tests, verifiers or reviews: run them. Automated review
   consent is always granted.

## Vibe coding from a PRD

- Every acceptance criterion of the PRD ends up covered by a test.
- A product decision the PRD does not cover is never invented: record it and report it at the close.
- The closing report includes a table mapping each acceptance criterion to its test and its result.

## During the run

- Work on a feature branch with one local commit per task (Conventional Commits, no AI attribution).
- Write only measured times in logs: `date`, commit times or script reports, never estimates.
- If the run must wait for the user, log when the wait starts and ends; waits are not active time.
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`.

## Closing

- Write the closing report with the `closing-report` skill and save it in the repository next to the
  execution document (for ODD, `odd/tasks/<feature>-report.md`).
- Merge the feature branch into `main` after full verification. Do not push until the user asks.

## Repository rules

- Never commit `chicago-crime-data.csv` (2.3 GB reference dataset, git-ignored).
- Code, identifiers, comments and technical docs in English; CLI messages and PRDs in Spanish.
