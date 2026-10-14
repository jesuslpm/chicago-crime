# soporte-linux — ODD feature document

- **Locator**: `odd/tasks/soporte-linux.md` · Engram mirror topic `odd/soporte-linux/tasks`
- **Branch**: `feat/soporte-linux` from `main` at `98c5b4d`. Delivery: local work-unit commits, no PRs, merge into
  `main` after full verification (user rule). Test runner: `cargo test --workspace` on Windows (baseline 424
  green) and on Ubuntu 24.04 / WSL2 in `~/chicago-crime` (baseline 407 green, 13 red).
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
  descartan" with `Se han descartado restos de una generación interrumpida anterior.`; those of a run still
  running "Se dejan intactos".
- **S4 — Documentation (AC-L7).** README: "un apartado nuevo de un par de párrafos que resume cómo compilar y
  pasar las pruebas en Linux. Distingue entre WSL y Linux normal, y enlaza con las guías detalladas." WSL
  guide: preparing the distribution, toolchain, code "dentro del sistema de archivos de Linux y no en el disco
  de Windows, explicando por qué", copying the reference CSV "que no está en el repositorio", build, tests,
  running both apps. Native guide: the same without the WSL part. Both say what does not work yet (the
  acceptance scripts) and how memory is measured (L3). PRD annotations: the generator and query PRDs' "fuera
  de alcance" Linux lines, and the parent's memory measurement.
- **S5 — Tests on both systems (AC-L1, AC-L2).** Linux: "pasan todas las que se compilan en Linux, sin ningún
  fallo". Windows: "pasan todas, y ninguna de las 424 que pasaban antes del cambio desaparece ni deja de
  pasar". "Cualquier cambio de comportamiento en Windows" is out of scope.

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
| T1 | S1, S5 | inline (no writer reason) | pending | |
| T2 | S2, S5 | inline | pending | |
| T3 | S3, S5 | inline | pending | |
| T4 | S4 | inline | pending | |
| T5 | S1–S5 | inline checks + independent verifier (T1 touches a concurrency guard, high risk item 4) | pending | |

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
