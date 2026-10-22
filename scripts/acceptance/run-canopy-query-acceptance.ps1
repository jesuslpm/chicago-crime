<#
.SYNOPSIS
    Acceptance of the CanopyDB crime query app (crime-canopy-query) on any compatible CSV (PRD
    canopy-db-consultor, decision KC8): AC-KC1 to AC-KC7 and the module criteria AC-M2 and AC-M5 to
    AC-M8.

.DESCRIPTION
    Every check of run-query-acceptance.ps1 (AC-C1 to AC-C18, AC-S1 to AC-S12 and the batch rows)
    against the CanopyDB query app, with the same figures and labels (AC-KC1). It is that script,
    started with -CanopyDb: it builds and generates with crime-canopy-generator, queries with
    crime-canopy-query and verifies with crime-canopy-acceptance instead of crime-acceptance, whose
    `select` stays the independent filtering of the CSV. The differences:

      AC-C13     becomes AC-M8: the peaks of F3, F4 and the whole fixed query set in a batch, with
                 the CanopyDB query app (AC-M8) and with the original one on an SSTable dataset of
                 the same CSV (AC-M8 (SSTables)); the largest at most 3 times the smallest;
      AC-C18     its second half checks that an SSTable dataset of schema 2 is rejected with
                 "No se encontró un conjunto de datos válido en <carpeta>." and 1;
      engine     a damaged base reads "No se pudo leer el conjunto de datos en <carpeta>." after one
      errors     "Detalle del error de CanopyDB: <texto>" line (KC5);
      verifier   crime-canopy-acceptance runs when no query runs, since CanopyDB lets one process
                 open a base at a time.

    It adds the rows of the PRD's own criteria:

      AC-M2-F1..F11  each query of the fixed set (canopy-fixed-queries.txt, KC7) alone with both
                     query apps: the same number of results; the same records in F1, F2 and F11;
                     F10 1,000 records, all from F11;
      AC-M5      a CanopyDB batch of more than 45 s holds the base: a second query warns and is
                 rejected 30 to 35 s after the warning, Ctrl+C during the wait ends with 130 in
                 less than 1 s, and with a batch shorter than 30 s the second query answers;
      AC-M6      the CanopyDB output folder, copied right after its generation, against itself
                 after every query: the same names, the same manifest and `--compare` equal;
      AC-M7      the fixed set in a batch gives what it gives alone; a batch of the fixed set run
                 twice, held inside its first F10 while the CanopyDB generator publishes a new
                 version, keeps the version it opened;
      AC-KC2..5  edited copies of small datasets: other schemas, other manifest shapes, another
                 CanopyDB version, a missing base or file, read-only files and a damaged base;
      AC-KC6     the JSON's dataset.schema_version is the text "canopydb-2";
      AC-KC7     the NOT MET rule below, with made-up figures, and invalid associations.

    The work folder entries are named canopy-*, so it can share -WorkDir with the other acceptance
    scripts. It writes canopy-query-acceptance-report.md and exits with 0 when the overall result is
    PASS and 1 when it is FAIL.

.PARAMETER WorkDir
    Folder for the datasets, the inputs, the outputs and the report. It needs about 16 GB free on the
    reference extract: two CanopyDB bases of the full CSV (the dataset and its AC-M6 copy, 2.6 GB
    each), the SSTable dataset (1.1 GB), the generator's sort folder (under 3 GB), the listings of
    F11 (about 0.6 GB each, removed once compared) and the small datasets.

.PARAMETER Csv
    The CSV: the reference extract or any later download. By default, chicago-crime-data.csv at
    the repository root.

.PARAMETER SliceRows
    Rows of each of the two CSV slices behind the versions of AC-C12, AC-B2 and AC-M7.

.PARAMETER CtrlCTimeoutSeconds
    How long AC-C10's Ctrl+C waits for the query's output file to reach 1 MiB.

.PARAMETER SkipBuild
    Use the release binaries already built.

.PARAMETER NotMetByCanopyDb
    Associations <criterion>=<finding> (decisions KC8 and KG10). Only AC-C7, AC-S8 and AC-M8 can be
    associated, each once, with a finding that has its section ("## <finding> — ...") in
    docs/canopy-db/hallazgos.md. An associated row reads NOT MET instead of FAIL, and does not count
    as a failure, when every run of it ended with 0 and the expected result and the only failure is
    its measured figure, of the CanopyDB query app: the time of AC-C7 or the memory ratio of AC-S8
    and AC-M8. AC-M8 (SSTables) is never NOT MET. Items are also split at commas, so a list passed
    through -File as one string reads the same. An invalid association ends the script with 1
    before anything runs.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-canopy-query-acceptance.ps1 -WorkDir work\acceptance

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-canopy-query-acceptance.ps1 -WorkDir work\acceptance -NotMetByCanopyDb AC-M8=H5
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [string]$Csv,
    [int]$SliceRows = 1000000,
    [int]$CtrlCTimeoutSeconds = 300,
    [switch]$SkipBuild,
    [string[]]$NotMetByCanopyDb = @()
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
. (Join-Path $PSScriptRoot 'query-acceptance-notmet.ps1')

# The associations are checked before anything runs.
$associations = Read-NotMetAssociations $NotMetByCanopyDb
if ($associations.Error) {
    [Console]::Error.WriteLine($associations.Error)
    exit 1
}

$forwarded = @{}
foreach ($name in $PSBoundParameters.Keys) {
    if ($name -ne 'NotMetByCanopyDb') { $forwarded[$name] = $PSBoundParameters[$name] }
}
& (Join-Path $PSScriptRoot 'run-query-acceptance.ps1') @forwarded -CanopyDb -NotMetFindings $associations.Findings
exit $LASTEXITCODE
