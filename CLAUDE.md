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

1. **Questions first.** Ask every open product question before the run starts. Once it starts, do not stop to
   ask: take a reasonable default, record it in the execution document and list it in the closing report.
2. **Estimate, then ask to start.** When no questions are left, send a message with only the time range for
   the whole run and the question whether to start, then stop and wait. Nothing else goes in that message (no
   plan, map or progress), so the estimate does not get lost in the agent's activity. After the user confirms,
   record the estimate and the real start time from `date`. Past runs finished in about half the estimated
   time; calibrate with the earlier closing reports in `odd/tasks/*-report.md`.
3. **Always verify.** Never ask whether to run tests, verifiers or reviews: run them. Automated review
   consent is always granted.

## Vibe coding from a PRD

When the user asks for vibe coding, follow the `vibe-coding` skill. It sets the bookends: a map of the
existing code, questions and estimate before the first change; DRY while working (search before writing,
refactor after each green step); a test per acceptance criterion, verification against the PRD and a closing
report with the criterion → test → result table and the reuse outcome at the end. No execution document,
phases or subagents.

## During the run

- Work on a feature branch with one local commit per task (Conventional Commits, no AI attribution).
- Write only measured times in logs: `date`, commit times or script reports, never estimates.
- If the run must wait for the user, log when the wait starts and ends; waits are not active time.
- Record each RDD review when it ends: commits, lenses, result, findings with severity and resolution, and
  lineage. Write it in the execution document's log (in vibe coding, the running list). Gentle AI keeps none
  of it after the review is acknowledged, and the closing report's RDD reviews table is built from this record.
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`.

## Closing

- **Review the last slice.** When RDD is on and commits remain after the last reviewed boundary (the last
  assessment said `under_budget`), request their review before the final checks and the closing report, and
  follow its transitions to the end, so every merged commit has been through RDD:

  ```text
  gentle-ai review status --cwd <repo> --contract gentle-ai.review-integration/v2 --agent claude-code \
    --next-transition --base-ref <last reviewed boundary> --committed-only \
    --lenses <lenses> --lenses-reason "<what changed and how>"
  ```

  Generated files (for example `docs/methodology/slides.html`) go in a commit of their own, after their source
  has been reviewed or assessed. A last slice made only of generated files needs no review: it is reviewed
  through its source.

- Write the closing report with the `closing-report` skill and save it in the repository next to the
  execution document (for ODD, `odd/tasks/<feature>-report.md`).
- Merge the feature branch into `main` after full verification (in a worktree copy, see *Working in a
  worktree*). Do not push until the user asks.

## Working in a worktree

A worktree copy holds one change at `C:\wt\chicago-crime-<n>` on branch `feat/<n>`; its `.git` is a file, while
in the main copy (`C:\rust\chicago-crime`) it is a folder. Create, list, merge and remove copies only with the
`worktree` skill. Guide: `docs/worktrees.md`.

- **Merge into the base, only when asked.** In a copy, the developer decides what is merged and what is
  discarded, so every rule that says "merge into `main`" (the Closing section above, the `vibe-coding` skill)
  becomes: close the run without merging, and merge into the copy's base only when the developer says
  `Fusiona el cambio en la base`, through the worktree merge (`worktree` skill), never with git by hand.
  Nothing is pushed.
- **Acceptance work folder.** Acceptance runs and generated datasets use the copy's `work` folder (ignored by
  git), for example `-WorkDir work\acceptance`, never a fixed folder such as `D:\acceptance`.
- **Stay inside the copy.** Never write in the main copy or in other copies; a guard hook blocks it.
- **Never Claude Code's built-in worktrees** (`EnterWorktree`, `--worktree`, worktree isolation), neither in
  the main copy nor in a worktree copy.
- The main copy keeps every rule above unchanged.

## Repository rules

- Never commit `chicago-crime-data.csv` (2.3 GB reference dataset, git-ignored).
- Code, identifiers, comments and technical docs in English; CLI messages and PRDs in Spanish.
