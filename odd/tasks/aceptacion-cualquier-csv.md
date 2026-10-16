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
| T5a | S5 | inline | done | `d14d901`: the query summary counted the new non-AC row as a criterion ("19 criteria"); now only `AC-C<n>` rows count. |
| T6 | S3, S7 | inline | done | From the real download (L7): `Split-CsvLine` keeps each field as written, quoted or not; AC-G5 rows and the AC-G9 header are rebuilt field by field. Check (functions taken from the script's AST): download first row 22 fields → 21/22/22, header field 5 `IUCR_ALTERADA`; reference extract: rows and altered header byte-identical to the pre-change construction. |
| T7 | S2, S7 | inline | done | From the GREEN query run on the download (L12): AC-C1, AC-C6, AC-C8 and AC-C12 compared query CSV output with source lines byte for byte, which holds only when the source quotes like the query app. The PRD asks for values ("equivalente en valor", C4 "la misma cabecera"), so on another CSV rows are compared field by field (`CanonicalRow`, `CompareRowSets(..., byValue)`, `Test-SameCsvText`); byte for byte on the reference extract, unchanged. Check on that run's real outputs: the four row sets "header lines differ" by bytes, equal by values; reference outputs equal both ways; AC-C1 first download row equal by value, unequal by bytes, and a changed field is detected. |
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
- **L7** 07:28:40: portal download complete after three more 503/500 answers retried by curl:
  `work/portal-download/chicago-crime-portal.csv`, 2,047,271,622 bytes, 8,659,584 data rows (reference: 8,656,904),
  LF line ends. **The portal no longer quotes every field**: header `ID,Case Number,Date,...` and rows such as
  `13311263,JG503434,07/29/2022 03:39:00 AM,...`; only fields that need it (Location) are quoted. Its first row
  differs from the reference extract's (13311263 vs 14348063), so AC-A3 is tested on it (no fallback copy
  needed). Two generator-script assumptions broke: the T1 first-row regex required quotes (startup throw), and
  AC-G9's `.Replace('"IUCR"', ...)` would leave an unquoted header unchanged (the "altered" run would succeed
  over Run A). Fixed in T6. The query script only uses `Trim('"')` on source fields, so it reads both forms.
- **L8** AC-A1 first pass (07:11:36–07:23:06): generator acceptance on the reference extract with the T1 script
  (report commit `cd55943`): PASS 15/15, header "CSV: the reference extract (SHA-256 5cb5fa41…ae35d); the exact
  figures of the PRDs are required.", AC-G1 summary `8.656.904 registros válidos, 0 rechazados`. Query acceptance
  on `e9891f4`: PASS 59/59 (the "19 criteria" text led to T5a), `--dataset full --reference`, THEFT 1,838,502 =
  select = PRD figure, community area nil 613,720, `Full dataset: 0 rejected` PASS.
- **L9** 07:31: Rust checks (no Rust change in this feature): `cargo fmt --all --check` OK, `cargo clippy --workspace
  --all-targets -- -D warnings` clean, `cargo test --workspace` 425 passed, 0 failed (07:31:49–07:32:27).
- **L10** 07:31–07:34, agent mistake: the first acceptance chain wrote `-WorkDir "C:\\...\\work\\$3"` inside bash
  double quotes, which reached PowerShell as the literal `C:\rust\chicago-crime\work$3`, a folder in the repository
  root outside the ignored `work/`. Stopped the chain and its processes by PID (a first command-line filter also
  matched the agent's own shells; all of them were the agent's). Deleting the folder was denied twice by the tool
  permissions, so the run **waited for the user from 07:34:19 to 08:01:20** (not active time); the user deleted it.
  The new `work/chain.sh` uses forward-slash work folders (`C:/rust/chicago-crime/work/<name>`).
- **L11** 08:01:21: chain restarted: RED (scripts of `407d43d`, untracked copies) and GREEN on the portal download,
  then the final runs on the reference extract.
- **L12** chain results (08:01:23–08:32:45; reports in `work/<name>/`):
  - RED generator (`407d43d`) on the download, 08:01:24–08:06:45: **FAIL 5 of 15**.
  - RED query (`407d43d`) on the download, 08:06:45–08:11:13: **FAIL 9 of 58**.
  - GREEN generator (`7cc3a83`) on the download, 08:11:13–08:17:18: **PASS 15/15**.
  - GREEN query (`7cc3a83`) on the download, 08:17:18–08:21:27: **FAIL 5 of 59**: AC-C1, AC-C6, AC-C8, AC-C12 ×2,
    all "byte-identical"/"header lines differ" between the query app's quoted CSV and the unquoted source rows.
    Fixed in T7.
  - Final generator (`7cc3a83`) on the reference extract, 08:21:27–08:27:02: **PASS 15/15**.
  - Final query (`7cc3a83`) on the reference extract, 08:27:02–08:32:45: **PASS 59/59 (18 criteria)**.
  - The feature document was modified but uncommitted during the GREEN and final runs, so those reports say
    "7cc3a83 with uncommitted changes"; no script differed from `7cc3a83`.
