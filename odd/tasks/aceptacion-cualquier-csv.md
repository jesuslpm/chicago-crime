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
| T1 | S1–S5 | inline (no writer reason) | done | `ce58182`. Shared `reference-extract.ps1`; generator script. No runnable RED (no test runner for the scripts; the old script on the portal download is the RED, see T4). Checks: PowerShell parse 0 errors; on the reference extract's first row the rebuilt AC-G5 rows have 21/22/22 fields and the rebuilt row equals the original byte for byte, so AC-G5 gets the same CSV as before. Assess: medium, under_budget. |
| T2 | S1, S2, S4, S5 | inline | done | `cd55943`. Query script. Parse 0 errors; `Complete-Selects` runs before AC-C11/AC-C17 use the `theft` select. Assess: medium, under_budget. |
| T3 | S6 | inline | done | `9ea8828`. README (modes, PRD list, layout row, C:\ examples) and worktree guide. No `D:\` left in README, worktree guide or scripts; 0 broken README links. Assess: medium, under_budget. |
| T5 | S1, S2, S4 | inline | done | Fixes from the RDD review of `407d43d..9ea8828` (see L6): query script requires 0 rejected in both modes, both scripts fail on a malformed manifest fingerprint, `$TheftFigure`/`$NoCommunityAreaFigure` names, full dataset folder `full` (old `reference` folder cleaned), one source for the AC-G5 sample row, README anchor name. Parse 0 errors; `Test-Sha256` true for the reference hash, false for a prefixed or empty value. |
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
- **L5** 07:11: AC-A1 run on the reference extract started in the background on `9ea8828` (generator then query
  app, sequential). The portal download crawls: 91 MiB at 07:14, then about 0.5 MiB/min.
- **L6** RDD review, 07:13–07:15. Commits `407d43d..9ea8828` (PRD, T1, T2, T3), risk medium, lenses readability +
  reliability, lineage `review-5265ff0ca918d340`. Result: **approved**, acknowledged 07:15:37 (authority burned);
  reviewed boundary is now `9ea8828`. Findings, all advisory:
  - WARNING R3-query-zero-rejected: on another CSV the query script no longer required 0 rejected. **Fixed** (T5).
  - WARNING R3-fingerprint-unvalidated: a malformed manifest fingerprint would silently drop the exact figures.
    **Fixed** (T5): FAIL row when it is not 64 hex digits.
  - WARNING R3-unproved-acceptance: no recorded run yet. **Resolved by T4** (acceptance runs).
  - SUGGESTION R2-sentinel-theftcount: constants overwritten with -1. **Fixed** (T5): renamed to `*Figure`.
  - SUGGESTION R2-reference-folder-name: report said `--dataset reference` for another CSV. **Fixed** (T5): folder
    `full`, the old `reference` folder is cleaned.
  - SUGGESTION R2-two-sources-sample-row: AC-G5 rows read the first row twice. **Fixed** (T5).
  - SUGGESTION R2-stale-anchor: "Get the data" vs "Get the dataset". **Fixed** (T5).
  - SUGGESTION R2-odd-status-stale: tasks shown pending. **Resolved**: the document was updated right after the
    reviewed commits.
  - SUGGESTION R3-ac-g5-row-parse: unquoted empty case number in the first row. **Declined**: Case Number is
    mandatory (parent PRD field rules), so such a row is itself rejected and AC-G5's 999 valid rows would not
    hold anyway.
