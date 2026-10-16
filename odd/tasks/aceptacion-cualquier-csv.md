# aceptacion-cualquier-csv — ODD feature document

- **Locator**: `odd/tasks/aceptacion-cualquier-csv.md` · Engram mirror topic `odd/aceptacion-cualquier-csv/tasks`
- **Branch**: `fix/acceptance-any-csv` from `main` at `407d43d` (PRD commit `4bf2e71`). Delivery: local work-unit
  commits, no PRs, merge into `main` after full verification. Test runner: none for the PowerShell acceptance
  scripts (they are the functional tests); checks are a PowerShell parse of both scripts, the acceptance runs
  themselves and `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (no Rust change expected).
- **Timing**: estimate **1–2 h** (calibration: soporte-linux 48 min vs 1.5–2.5 h, worktree-tooling 1 h 29 min
  vs 2–3.5 h). Real start **2026-10-16 07:05:01 +0100**. Waits: see Log.

## Specs

Product source: `docs/prd/chicago-crime/aceptacion-cualquier-csv.prd.md` **v1.0** (A1–A5, AC-A1…AC-A4), parent
`docs/prd/chicago-crime/chicago-crime.prd.md` v1.2. The PRD wins on any product question.

- **S1 — Reference extract (A1, AC-A1).** "la verificación compara la huella del CSV que guarda el manifiesto
  con la del extracto de referencia que publica el README". "Si coinciden, la verificación hace las mismas
  comprobaciones que hoy, incluidas las cifras exactas y el verificador independiente con las cifras de
  referencia."
- **S2 — Another CSV (A2, AC-A2).** "Si no coinciden, cada cifra esperada sale del propio CSV: registros
  válidos: las filas de datos del CSV, con 0 rechazados; `THEFT` y área comunitaria "sin valor": el filtrado
  directo del CSV; el verificador independiente se ejecuta sin pedir las cifras de referencia."
- **S3 — AC-G5 test CSVs (AC-A3).** "Los CSV de prueba de AC-G5 se construyen a partir de la primera fila del
  CSV recibido, sea cual sea."
- **S4 — Skipped checks (A3).** "Toda comprobación tiene que pasar, igual que hoy: una comprobación omitida por
  falta de datos cuenta como fallo".
- **S5 — Report header.** Reference: `CSV: the reference extract (SHA-256 <huella>); the exact figures of the
  PRDs are required.` Another CSV: `CSV: not the reference extract (SHA-256 <huella>); the expected figures are
  counted on this CSV.`
- **S6 — Documentation (A4, AC-A4).** README: "dice que las dos verificaciones funcionan con el extracto de
  referencia y con cualquier descarga posterior, explica los dos modos y cómo se reconoce el extracto de
  referencia". "todos los ejemplos con carpetas en `D:\` pasan a `C:\` ... en el README, en los ejemplos de
  ayuda de los dos scripts y en la guía de worktrees".
- **S7 — Test CSV (A5).** AC-A2 and AC-A3 use "una descarga real del portal, hecha durante la verificación de
  este cambio y que no se guarda en el repositorio". "Si su primera fila coincide con la del extracto de
  referencia, AC-A3 se prueba con una copia del extracto con otra fila en primer lugar, generada en el
  momento."

## Implementation decisions (defaults chosen without asking, per the user's standing rule)

- **Shared reference identity**: a new dot-sourced `scripts/acceptance/reference-extract.ps1` holds the
  reference SHA-256 and the S5 header line, so both scripts share one copy.
- **Generator AC-G1 with another CSV**: the expected summary is built from the manifest's valid count with
  `0 rechazados`; the verifier's AC-G1 (always run) proves that count equals the CSV data rows minus rejected.
- **Generator fingerprint when Run A fails**: no manifest exists, so the script hashes the CSV itself; the run
  fails anyway, the hash only feeds the header.
- **Query app with another CSV**: no fixed figure (`-1`, which `Test-Count` already treats as "none"); AC-C11
  and AC-C17 compare with the `theft` select instead.
- **Folder names**: the query script keeps its `reference` dataset folder name, so the cleanup of earlier runs
  still finds it.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1–S5 | inline (no writer reason) | pending | generator script + shared file |
| T2 | S1, S2, S4, S5 | inline | pending | query script |
| T3 | S6 | inline | pending | README, script help, worktree guide |
| T4 | S1–S7 | inline checks; acceptance runs in the background | pending | AC-A1 on the reference extract, AC-A2/AC-A3 on the portal download, last-slice review, closing report |

## Log

- **L1** (2026-10-16, user's original request, verbatim): "En chicago-crime.prd.md hay halgunos criterios de
  aceptación como: Dado el índice de tipo principal, cuando se cuenta THEFT, entonces el resultado es
  1.838.502, igual que en el CSV. AC5 — Dado el índice de área comunitaria, cuando se cuentan los registros
  "sin valor", entonces el resultado es 613.720. AC5 — Dado el índice de área comunitaria, cuando se cuentan
  los registros "sin valor", entonces el resultado es 613.720. Esos número salen de csv de referencia. Si te
  descargas un csv más actualizado los números cambia y los scripts de aceptación en scripts/acceptance
  fallarán si usan esos números."
- **L2** (2026-10-16, verbatim): "Un PRD de cambio, por cierto también quiero que modifiques el README y use
  C:\ en vez de D:\"
- **L3** (2026-10-16, verbatim): "Sí, ejecuta  el ciclo ODD a partir del PRD aceptacion-cualquier-csv.prd.md"
  — then "Arranca" at 07:05:01.
- **L4** 07:05:18: the first portal download returned `{"message":"Service unavailable",...}` (77 bytes, not a
  CSV). Retried in the background with `curl -f --retry 10 --retry-delay 60 --retry-all-errors`; work goes on
  meanwhile (not a user wait).
