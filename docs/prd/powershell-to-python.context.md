# Context for the PRD: PowerShell scripts to Python

Input for the PRD session that will specify replacing every PowerShell script of the repository with Python.
It is a feasibility analysis made on 2026-10-17, before any implementation. It is not a PRD and decides
nothing: the open product questions are listed in [Open decisions](#open-decisions-for-the-prd).

## What the user asked

> Ya que tenemos la dependencia de Python, me gustaría tener sólo scripts de python, no sólo es por
> homogenedidad y menos piezas, sino por portabilidad. Haz un analisis de si es posible hacelo en Python y que
> todo funcione igual, no implementes nada, sólo el análisis, si éste es positivo, ya lo haremos en otro cambio.

Standing constraints from the same conversation:

- Python 3.10 or later is already a project requirement (README, section *Requirements*); every Python test
  suite of the repository passes on 3.10 and 3.14.
- No global Python packages. When a script needs a package, it declares it with PEP 723 inline metadata, runs
  with `uv run` and locks its versions with `uv lock --script` (as `scripts/slides/notes_audio.py` does). The
  standard library is preferred.
- "Funcione igual": same behavior, same Spanish messages, same exit codes, same acceptance verdicts.

## Verdict

- **Windows (the reference platform): feasible with identical behavior, standard library only.** No uv
  packages are needed; the Windows-specific pieces are reachable through `ctypes`, `os`, `stat` and `winreg`.
- **Linux: feasible with documented differences, after product decisions.** Two PRDs say the scripts are
  Windows only, and a few behaviors cannot be reproduced identically on Linux.

## What exists today

| Family | Files | Lines | Automated tests |
|---|---|---|---|
| Acceptance, `scripts/acceptance/` | `run-acceptance.ps1` (684), `run-query-acceptance.ps1` (1,327), `send-ctrl-c.ps1` (129), `reference-extract.ps1` (28) | 2,168, about 195 of them embedded C# | None. The execution document says "Test runner: none for the PowerShell acceptance scripts (they are the functional tests)" (`odd/tasks/aceptacion-cualquier-csv.md:5-6`) |
| Worktree tooling, `scripts/worktree/` | `WorktreeCommon.psm1` (556), `New-Worktree.ps1`, `Get-Worktrees.ps1`, `Merge-Worktree.ps1`, `Remove-Worktree.ps1`, `New-RehearsalBase.ps1`, `Remove-Rehearsal.ps1` | 1,369 | `tests/Test-Worktree.ps1`, 911 lines, 42 cases |

The rest of the Python already in the repository needs only the standard library: the worktree guard hook
(`.claude/hooks/worktree_guard.py`), the `closing-report` and `prd` skill scripts, and their tests.

## Acceptance scripts

### What they do

- `run-acceptance.ps1` (generator): parameters `-WorkDir`, `-Csv`, `-KillTimeoutSeconds 600`,
  `-CtrlCAfterSeconds 25`, `-SkipBuild` (lines 56-63). It checks the CSV shape and free space, builds with
  `cargo build --release`, runs two generations, the independent verifier and the Ctrl+C, kill-and-recover
  and bad-row scenarios, and produces 15 checks (AC-G1…G10, manifest, AC-C18, parent AC9/AC13). It compares
  the generator's output with the exact Spanish texts kept in `$Messages` (lines 86-103).
- `run-query-acceptance.ps1` (query app): parameters `-WorkDir`, `-Csv`, `-SliceRows 1000000`,
  `-CtrlCTimeoutSeconds 300`, `-SkipBuild` (lines 67-74). It produces 59 checks for AC-C1…C18, running 16
  `crime-acceptance select` processes in parallel (lines 865-883).
- `send-ctrl-c.ps1`: started by both scripts in a hidden console of its own; starts the program there, waits,
  sends a real Ctrl+C and writes the outcome as JSON.
- `reference-extract.ps1`: the reference extract's SHA-256 and the CSV-mode line of the reports.

Outputs: `PASS|FAIL  <criterion>  <evidence>` console lines, a Markdown report (`acceptance-report.md`,
`query-acceptance-report.md`, UTF-8 without BOM, LF), exit code 0 when every check passes and 1 otherwise.

### Windows-specific pieces and their Python equivalent

| Piece | Today | Python | Same on Windows | On Linux |
|---|---|---|---|---|
| Real Ctrl+C | C# `GenerateConsoleCtrlEvent` and `SetConsoleCtrlHandler` (`send-ctrl-c.ps1:38-40`), new hidden console | `ctypes` on kernel32, helper started with `CREATE_NEW_CONSOLE` hidden | Yes | SIGINT to the process group (`start_new_session` + `os.killpg`), which is what a terminal does |
| Peak memory | `.NET PeakPagedMemorySize64` sampled every 50 ms (`run-acceptance.ps1:228`); psapi `PeakPagefileUsage` every 10 ms plus a read after exit | `ctypes` `GetProcessMemoryInfo` on a kept process handle (psutil cannot read it after exit) | Yes | Peak resident memory (`VmHWM`, or `ru_maxrss` from `os.wait4`). **Not the same metric** |
| Reading files the query app renames or deletes | `FileShare ReadWrite|Delete` | `ctypes` `CreateFileW` with every share flag. Plain `open()` lacks FILE_SHARE_DELETE and could break the app | Yes | `os.stat` / plain reads |
| Hard kill | `Stop-Process -Force` | `TerminateProcess(-1)` through `ctypes` (`Popen.kill` gives exit code 1, shown only in evidence) | Yes | SIGKILL |
| CSV, JSON, hashes | `TextFieldParser`, `ConvertFrom-Csv`, `ConvertFrom-Json`, `Get-FileHash` | `csv`, `json`, `hashlib` | Yes | Same |
| Machine information in the report header | `Get-CimInstance` | `winreg`, `GlobalMemoryStatusEx` | Header wording may differ | `/proc/cpuinfo`, `os-release` |

### Parity traps

- PowerShell's `-eq`, `-like`, `-match`, `-contains` and hashtable keys ignore case; a literal port becomes
  silently stricter.
- Process output must be read as bytes: `subprocess` text mode turns CRLF into LF and breaks the byte-exact
  comparisons of AC-C14.
- A Python sampler thread is slower than the PowerShell one and could miss the short-lived temporary file of
  AC-C10/AC-C15 (estimate, not measured).
- Scanning the 2.4 GB CSV in Python may be slower (estimate: tens of seconds).

## Worktree tooling

### What it does

- `WorktreeCommon.psm1` derives every name from `<n>`, keeps the verbatim Spanish messages of the PRD
  (lines 42-83, for example `InUse` at line 59), prints everything on stdout and exits 0, 3 or 1.
- `New-Worktree.ps1` refuses before creating anything (name, base, path length, existing branch, folder or
  Engram project, `engram` and `code` on PATH, CSV on the same drive), then runs `git worktree add`, records
  `branch.feat/<n>.worktreebase`, hard-links the CSV and marks it read-only, copies the private files, creates
  `work`, runs `engram init` and opens VS Code.
- `Get-Worktrees.ps1` lists the copies; `Merge-Worktree.ps1` fast-forwards the base, with exit 3 when the
  base moved and a compare-and-swap `update-ref`; `Remove-Worktree.ps1` removes or discards (tag `exp/<n>`,
  Engram project, Claude Code memory) and stops on a file in use; `New-RehearsalBase.ps1` and
  `Remove-Rehearsal.ps1` handle the rehearsal base.
- The guard hook (`worktree_guard.py`) names no script, but relies on `branch.<b>.worktreebase` and on the
  `.git` file layout, which a port must keep.

### Windows-specific pieces and their Python equivalent

| Piece | Python | Same on Windows | On Linux |
|---|---|---|---|
| Hard link of the CSV | `os.link` | Yes | Works on the same file system |
| Same-drive check | compare `os.stat().st_dev` | Yes (stricter) | Needed: a drive-letter check always sees `/` |
| Read-only attribute | `os.chmod` (CPython maps it to FILE_ATTRIBUTE_READONLY) | Yes | Mode bits per inode, shared by every link; **root ignores them, so AC-W4 fails under root** |
| "En uso" stop when a file is open | catch `OSError` with WinError 32 | Yes | **Cannot be reproduced**: Linux deletes open files and folders |
| Reparse points skipped when deleting | `st_file_attributes` (`isjunction` needs 3.12) | Yes | `is_symlink()` |
| 259-character path limit | arithmetic | Yes | Harmless but meaningless |
| Case-insensitive paths | `os.path.normcase` | Yes | Case-sensitive, which is correct there |
| `code -n` to open the copy | `shutil.which` + `subprocess` | Yes | **No `code` on a headless machine**: creation is refused |
| Default root `C:\wt`, Claude memory under `%USERPROFILE%` | `Path.home()` | Yes | **`C:\wt` means nothing on Linux** |

### Parity traps

- Every user message goes to stdout, and the skill and the tests read stdout.
- `argparse` exits 2 on bad arguments; the scripts exit 1.
- `%TEMP%` appears in 8.3 form here (`JESUS~1.LOP`) while git prints long paths: test fixtures need a long-path
  temporary root.
- Removal must keep deleting without following links, restore the read-only attribute in a `finally`, and
  never delete a folder that is not one of this repository's copies.

## Open decisions for the PRD

1. **Scope:** Windows parity only (a refactor with no product change, which `CLAUDE.md` lets run without a
   PRD), or Windows and Linux (a product change).
2. **Acceptance on Linux:** accept peak resident memory as the memory metric for AC-G2 (103.5 MiB was measured
   in WSL, `docs/linux/native.md:74`) and AC-C13 (never measured on Linux), and SIGINT to the process group as
   the Ctrl+C.
3. **Worktrees on Linux:** the default root instead of `C:\wt`; whether VS Code is optional; what replaces the
   "en uso" stop; whether running as root is excluded, since read-only does not protect the CSV then.
4. **PRD texts to revise:** `docs/prd/worktrees/worktrees.prd.md` line 214 ("las herramientas son solo para
   Windows") and W10 ("Solo Windows", line 288); the soporte-linux PRD and `README.md:97`, which say the
   acceptance scripts are Windows only.
5. **Command contract:** the `worktree` skill routes Spanish phrases to `powershell … -File …ps1`; the new
   command line and the skill's routing table.
6. **Order and coexistence:** keep the `.ps1` until the Python version has proven parity, then delete them.

## How to prove "funciona igual"

- **Worktrees:** port the 42 cases of `Test-Worktree.ps1` to `unittest` (temporary git repositories, the real
  Engram CLI on a throwaway database, a "nothing changed" snapshot on every refusal), plus the guard tests.
- **Acceptance:** unit tests for the pure helpers, small runnable tests for the Ctrl+C helper and the memory
  probe, and side-by-side runs of the old and the new scripts on the reference extract and on a portal
  download, comparing normalized reports (times, MiB and sample counts masked): the same criteria with the
  same PASS/FAIL. A replay of an old commit should give the same RED rows.

## Files that would change

`.claude/skills/worktree/SKILL.md`, `CLAUDE.md` (worktree section and the acceptance work-folder example),
`README.md` (*Verify*, *Work on several changes at once*, layout table), `docs/worktrees.md`,
`docs/linux/native.md`, `docs/methodology/prompts-worktrees.md`, `.claude/hooks/test_worktree_guard.py` (merge
command strings), the two PRDs above and their English translations. The execution documents in `odd/tasks/`
are history and stay as they are.

## Size, time and risk (estimates of the analysis, not measurements)

| Port | Python size | Time | Risk |
|---|---|---|---|
| Worktrees | 1,000–1,200 lines of code and about 850 of tests | 2–3.5 h (the original build took 1 h 29 min) | High: it deletes branches, tags, folders, Engram projects and Claude Code memory, and changes the attribute of the 2.3 GB CSV |
| Acceptance | 2,500–3,000 lines including new tests | About a day, plus 8–10 acceptance runs of 6–15 min | Medium-high: case sensitivity, Ctrl+C, share modes, sampler timing, speed on 2.4 GB |

Past runs in this repository took about half of their estimate.

## Suggested order

1. Worktree tooling in Python with Windows parity (no product change), proven by the ported tests.
2. Acceptance in Python with Windows parity, proven by side-by-side runs.
3. PRD revisions for Linux with the decisions above, then enabling Linux.
