# soporte-linux — ODD feature document

- **Locator**: `odd/tasks/soporte-linux.md` · Engram mirror topic `odd/soporte-linux/tasks`
- **Branch**: `feat/soporte-linux` from `main` at `98c5b4d`. Delivery: local work-unit commits, no PRs, merge into
  `main` after full verification (user rule). Test runner: `cargo test --workspace` on Windows (baseline 423
  green: 424 tests in source, one of them non-Windows only) and on Ubuntu 24.04 / WSL2 in `~/chicago-crime` (baseline 407 green, 13 red).
- **Timing**: estimate **1.5–2.5 h** (calibration: crime-query, ~11.6k lines in ~3 h 15 min; this change is
  far smaller but needs a full-CSV run on Linux). Real start **2026-10-14 07:27:41 +0100**. Waits: none.

## Specs

Product source: `docs/prd/chicago-crime/soporte-linux.prd.md` **v1.0** (L1–L3, AC-L1…AC-L9), parent
`docs/prd/chicago-crime/chicago-crime.prd.md` v1.2. The PRD wins on any product question.

- **S1 — Output folder on Linux like on Windows (L2, AC-L3…AC-L6).** "en Linux, el generador no deja en la
  carpeta de salida nada que no dejaría en Windows". Successful run: "solo quedan el conjunto publicado y lo
  que indica cuál es el actual". Failed or cancelled run that created the folder: "La carpeta se borra
  entera". Foreign content: "Se rechaza sin crear, cambiar ni borrar nada en ella". Two runs at once: "La
  segunda aborta sin tocar nada" with `Otra generación está usando la carpeta de salida <carpeta>.` and "la
  primera termina con normalidad".
- **S2 — Peak memory on Linux (L3, AC-L8).** "el pico se mide como la memoria residente máxima del proceso,
  que el propio sistema guarda desde que el proceso arranca … El resumen y el aviso usan el mismo texto que
  en Windows." The summary shows the peak "nunca \"no disponible\""; over the limit, once:
  `Aviso: el pico de memoria privada comprometida (<cifra> MiB) supera el límite de <límite> MiB.` AC-L8:
  "Dado el CSV de referencia completo en Linux, cuando se ejecuta el generador, entonces el pico de memoria
  medido no supera 512 MiB y el resumen final lo muestra."
- **S3 — Leftover sort folders on Linux (AC-L9).** Leftovers of a run that is no longer running "Se
  descartan" with `Se han descartado carpetas temporales de una generación interrumpida anterior.` (the
  existing sort-folder message; corrected from the output-folder one, see L6); those of a run still
  running "Se dejan intactos".
- **S4 — Documentation (AC-L7).** README: "un apartado nuevo de un par de párrafos que resume cómo compilar y
  pasar las pruebas en Linux. Distingue entre WSL y Linux normal, y enlaza con las guías detalladas." WSL
  guide: preparing the distribution, toolchain, code "dentro del sistema de archivos de Linux y no en el disco
  de Windows, explicando por qué", copying the reference CSV "que no está en el repositorio", build, tests,
  running both apps. Native guide: the same without the WSL part. Both say what does not work yet (the
  acceptance scripts) and how memory is measured (L3). PRD annotations: the generator and query PRDs' "fuera
  de alcance" Linux lines, and the parent's memory measurement.
- **S5 — Tests on both systems (AC-L1, AC-L2).** Linux: "pasan todas las que se compilan en Linux, sin ningún
  fallo". Windows: "pasan todas, y ninguna de las 423 que pasaban antes del cambio desaparece ni deja de
  pasar" (corrected from 424, see L5). "Cualquier cambio de comportamiento en Windows" is out of scope.

## Implementation decisions (defaults chosen without asking, per the user's standing rule)

- **Lock (S1)**: on Unix the generator locks the output folder itself (an advisory `flock` on the directory)
  instead of a lock file, so nothing is created inside it. Removing a lock file before releasing it is racy
  (a waiter can end up holding a lock on an unlinked file). Windows keeps its delete-on-close lock file. A
  `.generator.lock` left by an older Linux build is still accepted as the generator's own.
- **Memory (S2)**: Linux reads `VmHWM` from `/proc/self/status` (kernel-kept peak resident set, in kB); no new
  dependency. Other Unix systems still report "no disponible".
- **Process liveness (S3)**: Linux reads `/proc/<pid>/stat`; no entry or a zombie/dead state means ended;
  any other doubt means alive (the module's existing rule). The three Windows-only stale-folder tests become
  cross-platform with per-system helpers.
- **Docs (S4)**: English, like the README: `docs/linux/wsl.md` and `docs/linux/native.md`.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1, S5 | inline (no writer reason) | done | `6cc19f8`. RED: the 13 baseline failures on Linux; GREEN: generator 167/167 on Windows and all green on Linux. Review: high (escalated, item 4), lenses risk+resilience, approved, acknowledged. |
| T2 | S2, S5 | inline | done | `9e64311`. RED: `the_system_reports_a_peak_that_includes_committed_allocations` failed on Linux (`None`); GREEN after VmHWM; `binary.rs` summary and warning assertions widened to Linux (both went red, then green). Reviewed with T3. |
| T3 | S3, S5 | inline | done | `5fcdd8b`. RED: `this_process_is_alive_and_an_ended_one_is_not` and `a_run_discards_the_sort_folders_of_a_killed_generator_and_says_so` failed on Linux; GREEN via `/proc/<pid>/stat`. T2+T3 review: high (escalated, item 4), lenses risk+resilience+reliability, approved with warnings (PID namespace, hidden `/proc`, PID reuse in tests), acknowledged. |
| T3a | S3 | inline | done | `29b57c2`, from the T2+T3 review warnings: a Unix sort holds an advisory `flock` on its folder; held folders are never discarded; Linux ended-process helper keeps a zombie so its PID is not reused. RED: the two `folder_lock` tests; GREEN. Review: high (escalated), approved; 2 of 3 warnings disproved (module is `cfg(any(windows, linux))`; zombie-as-ended is asserted), 1 real (creation window) → T3b. |
| T3b | S3 | inline | done | `0b13f7c`: a sort that cannot hold its new folder (taken by a cleaner in the creation window) starts another one (max 10). Unit test of `FolderHold::take`. Review approved; warning: the retry loop itself has no test (needs a production test hook; accepted, see Closure). |
| T3c | S5 | inline | done | `0d138bc`: test-only isolation. 40/200 parallel runs of `stale_sort_dirs` failed (0/100 serial): a child between `fork` and `exec` shares the test process's lock. Process starts and lock-release tests now take one mutex: 0/300 failures. |
| T4 | S4 | inline | done | `dfb17f9`: README section (two paragraphs) + `docs/linux/wsl.md` + `docs/linux/native.md`; PRD annotations (generator ES/EN, query, parent ES/EN). Link check: 0 bad links. Assess: passive. |
| T1a | S1 | inline | done | `924f3e9`, from verifier advisory 1: on Unix a `.generator.lock` left by an older build is discarded as a leftover. RED: `a_lock_file_left_by_an_older_unix_build_is_discarded_as_a_leftover` (0 leftover messages); GREEN. Review: high (escalated, item 1), approved; warning (mixed old/new builds side by side) accepted: the two builds lock different objects, so they never excluded each other anyway. |
| T6 | S4 | inline | done | `6401572` (guides: one test can skip as root, verifier advisory 2) and `f2d8e5f` (readability suggestions of the close-of-feature review). Reviews approved, no findings on the last one. |
| T5 | S1–S5 | inline checks + independent verifier | done | Acceptance table below. Independent verifier on `0d138bc`: S1–S5 pass; 1 blocker (wrong quoted message in PRD/S3, docs only), already fixed in `0c0aa8f`; Windows baseline 423 confirmed by an actual run, no baseline test lost. Final: Windows 425/425, Linux 429/429, clippy clean on both. |

## Acceptance results (reference CSV, Ubuntu 24.04 / WSL 2, 2026-10-14 07:52–08:00)

Script: scratchpad `linux-acceptance.sh` (not committed), commit `0d138bc`, Rust 1.99, kernel
5.15.133.1-microsoft-standard-WSL2.

| Criterion | Observed |
|---|---|
| AC-L1 | `cargo test --workspace`: 428 passed, 0 failed (Linux baseline 420 ran: 407 + 13 failing). |
| AC-L2 | Windows `cargo test --workspace`: 425 passed, 0 failed (baseline 423, see L5). |
| AC-L8 | Full CSV: exit 0, `Generación completada: 8.656.904 registros válidos, 0 rechazados`, `Pico de memoria privada comprometida: 103,5 MiB (límite: 512 MiB).`, 1 min 32.2 s. |
| AC-L3 | Output folder after the run: `CURRENT`, `dataset-20261014-075249-2444`; temp folder empty. |
| Verifier | `crime-acceptance --reference` on the Linux dataset: `RESULT: PASS (18 checks)`. |
| AC-L6 | Second run while the first runs: exit 1, `Otra generación está usando la carpeta de salida /home/jesuslpm/acc/ds.` then `Generación cancelada. No se ha modificado el conjunto existente.`; first run exit 0, completed; folder holds `CURRENT` + one version. |
| AC-L4 | SIGINT 3 s into a run that created its folder: exit 130, folder gone, `Generación cancelada por el operador. No se ha modificado el conjunto existente.`, temp folder empty. |
| AC-L9 | `kill -9` left `crime-generator-sort-13141-oBRZ2u`; with a fake folder of a live `sleep` (`crime-generator-sort-13332-l1v3ab`), the next run logged `Se han descartado carpetas temporales de una generación interrumpida anterior.` once, removed the dead one and kept the live one. |
| AC-L5 | Covered by `foreign_files_in_the_output_folder_are_refused_and_left_untouched` and `folders_that_only_look_like_versions_are_refused_and_never_deleted` (both red at baseline on Linux, green now). |
| AC-L7 | README section + guides; link check 0 bad links. |

## Log

- **L1 (2026-10-14, user, verbatim)**: "Bien pues hagamos un nuevo cambio empezado por el PRD. El objetivo es
  que pasen los tests en Linux, modificar README con un apartado que diga como compilar y pasar los tests en
  WSL y en un Linux \"normal\", el apartado tiene que estar resumido en un par de párrafos con enlaces a los
  detalles que se ponen en otros MD en docs"
- **L2 (2026-10-14, user, verbatim)**: "Vale, los metemos en este cambio" — peak memory and leftover cleanup on
  Linux join the scope; PRD updated (L3, AC-L8, AC-L9).
- **L3**: baseline on WSL (Ubuntu 24.04, rustup 1.99): clippy clean; 407 passed, 13 failed, all caused by
  `.generator.lock` staying in the output folder (Windows deletes it on close). Reference CSV copied to
  `~/data/chicago-crime-data.csv`.
- **L4 (07:27–07:50)**: T1–T4 and T3a–T3c committed as listed in `## Tasks`. The earlier "Linux generator
  171 green" after T3b was a lucky run: the T3c flake hit 1 run in 5.
- **L5 (07:55)**: correction: "424" was a count of `#[test]` in source. At `98c5b4d` one test
  (`the_peak_is_not_available_elsewhere`) is `cfg(not(windows))`, so Windows ran 423. Candidate Windows run:
  425 passed, 0 failed (423 + the `VmHWM` and `/proc` stat parser tests). PRD AC-L2 and S5 corrected.
- **L6 (08:00)**: Linux acceptance passed (table above). Correction: S3 and the PRD quoted the output-folder
  message for sort-folder leftovers; the existing sort-folder message is `Se han descartado carpetas
  temporales de una generación interrumpida anterior.` (unchanged code, Windows included).
- **L7 (08:13)**: verifier advisories: (1) old lock file never removed on Unix → T1a; (2) "a few tests" skip as
  root, really one → T6; (3) clone URL unverifiable → disproved, `origin` is
  `https://github.com/jesuslpm/chicago-crime.git` (the verifier's clone had no remote); (4) two runs in the
  same second collide on the default log name → baseline design, out of scope, not changed.

## Closure

- **Outcome**: every S# met. AC-L1 Linux 429 passed / 0 failed; AC-L2 Windows 425 passed / 0 failed, all 423
  baseline tests present and green; AC-L3–AC-L9 observed on the reference CSV (table above); AC-L7 README
  section and two guides.
- **Reviews**: T1 (high), T2+T3 (high), T3a (high), T3b, T1a (high), the close-of-feature slice (T3c, T4,
  T1a, T6) and `f2d8e5f`: all approved and acknowledged; consent granted per the user's standing rule.
- **Accepted and open items**: the retry loop of T3b has no test (it needs a production test hook to force
  the race); mixed old/new Unix builds on one output folder do not exclude each other; back-to-back runs in
  the same second collide on the default log name (baseline behavior); the acceptance PowerShell scripts
  stay Windows-only (out of scope).
- **Timing**: estimate 1.5–2.5 h; real 2026-10-14 07:27:41 → 08:15 (about 48 min), no waits for the user.
  Faster than estimated: the code changes were small, reviews and the full-CSV acceptance ran in the
  background while other tasks went on.
- **Change report** (`git diff --numstat 98c5b4d..HEAD`, before this closing edit; no generated files or
  lockfiles):

| Area | File | + | − |
|---|---|---:|---:|
| `crates/crime-generator` | `src/memory.rs` | 44 | 10 |
| | `src/process.rs` | 41 | 3 |
| | `src/publish.rs` | 20 | 17 |
| | `src/sort.rs` | 97 | 7 |
| | `tests/binary.rs` | 2 | 2 |
| | `tests/publish.rs` | 24 | 0 |
| | `tests/stale_sort_dirs.rs` | 159 | 13 |
| | **subtotal** | **387** | **52** |
| `docs/linux` | `wsl.md`, `native.md` | 182 | 0 |
| `docs/prd` | `soporte-linux.prd.md` + 5 annotated PRDs | 157 | 5 |
| `README.md` | | 23 | 4 |
| `odd` | `tasks/soporte-linux.md` | 104 | 0 |
| **total** | | **853** | **61** |

- **Summary**: the project now builds and passes every test on Linux as well as on Windows. On Unix the
  generator locks the output folder itself instead of leaving a lock file in it, so the output folder ends
  exactly as on Windows after a successful, failed or cancelled run. It reports its peak memory from the
  kernel-kept resident peak (103.5 MiB on the reference CSV). It discards the sort folders of ended runs,
  telling them from running ones by `/proc` and by an advisory lock every running sort holds on its folder,
  so runs in other containers are safe too. The README gains a two-paragraph Linux section that links a WSL
  guide and a native Linux guide.
