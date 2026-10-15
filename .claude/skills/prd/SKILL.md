---
name: prd
description: "Trigger: PRD, write a PRD, nuevo PRD, product requirements, documento de producto, start a feature from a PRD. Write the product PRD that opens an ODD feature."
model: opus
effort: high
license: MIT
metadata:
  author: inercya
  version: "1.1"
---

## Activation Contract

Load before writing or revising any `*.prd.md`, or before starting a change that alters product behavior.
The PRD comes first; the ODD feature document (`odd/tasks/<feature>.md`) is derived from it.
Run inline in the main session so the PRD is agreed with the user in conversation — do not delegate the writing.

## Hard Rules

- Product document: the WHAT and WHY in business language. No class names, endpoints, file paths or line
  numbers — those belong to the ODD feature document and the code.
- Language: match the project's existing PRDs; with no precedent, ask the user once.
- Location: reuse the project's PRD convention (search `**/*.prd.md`). With none, use
  `docs/prd/<domain>/<domain>.prd.md` (general) and `docs/prd/<domain>/<change-name>.prd.md` (per change).
- A per-change PRD names its general PRD in the `Parent document` header field.
- Before declaring the parent PRD or project docs silent on a rule, re-read them. Rules that feel tacit are
  usually written down.
- `Starting state` reflects what already exists in code and data, verified by search — never assumed.
- `Out of scope` is mandatory and explicit — it is the line the next change starts from.
- Ask one question at a time when a product decision is genuinely the user's; never invent it.
- Writing a PRD does not authorize implementation.
- The `Elaboration time` section is written only by `scripts/prd_time.py`, never by hand and never estimated.

## Decision Gates

| Situation | Action |
|---|---|
| New module or domain | General PRD, no parent |
| Change inside a domain | Per-change PRD with `Parent document` |
| Correcting a rule of an earlier PRD | Annotate the earlier PRD; do not rewrite its history |
| Bug fix, refactor or tooling with no product behavior change | No PRD; go straight to ODD |

## Execution Steps

1. Read the parent PRD and the relevant project docs.
2. Verify the starting state in code and data; delegate a read-only explorer when it exceeds the inline
   evidence budget.
3. Draft from `assets/prd-template.md`; translate headings consistently when the PRD is not in English.
4. Review the draft with the user; iterate until no product question is open.
5. As the last step of the session, once the PRD and its translations are final, record the elaboration time
   in every one of them:

   ```
   python .claude/skills/prd/scripts/prd_time.py <prd.md> [<prd.en.md> ...]
   ```

   It reads this session's Claude Code history (`CLAUDE_CODE_SESSION_ID`) and splits the time into agent
   work and the person's time; a wait for the person longer than 10 minutes (`--idle-minutes`) counts only
   10 and the rest is discarded as idle. Rerun it after any later edit in the same session: it replaces this
   session's row. A revision in another session adds a row and a total. To measure a past session, pass
   `--session <id>` (the file name of its history under `~/.claude/projects/`). On exit code 2, report the
   error and leave the section out.
6. When the user authorizes implementation, open the ODD feature document linked to the PRD:

| PRD section | ODD feature document |
|---|---|
| Objective and context | Objective, problem, why |
| Starting state | Constraints and current evidence |
| Flow, Validations and messages | Task checklist |
| Out of scope | Scope limits |
| Acceptance criteria | Acceptance criteria and checks |

## Output Contract

Return the PRD path, the open product questions (if any), the elaboration time (active, agent and person,
discarded idle time) and whether it is ready to start ODD.

## References

- `assets/prd-template.md` — PRD skeleton with per-section guidance.
- `scripts/prd_time.py` — measures the elaboration time from the session history and writes it in the PRD.
  Tests: `python -m unittest discover -s .claude/skills/prd/scripts`.
