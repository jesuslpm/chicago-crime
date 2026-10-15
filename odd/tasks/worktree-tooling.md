# worktree-tooling — ODD feature document

- **Locator**: `odd/tasks/worktree-tooling.md` · Engram mirror topic `odd/worktree-tooling/tasks`
- **Branch**: `feat/worktree-tooling` from `main` at `e7efa04`. Delivery: local work-unit commits, no PRs, merge into
  `main` after full verification (user rule), no push. Test runners: `powershell -NoProfile -ExecutionPolicy
  Bypass -File scripts/worktree/tests/Test-Worktree.ps1` (scripts) and `python -m unittest discover -s
  .claude/hooks` (guard); the three cargo checks still run at close.
- **Timing**: estimate **2–3.5 h** (calibration: crime-query ~3 h 15 min vs 4–7 h; soporte-linux ~48 min vs
  1.5–2.5 h; estimates have run ~2x high). Real start **2026-10-15 11:59:02 +0100**. Waits: none.

## Specs

Product source: `docs/prd/worktrees/worktrees.prd.md` **v1.0** (decisions W1–W10, AC-W1…AC-W15). The PRD wins
on any product question; every message below is quoted verbatim from its "Validaciones y mensajes" table and
must be printed exactly (placeholders filled). Read the PRD whole before writing.

- **S1 — Names (W2).** "De un único nombre de cambio `<n>` (minúsculas, dígitos y guiones simples) se deriva
  todo": branch `feat/<n>`, folder `C:\wt\chicago-crime-<n>`, Engram project `chicago-crime-<n>`, work folder
  "la carpeta `work` dentro de la copia, ignorada por git", discard tag `exp/<n>`.
- **S2 — Create (AC-W1, AC-W2, AC-W3, AC-W14).** Flow "Crear una copia" steps 1–7. "Antes de crear nada se
  comprueba todo: que se está en la copia principal, que el nombre es válido y cabe en el límite de rutas de
  Windows, que la base existe y que no hay ya una rama, carpeta, copia registrada ni proyecto Engram con ese
  nombre. Si algo falla, no se crea nada." The CSV "como un enlace al de la copia principal … El CSV queda
  marcado de solo lectura". "Se copian los permisos locales del agente y el registro de skills, se crea la
  carpeta de trabajo y se asigna a la copia su proyecto Engram propio." Clean `git status`, new editor window
  at the root, summary with "carpeta, rama, base, proyecto Engram y la frase con la que empezar". "Si un paso
  de la creación falla, se dice qué paso falló y qué quedó creado, y se limpia eliminando la copia; la creación
  nunca deshace sus propios pasos." Messages: `Esto se hace desde la copia principal, y esta sesión está en la
  copia '<carpeta>'. No se ha cambiado nada.`; `Nombre '<n>' no válido: solo minúsculas, dígitos y guiones
  simples (por ejemplo, startswith-odd). No se ha creado nada.`; `El nombre '<n>' tiene <x> caracteres; el
  máximo es <m> para que ninguna ruta pase de 259. No se ha creado nada.`; `La base '<base>' no existe. Para
  un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada.`; `Ya existe <qué>
  '<valor>'. No se ha creado nada.`; `Falló el paso '<paso>': <causa>. Ya creado: <lista>. Corrige la causa y
  limpia con la frase de eliminar el worktree '<n>'.`
- **S3 — List.** "Desde cualquier copia, se ven todas: nombre, rama, base, si la rama ya está fusionada en su
  base, carpeta y proyecto Engram." Empty: `No hay worktrees (solo la copia principal).`
- **S4 — Merge (AC-W7, AC-W8, AC-W14, W9).** "la base avanza hasta la rama sin commit de fusión … la rama debe
  contener antes toda la base. Si la base está abierta en una copia con cambios sin confirmar, la fusión se
  niega". "Si la base está abierta en otra copia, esa copia se actualiza también". "La copia y su rama se
  conservan después de fusionar. Nunca se publica nada en el remoto." Messages: `La fusión se hace desde la
  copia del cambio, no desde la copia principal. No se ha cambiado nada.`; `La copia tiene cambios sin
  confirmar: <lista>. Confírmalos o descártalos antes. No se ha cambiado nada.`; `La base '<base>' tiene <x>
  commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión.`; `La rama
  'feat/<n>' no tiene nada que la base '<base>' no tenga. No se ha cambiado nada.`; `La base '<base>' está
  abierta en '<carpeta>' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión.`;
  success `'<base>' avanza hasta 'feat/<n>' (<commit>). La copia y la rama se conservan.`
- **S5 — Remove and discard (AC-W10, AC-W11, AC-W14, W5).** Merged + clean: "se eliminan la copia, su carpeta
  y su rama. El proyecto Engram y la memoria de Claude Code de esa carpeta se conservan." Unmerged: refused
  unless discard; discard "crea la etiqueta `exp/<n>` en la punta de la rama … y después se eliminan la copia,
  la rama, el proyecto Engram y la memoria de Claude Code de esa carpeta." "Eliminar nunca fuerza nada ni
  cierra procesos … Se puede repetir sin riesgo." "El CSV de la copia principal no cambia nunca." Messages:
  `No se elimina '<n>': la rama tiene <x> commit(s) que no están en '<base>'. Para descartarla y conservar su
  historia en exp/<n>, pide descartar el worktree. No se ha eliminado nada.`; `No se elimina '<n>'. Se
  perdería: <lista>. No se ha eliminado nada.`; `Esta sesión está dentro de '<carpeta>'. Elimínala desde la
  copia principal.`; `No se pudo borrar '<archivo>': está en uso. Cierra el editor, la aplicación o las
  pruebas que lo usan y repite. No se ha cerrado ningún proceso.`; `Ya existe la etiqueta exp/<n>. No se ha
  eliminado nada.`
- **S6 — Rehearsal (AC-W12, AC-W13, W7).** Create the rehearsal base "a partir de `main`"; discard "En un solo
  paso se eliminan todas las copias cuya base es la de ensayo, sus ramas, sus etiquetas `exp/`, sus proyectos
  Engram, la memoria de Claude Code de sus carpetas y la propia base. `main` no cambia." Messages: `'main' no
  es una base de ensayo. No se ha cambiado nada.`; `La base '<base>' ya existe. No se ha creado nada.`
- **S7 — Guard (AC-W9, W8).** In a copy it blocks "comandos que escriban en la carpeta de la copia principal o
  de otro worktree, y los git que muevan la base a mano (checkout, merge, push o reset sobre ella)"; in the
  main copy it blocks nothing; it has its own tests. Messages: `Bloqueado: esta sesión trabaja en '<carpeta>'
  y no escribe fuera de ella.`; `Bloqueado: la base '<base>' solo avanza con la fusión del worktree.`
- **S8 — Isolation (AC-W4, AC-W5, AC-W6).** A write to a copy's CSV fails and the main CSV keeps size and
  content; two copies build and run acceptance with their work folders at the same time without interfering;
  a memory saved in one copy's Engram project is not found from another copy or the main copy.
- **S9 — Documentation (AC-W15).** Project instructions section ("En una copia, \"fusionar en `main`\" pasa a
  ser \"fusionar en la base de la copia, solo con la fusión del worktree\"; la carpeta de trabajo de la
  verificación de aceptación es la de la copia; el agente nunca usa los worktrees integrados de Claude
  Code."); worktree skill ("una tabla que lleva cada frase a su operación y dice en qué sesión se pide, sin
  lógica propia"); README short section linking to the guide; user guide with "el flujo completo, incluido el
  ensayo, y una tabla \"mensaje → qué hacer\" con todos los mensajes". Phrases (verbatim, PRD "Frases"):
  `Crea un worktree para el cambio <n>`, `Crea un worktree para el cambio <n> sobre <base>`, `¿Qué worktrees
  tengo?`, `Fusiona el cambio en la base`, `Elimina el worktree <n>`, `Descarta el worktree <n>`, `Crea la base
  de ensayo <base>`, `Descarta el ensayo <base>`. Wrong session: "el agente dice dónde hay que pedirlo y se
  detiene". `docs/methodology/prompts-worktrees.md` stays consistent with the PRD.

## Implementation decisions (defaults chosen without asking, per the user's standing rule)

- **Language and runtime**: Windows PowerShell 5.1 compatible scripts, run as `powershell -NoProfile
  -ExecutionPolicy Bypass -File scripts/worktree/<script>.ps1` (like `scripts/acceptance`). Files with
  non-ASCII text are saved **UTF-8 with BOM** (5.1 reads BOM-less files as ANSI). User messages in Spanish,
  code and comments in English.
- **Scripts** (`scripts/worktree/`): `WorktreeCommon.psm1` (shared helpers); `New-Worktree.ps1 -Name <n>
  [-Base main] [-Root C:\wt] [-NoOpen]`; `Get-Worktrees.ps1`; `Merge-Worktree.ps1` (no parameters, run from
  the copy); `Remove-Worktree.ps1 -Name <n> [-Discard] [-Root C:\wt]`; `New-RehearsalBase.ps1 -Base <b>`;
  `Remove-Rehearsal.ps1 -Base <b> [-Root C:\wt]`. Test seams (not for daily use): `-ClaudeProjectsDir`
  (default `~\.claude\projects`) on the scripts that delete Claude memory, `-NoOpen` on create.
- **Exit codes**: 0 done; 3 merge refused because the base has commits the branch lacks (go back once); 1 any
  other refusal or error.
- **Main copy detection**: `.git` is a folder in the main copy and a file in a copy; the main copy is the
  parent of `git rev-parse --git-common-dir`.
- **Recorded base**: `git config branch.feat/<n>.worktreebase <base>` in the shared repository config. Every
  copy ever created on a base is also appended to the multi-valued `worktreebase.<base>.created`, so the
  rehearsal discard also finds copies already removed (their Engram projects and Claude memory) and the `exp/`
  tags of those names. It goes by name: it skips a name whose live branch records another base, but a tag,
  Engram project or memory of the same name made outside the rehearsal is not told apart (verifier A9; the
  documented order — discard the rehearsal before the demo — prevents it).
- **Name length**: max = 259 − length of `C:\wt\chicago-crime-` prefix path − 1 − max(longest tracked path in
  the base, 100 reserved for build output).
- **CSV**: NTFS hard link to the main copy's CSV; the read-only attribute is shared by all links. Deleting a
  read-only link clears the attribute for every link, so removal clears it, deletes the link and restores it on
  the main CSV in a `finally`. A missing main CSV is a creation refusal (it is part of the copy's contract).
- **Private files**: `.claude/settings.local.json` and `.atl/skill-registry.md` only, when present.
- **Engram**: `engram init chicago-crime-<n>` inside the copy (writes `.engram/config.json`, ignored);
  `engram delete project <name> --hard` on discard and rehearsal discard. Normal removal keeps the project.
- **Claude Code memory**: only `<ClaudeProjectsDir>\<path with ':' and '\' turned into '-'>\memory` is deleted
  (case-insensitive match), never the session history.
- **Merge**: `merge-base --is-ancestor` checks; when the base is checked out in a copy, refuse if it has
  tracked uncommitted changes, else `git -C <that copy> merge --ff-only feat/<n>`; when it is not checked out,
  `git update-ref refs/heads/<base> <new> <old>` (compare-and-swap).
- **Rehearsal discard with dirty copies**: refused, listing them (never force, like removal). It also works
  when some copies are already gone and can be repeated.
- **Guard**: Python (like the skill scripts), `.claude/hooks/worktree_guard.py`, wired in a new versioned
  `.claude/settings.json` as a `PreToolUse` hook for `Bash|PowerShell|Write|Edit|NotebookEdit` that starts
  Python only when `$CLAUDE_PROJECT_DIR/.git` is a file. File tools: block a path inside the main copy or another
  copy. Shell commands: block when they name the main copy or another copy, unless every segment starts with a
  read-only command; block git commands that move the base (`checkout`/`switch` to it, `branch -f/-d/-D/-m/-M`
  on it, `update-ref refs/heads/<base>`, `push` or `fetch` refspecs ending in it, `merge` or `reset` run on
  it). Assumption: `main` is protected the same way even when it is not the base, so a rehearsal never touches
  it. Unreadable input never blocks. Exit 2 with the PRD message.
- **Docs**: English like the README (Spanish UI strings verbatim with English meaning): guide
  `docs/worktrees.md`; skill `.claude/skills/worktree/SKILL.md`; CLAUDE.md section; README section.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T0 | S1, S9 | inline | done | `6d679b4`: PRD, prompts doc, `.gitignore` (`.engram/`, `/work/`), this document. Assess: medium, under budget (reviewed with the slice). |
| T1 | S1–S6, S8 | delegated writer A (parallel with T2/T3: disjoint surfaces) | done | RED: every script test failed with `-File ... does not exist` (12:20:42); GREEN 39/39 (writer 12:37–12:39; parent rerun 39/39). Extra script messages added to the guide (L6). |
| T2 | S7 | delegated writer B (parallel) | done | `5d5d366`. RED: `Ran 29 tests … FAILED (failures=82, errors=2)`; GREEN: 29/29 (parent rerun OK); closing-report tests 16/16; settings pre-check run under Git Bash. |
| T3 | S9 | delegated writer C (parallel; interfaces fixed above) | done | `d6f55e0`. 22/22 PRD messages verbatim in the guide; 8/8 phrases in skill and guide. Parent fix L3. |
| T4 | all | independent verifier + real rehearsal on copies | in progress | Verifier: S1–S9 and AC-W1…AC-W15 PASS, no blockers, 11 advisories (L8). AC-W5: two copies, release builds 80 s each and generator acceptance 576 s each (15/15 PASS each) at the same time. Fixes A1, A4, A5, A10 (L8). |
| T5 | — | close: closing report, merge into `main` | pending | |

## Log

- **L1** (2026-10-15, user, verbatim): "Vamos con el ODD. ¿Me darías una estimación de tiempo si no te la
  pido?" — preceded by the approval of the PRD ("Sí, las apruebo") and the request that produced
  `docs/methodology/prompts-worktrees.md`: "antes necesito los prompts en un md del repo que harían falta para realizar cada
  una de las operaciones posibles en el worktree".
- **L2** (11:59:02) Branch created from `e7efa04`. Risk: high (deletes branches, folders, Engram projects and
  Claude memory; adds a guard) → independent verifier at close and RDD review per commit.
- **L3** Writer C found that "merge after verification" in a copy would make both experiment runs merge,
  contradicting "Solo se fusiona la copia de ODD" and the PRD actor rule "decide qué se fusiona y qué se
  descarta" (the developer). Decision: in a copy the run closes without merging; the merge happens only on the
  developer's `Fusiona el cambio en la base` (CLAUDE.md *Working in a worktree*, guide). Guard deviations
  accepted: PowerShell aliases count as read-only; `pull`, `cherry-pick`, `revert`, `am`, `branch -C` and
  `rebase <up> <base>` also count as base moves; commands that name the base without moving it pass.
- **L4** Slice `e7efa04..d6f55e0` (T0, T2, T3; 12 files, 2,123 lines) assessed high (process start in the
  guard) → review granted, lenses risk + reliability + readability, lineage `review-58899355a41ea11e`.
  Approved with 14 advisory findings (no blockers); acknowledged, authority burned; reviewed boundary →
  `d6f55e0`.
- **L5** Advisory follow-ups fixed in `923e15a` (RED 3 failures → GREEN 30/30): spaced redirects
  (`git checkout ensayo > nul`) no longer hide a base move (R2-redirect); the base is read from the shared
  config file when git cannot run, so a rehearsal base stays protected (R3-002). Accepted as documented
  limitation: the shell rule is textual (relative paths, variables, nested shells pass; R1-001). The "docs
  route to absent scripts" findings close with T1.
- **L6** T1 deviations accepted: Claude memory folder name maps every non-alphanumeric character to `-` (what
  Claude Code does); New-RehearsalBase marks the base (`worktreebase.<b>.rehearsal=true`) and Remove-Rehearsal
  refuses unmarked branches with the PRD message generalised to `'<b>' no es una base de ensayo. No se ha
  cambiado nada.`; Remove-Rehearsal skips a reused name whose live branch records another base; tests isolate
  Engram with `ENGRAM_DATA_DIR`; 16 extra messages in the PRD style (tool missing, Engram failure, CSV missing
  or on another drive, merge preconditions, partial removal, nothing to remove, foreign folder, rehearsal base
  checked out, nothing left) — added to the guide's message table. Found in the real repo: a worktree
  `C:\wt\chicago-crime-start-confirmation` (`feat/start-confirmation`, created 12:10 by another session of the
  user, not by this run); left untouched and excluded from verification.
- **L7** (12:44:30) T1 committed `43b7965`. Assess `d6f55e0..43b7965`: medium/under budget, escalated to high
  (item 1: deletes copies, branches, tags, Engram projects, Claude memory; toggles the CSV attribute; moves
  refs). Independent verifier launched first (whole document, baseline `e7efa04`, real rehearsal on a scratch
  clone with the real CSV hard-linked, AC-W5 parallel builds + acceptance); the four-lens review runs on the
  final slice after its corrections.
- **L8** Verifier report (scratch clone with `main` at `43b7965`, real CSV hard-linked, `ENGRAM_DATA_DIR`
  isolated): every S# and AC passes. AC-W5 measured: builds 12:54:49–12:56:09, acceptance 12:56:22–13:05:58,
  both copies exit 0. Real CSV SHA-256 `5CB5FA41…AE35D`, 2,406,401,982 bytes before and after, now read-only
  (set by the first copy, as designed). Checks: scripts 39/39, guard 30/30, cargo fmt/clippy OK, cargo test 425
  passed. Advisories fixed as regression tests (RED → GREEN): A1 the guard compares branch names without case
  (`git checkout ENSAYO` moved the base on Windows; guard RED 4 → 31/31); A4 a failed step's cause is the
  tool's first line, at most 200 characters (a help text followed it); A5 an invalid name on removal ends
  with `No se ha eliminado nada.`; A10 guide rows fixed (`git` is not checked; new rows for the generalised
  rehearsal-base refusal and the removal invalid name). Accepted: A2 8.3 short names of a protected folder pass
  the guard; A3 textual limit (R1-001); A6 a plain removal names the kept Engram project even if it never
  existed; A7 the creation summary's next step is generic (the first prompt depends on the run: PRD, vibe or
  ODD, see the prompts doc); A8 `worktreebase.main.created` keeps growing; A9 name-based rehearsal cleanup;
  A11 `run-acceptance.ps1` output encoding when redirected (baseline, not this change).
- **L9** Fixes committed `22e2591`. Review of slice `d6f55e0..22e2591` (2,385 lines, escalated high, consent
  granted, four lenses risk + resilience + readability + reliability): approved, 5 warnings + 4 suggestions, no
  blockers; acknowledged, authority burned; reviewed boundary → `22e2591`. Follow-ups fixed (scripts RED: the
  foreign-folder test failed, its `notes.txt` was deleted → GREEN): R1-001 a folder without `.git` is deleted as
  a removal's leftover only while the copy's base is still recorded (the branch, and its config, go after the
  folder), so a folder someone else made is refused with the foreign-folder message; R3-merge-holder-race a
  fast-forward that fails because the base moved in its holder now exits 3; R4 the rehearsal discard checks
  that its config section is really gone; R3-rehearsal-skip-untested the reused-name skip now has a test.
  Accepted: R2-dup-copy-plan (the per-copy checks are duplicated in Remove-Worktree and Remove-Rehearsal;
  refactor left for later, both paths are tested). No test forces the holder race or a config lock (races).
