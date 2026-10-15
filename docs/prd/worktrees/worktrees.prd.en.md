# Parallel working copies (worktrees) — PRD

**Version**: 1.0
**Date**: 2026-10-15
**Parent document**: none

> English translation of [worktrees.prd.md](worktrees.prd.md). The Spanish original is authoritative if they differ.

## Objective and context

Let the developer work on several changes at once, each in its own copy of the repository with its own agent
session, without one copy breaking, slowing down or contaminating another, and let them rehearse the whole
process and throw it away afterwards without leaving a trace on `main`.

**Why now**: for the methodology talk, the query app's prefix search ("startswith") will be implemented twice
at the same time from one PRD: once with vibe coding and once with ODD, measuring the time of each. The
agreed flow is:

1. One copy to write the PRD, which is merged into the base.
2. Two copies that start from that base, one for vibe coding and one for ODD, each with its own fresh session,
   working at the same time.
3. Only the ODD copy is merged; the vibe coding copy is discarded.

Before the real demo, the developer rehearses the whole flow on a throwaway base (for example `ensayo`) that
plays the role of `main`, and discards everything at the end. The rehearsal and the demo use the same
commands; only the base changes.

A plain git worktree is not enough: it does not bring the reference CSV, it shares the agent's memory with the
other copies, the acceptance checks of two copies collide when they use the same work folder, and the current
rule "merge into `main` when done" cannot be followed from a copy and is not the right one during a rehearsal.

The idea comes from another project's working-copy tooling (LandPro), adapted: here there is no database per
copy and no remote integration branch, and what has to be isolated is the CSV, the work folders and the
memory.

### Starting state

Verified on 2026-10-15 on the reference machine, with the repository at commit `e7efa04`:

| Aspect | Situation |
|---|---|
| Working copies | Only the main copy exists. There are no tools to create, list, merge or remove copies. |
| Branches | Changes have been made on `feat/<name>` branches and brought into `main` almost always by moving it forward (no merge commit). |
| Reference CSV | 2,406,401,982 bytes at the root of the main copy, ignored by git and not protected against writes. A new copy does not have it. |
| Data-loss precedent | In the generator run, pointing the log at the input truncated the CSV. The generator has been fixed, but it shows an accidental write is possible. |
| Build | Each copy builds into its own output folder; no setting shares it. A new copy builds from scratch. |
| Acceptance checks | The scripts default to the CSV and executables of the copy they are in, and require a work folder. The README uses fixed folders (for example `D:\acceptance`), so two copies that follow the README collide. |
| Private files | The agent's local permissions and the skill registry exist only in the main copy, and git does not version them. |
| Engram memory | Every copy of this repository writes to the `chicago-crime` project, because Engram derives it from the git remote. A folder can be given its own project, and a whole project can be deleted. The folder for that setting is not ignored by git yet. |
| Claude Code memory | It is per folder path, not per repository: a new copy does not load the main copy's memory, and a copy created at a path used before gets back the memory left there. |
| Merge rule | The project instructions say "merge the feature branch into `main` after full verification". From a copy, the agent cannot switch to `main` (the main copy has it), and during a rehearsal `main` must not be touched. |
| Guards | The project has no agent guard (hook) and no shared agent settings. |
| Copies root | `C:\wt` already exists and holds LandPro copies. |

## Actors

- **Developer**: decides which copies to create, opens a fresh agent session in each, decides what is merged
  and what is discarded, and rehearses the flow before the demo.
- **Agent in the main copy**: creates, lists and removes copies, and creates and discards the rehearsal base.
  It does not work on the changes.
- **Agent in a copy**: works on a single change inside its copy and, when done, merges it into that copy's
  base. It never touches the main copy or other copies.

## Flow

### Names

Everything derives from a single change name `<n>` (lowercase letters, digits and single hyphens):

| Item | Value |
|---|---|
| Branch | `feat/<n>` |
| Folder | `C:\wt\chicago-crime-<n>` |
| Engram project | `chicago-crime-<n>` |
| Work folder | the `work` folder inside the copy, ignored by git |
| Tag when discarded | `exp/<n>` |

### Create a copy

1. From the main copy, the developer asks for a copy for change `<n>`, optionally with a base other than
   `main` (for example `ensayo`).
2. Everything is checked before anything is created: that this is the main copy, that the name is valid and
   fits the Windows path limit, that the base exists, and that no branch, folder, registered copy or Engram
   project with that name exists yet. If anything fails, nothing is created.
3. The `feat/<n>` branch is created from the base and the copy in its folder, and its base is recorded.
4. The copy gets the reference CSV as a link to the main copy's one (it takes no space). The CSV is marked
   read-only, which protects all its links.
5. The agent's local permissions and the skill registry are copied, the work folder is created and the copy
   is given its own Engram project.
6. It is checked that git sees the copy as clean, and a new editor window opens at its root, where the
   developer starts a fresh agent session.
7. The final summary shows folder, branch, base, Engram project and the phrase to start with.

If a creation step fails, it says which step failed and what was created, and cleanup is done by removing the
copy; creation never undoes its own steps.

### Work in a copy

1. The agent works as usual (PRD, ODD or vibe coding), committing on its branch.
2. It builds into its copy's output folder and uses its copy's work folder for acceptance checks and
   generated datasets.
3. It reads and writes memory only in the copy's Engram project.
4. A guard prevents it from writing to the main copy or to other copies, and from moving the base by hand.

### Merge a copy into its base

1. From the copy, the agent brings into its branch whatever is new in the base, resolves conflicts with the
   developer and runs the full checks again.
2. It asks for the merge. It is checked that the copy is clean, that the branch contains the whole base and
   that it adds something new.
3. The base moves forward to the branch, without the agent leaving its copy. If the base is open in another
   copy, that copy is updated too, unless it has uncommitted changes: then the merge is refused.
4. The copy and its branch are kept after the merge. Nothing is ever published to the remote.

### List copies

From any copy, all of them are shown: name, branch, base, whether the branch is already merged into its base,
folder and Engram project.

### Remove a copy

1. From the main copy, the developer asks to remove copy `<n>`.
2. If the branch is merged into its base and the copy has no uncommitted changes, the copy, its folder and its
   branch are removed. The Engram project and that folder's Claude Code memory are kept.
3. If the branch is not merged, it is refused, unless discarding it is asked for explicitly. When
   discarding, the tag `exp/<n>` is created at the tip of the branch (its history and closing report stay
   reachable) and then the copy, the branch, the Engram project and that folder's Claude Code memory are
   removed.
4. Removing never forces anything or stops processes: if a file is in use, it says which one and asks to close
   whatever uses it. It can be repeated safely.
5. The main copy's CSV never changes.

### Rehearse and discard the rehearsal

1. From the main copy, the developer creates the rehearsal base from `main`.
2. They run the whole flow with the same commands as in the demo, giving that base when creating each copy.
3. At the end, they ask to discard the rehearsal. In a single step, every copy whose base is the rehearsal
   base is removed, together with its branch, its `exp/` tags, its Engram project, its folder's Claude Code
   memory and the base itself. `main` does not change.

### Phrases

| Developer phrase (verbatim Spanish / English meaning) | Session | What it does |
|---|---|---|
| "Crea un worktree para el cambio `<n>`"<br>*Create a worktree for change `<n>`* | main | Create with base `main` |
| "Crea un worktree para el cambio `<n>` sobre `<base>`"<br>*Create a worktree for change `<n>` on `<base>`* | main | Create with another base |
| "¿Qué worktrees tengo?"<br>*What worktrees do I have?* | any | List |
| "Fusiona el cambio en la base"<br>*Merge the change into the base* | copy | Merge |
| "Elimina el worktree `<n>`"<br>*Remove worktree `<n>`* | main | Remove |
| "Descarta el worktree `<n>`"<br>*Discard worktree `<n>`* | main | Remove, discarding the branch |
| "Crea la base de ensayo `<base>`"<br>*Create rehearsal base `<base>`* | main | Create a rehearsal base |
| "Descarta el ensayo `<base>`"<br>*Discard rehearsal `<base>`* | main | Discard the rehearsal |

If the phrase is said in the wrong session, the agent says where to ask for it and stops.

## Validations and messages

| Situation | Rule | Message to the user (verbatim Spanish UI text / English meaning) |
|---|---|---|
| Create, remove or manage the rehearsal from a copy | Refused without changing anything | `Esto se hace desde la copia principal, y esta sesión está en la copia '<carpeta>'. No se ha cambiado nada.`<br>*This is done from the main copy, and this session is in copy `<folder>`. Nothing was changed.* |
| Merge from the main copy | Refused | `La fusión se hace desde la copia del cambio, no desde la copia principal. No se ha cambiado nada.`<br>*The merge is done from the change's copy, not from the main copy. Nothing was changed.* |
| Name with invalid characters | Refused | `Nombre '<n>' no válido: solo minúsculas, dígitos y guiones simples (por ejemplo, startswith-odd). No se ha creado nada.`<br>*Invalid name `<n>`: only lowercase letters, digits and single hyphens (for example, startswith-odd). Nothing was created.* |
| Name too long for Windows paths | Refused | `El nombre '<n>' tiene <x> caracteres; el máximo es <m> para que ninguna ruta pase de 259. No se ha creado nada.`<br>*Name `<n>` has `<x>` characters; the maximum is `<m>` so that no path exceeds 259. Nothing was created.* |
| The base does not exist | Refused; a base is never created implicitly | `La base '<base>' no existe. Para un ensayo, créala antes con la frase de crear base de ensayo. No se ha creado nada.`<br>*Base `<base>` does not exist. For a rehearsal, create it first with the create-rehearsal-base phrase. Nothing was created.* |
| A branch, folder, registered copy or Engram project with that name already exists | Refused | `Ya existe <qué> '<valor>'. No se ha creado nada.`<br>*`<what>` `<value>` already exists. Nothing was created.* |
| A creation step fails | Stops and says what is left | `Falló el paso '<paso>': <causa>. Ya creado: <lista>. Corrige la causa y limpia con la frase de eliminar el worktree '<n>'.`<br>*Step `<step>` failed: `<cause>`. Already created: `<list>`. Fix the cause and clean up with the remove-worktree phrase for `<n>`.* |
| Merge with uncommitted changes | Refused | `La copia tiene cambios sin confirmar: <lista>. Confírmalos o descártalos antes. No se ha cambiado nada.`<br>*The copy has uncommitted changes: `<list>`. Commit or discard them first. Nothing was changed.* |
| The base has commits the branch does not contain | Refused; go back to merge step 1 once | `La base '<base>' tiene <x> commit(s) que tu rama no contiene. Incorpóralos, vuelve a verificar y repite la fusión.`<br>*Base `<base>` has `<x>` commit(s) your branch does not contain. Bring them in, verify again and repeat the merge.* |
| The branch adds nothing to the base | Refused | `La rama 'feat/<n>' no tiene nada que la base '<base>' no tenga. No se ha cambiado nada.`<br>*Branch `feat/<n>` has nothing that base `<base>` does not have. Nothing was changed.* |
| The base is open in a copy with uncommitted changes | Refused | `La base '<base>' está abierta en '<carpeta>' con cambios sin confirmar. Confírmalos o guárdalos allí y repite la fusión.`<br>*Base `<base>` is open in `<folder>` with uncommitted changes. Commit or stash them there and repeat the merge.* |
| Successful merge | The base moves forward to the branch | `'<base>' avanza hasta 'feat/<n>' (<commit>). La copia y la rama se conservan.`<br>*`<base>` moves forward to `feat/<n>` (`<commit>`). The copy and the branch are kept.* |
| Removing an unmerged branch without asking to discard it | Refused | `No se elimina '<n>': la rama tiene <x> commit(s) que no están en '<base>'. Para descartarla y conservar su historia en exp/<n>, pide descartar el worktree. No se ha eliminado nada.`<br>*`<n>` is not removed: the branch has `<x>` commit(s) that are not in `<base>`. To discard it and keep its history in exp/`<n>`, ask to discard the worktree. Nothing was removed.* |
| Removing with uncommitted changes | Refused, also when discarding | `No se elimina '<n>'. Se perdería: <lista>. No se ha eliminado nada.`<br>*`<n>` is not removed. Would be lost: `<list>`. Nothing was removed.* |
| Removing the copy the session runs in | Refused | `Esta sesión está dentro de '<carpeta>'. Elimínala desde la copia principal.`<br>*This session is inside `<folder>`. Remove it from the main copy.* |
| A file in the copy is in use | Stops without forcing | `No se pudo borrar '<archivo>': está en uso. Cierra el editor, la aplicación o las pruebas que lo usan y repite. No se ha cerrado ningún proceso.`<br>*Could not delete `<file>`: it is in use. Close the editor, app or tests using it and repeat. No process was stopped.* |
| Tag `exp/<n>` already exists when discarding | Refused | `Ya existe la etiqueta exp/<n>. No se ha eliminado nada.`<br>*Tag exp/`<n>` already exists. Nothing was removed.* |
| Discarding the rehearsal with base `main` | Refused | `'main' no es una base de ensayo. No se ha cambiado nada.`<br>*`main` is not a rehearsal base. Nothing was changed.* |
| Creating a rehearsal base that already exists | Refused | `La base '<base>' ya existe. No se ha creado nada.`<br>*Base `<base>` already exists. Nothing was created.* |
| No copies | Reported | `No hay worktrees (solo la copia principal).`<br>*No worktrees (only the main copy).* |
| A copy's agent tries to write to the main copy or another copy | The guard blocks it | `Bloqueado: esta sesión trabaja en '<carpeta>' y no escribe fuera de ella.`<br>*Blocked: this session works in `<folder>` and does not write outside it.* |
| A copy's agent tries to move the base by hand | The guard blocks it | `Bloqueado: la base '<base>' solo avanza con la fusión del worktree.`<br>*Blocked: base `<base>` only moves forward through the worktree merge.* |
| A write to the reference CSV from any copy | The system rejects it because it is read-only | (the usual app or system error) |

### Accepted risks

- **There is only one CSV.** All copies read the same file. Read-only prevents accidental writes, but anyone
  who removes that attribute by hand exposes it in every copy.
- **Each copy's first build is a full build.** It takes the same time in every copy, so it does not distort
  the comparison between them, but it counts in each run's time.
- **The guard only acts in sessions opened at a copy's root.** A session opened in a subfolder does not load
  it.

## Documentation

- **Project instructions**: a short section on copies. In a copy, "merge into `main`" becomes "merge into the
  copy's base, only through the worktree merge"; the acceptance work folder is the copy's one; the agent never
  uses Claude Code's built-in worktrees.
- **Worktree skill**: a table mapping each phrase to its operation and the session where it is asked for, with
  no logic of its own.
- **README**: a short section on working with copies, linking to the guide.
- **User guide**: the whole flow, including the rehearsal, and a "message → what to do" table with every
  message in this section.

## Reuse

- The ideas of the LandPro tooling and of the repository's generic parallel development guide: one name from
  which everything derives, check before creating, never force when removing, and messages that say what to
  do.
- The current acceptance scripts, unchanged: they already use the CSV and executables of the copy they are in
  and accept any work folder.
- Engram's existing mechanisms to give a folder its own project and to delete a project.
- The current working flow (PRD, ODD, vibe coding, closing report), which does not change inside a copy.

## Out of scope

- Linux and other systems: the tools are Windows only.
- Publishing branches to the remote, opening PRs or merging on the remote.
- Copying a discarded branch's closing report or metrics to the base: they stay in its `exp/<n>` tag.
- Sharing build output between copies or building when the copy is created.
- Changing the acceptance scripts or the behavior of the generator and the query app.
- Keeping Claude Code's session history: removing or discarding only deletes its memory.
- The "startswith" experiment itself: this change only prepares the tools.

## Acceptance criteria

1. **AC-W1** — Given a valid name and the main copy, when a copy is created without a base, then the branch
   `feat/<n>` from `main`, the folder `C:\wt\chicago-crime-<n>`, the linked CSV with the same size as the
   original, the local permissions and the skill registry, the work folder, the Engram project
   `chicago-crime-<n>` and the recorded base exist, and git sees the copy as clean.
2. **AC-W2** — Given an existing rehearsal base, when a copy is created on it, then its branch starts from
   that base and the list shows it as the base.
3. **AC-W3** — Given an invalid, too long or already used name, or a base that does not exist, when creating
   the copy is requested, then the matching message is shown and no branch, folder, registered copy or Engram
   project is created.
4. **AC-W4** — Given a copy, when its CSV is written, then the write fails and the main copy's CSV keeps its
   size and content.
5. **AC-W5** — Given two copies, when each builds and runs the acceptance checks with its work folder at the
   same time, then neither blocks or alters the other's results.
6. **AC-W6** — Given two copies, when one copy's agent saves a memory in Engram, then the other copy's agent
   does not find it, and neither does the main copy.
7. **AC-W7** — Given a copy whose branch is up to date with the base, when it is merged, then the base moves
   forward to the branch, the main copy (if it has the base open and clean) is updated, and the copy and the
   branch are kept.
8. **AC-W8** — Given a base with commits the branch does not contain, or a copy with uncommitted changes, or a
   base open in a copy with uncommitted changes, when merging is requested, then the matching message is shown
   and the base does not change.
9. **AC-W9** — Given an agent session in a copy, when it tries to write to the main copy or another copy, or
   to move the base with git, then the guard blocks it with its message; and in the main copy the guard blocks
   nothing.
10. **AC-W10** — Given a merged, clean copy, when it is removed, then the copy, its folder and its branch are
    gone; its Engram project and Claude Code memory are kept; and the main copy's CSV does not change.
11. **AC-W11** — Given an unmerged copy, when removing it without discarding is requested, then it is refused
    and nothing changes; when discarding it is requested, then the tag `exp/<n>` exists at the tip of its
    branch and the copy, the branch, the Engram project and its folder's Claude Code memory are gone.
12. **AC-W12** — Given a complete rehearsal (PRD merged into the rehearsal base, two copies from it, one merged
    and one discarded), when the rehearsal is discarded, then no copy, branch, `exp/` tag, Engram project or
    Claude Code memory from the rehearsal is left, the rehearsal base no longer exists and `main` is at the
    same commit as before starting.
13. **AC-W13** — Given the demo flow, when it is run with base `main`, then exactly the same phrases as in the
    rehearsal are used, changing only the base when creating each copy.
14. **AC-W14** — Given creation or removal requested from a copy, or a merge requested from the main copy,
    then it is refused with its message without changing anything.
15. **AC-W15** — Given the user guide, when a developer follows it, then they can run the whole rehearsal
    without further help, and every message in this document appears in its "message → what to do" table.

## Decisions

- **W1 — Location**: this PRD opens its own development-tooling domain, with no parent, separate from the
  product PRDs. The project instructions say tooling needs no PRD; the developer chose PRD + ODD for this
  change.
- **W2 — Names**: branch `feat/<n>`, like the branches the repository already has, and folder
  `C:\wt\chicago-crime-<n>`, with the project prefix to tell it apart among windows.
- **W3 — CSV**: a link to the main copy's CSV, marked read-only. Reason: it takes no space or time, the
  acceptance scripts work unchanged and read-only protects every copy at once.
- **W4 — Work folder**: inside each copy and ignored by git. Reason: two copies cannot collide and it goes away
  with the copy.
- **W5 — Discard**: an unmerged branch is only removed when discarding it is asked for, and it is tagged
  `exp/<n>` first. The closing report is not copied to the base.
- **W6 — Memory**: one Engram project per copy, so the vibe coding and ODD sessions do not see each other
  during the race. Claude Code memory is deleted when discarding because the rehearsal and the demo use the
  same names and therefore the same folders.
- **W7 — Rehearsal**: the rehearsal base is created and discarded with their own phrases; creating a copy on a
  base that does not exist fails, so a typo does not create another base.
- **W8 — Guard**: in a copy, it blocks writing to the main copy or other copies and moving the base by hand.
  It has its own tests.
- **W9 — Merge**: the base moves forward to the branch with no merge commit, as has been done so far on
  `main`; that is why the branch must first contain the whole base. If the base is open in a copy with
  uncommitted changes, the merge is refused so those changes are not mixed in.
- **W10 — Windows only**: like the acceptance scripts; the demo and the rehearsal run on Windows.

## References

- [Generic guide to parallel development with worktrees](../../methodology/parallel-developing.md).
- LandPro's working-copy PRD and tooling, where the ideas come from.
- [Project instructions](../../../CLAUDE.md): workflow, merge after verification and closing. When this change
  closes, they are annotated with the merge rule inside a copy.

## Elaboration time

Measured from the Claude Code session history. When the person takes more than 10 min to reply, 10 min count and the rest is discarded as idle time. Interventions: messages and answers from the person after the initial request.

| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |
|---|---|---|---|---|---|---|---|
| bd359d3a | 2026-10-15 11:22 UTC+01:00 | 21 min | 0 min | 21 min | 6 min | 15 min | 10 |

<!-- prd-time: [{"session": "bd359d3a-70a8-430d-813a-307fe28bf9dc", "start": "2026-10-15T10:22:00.662000+00:00", "wall": 1237, "discarded": 0, "agent": 337, "human": 899, "interventions": 10, "idle_minutes": 10}] -->
