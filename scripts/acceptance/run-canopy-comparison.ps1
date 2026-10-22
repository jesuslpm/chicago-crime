<#
.SYNOPSIS
    Comparison bench of the CanopyDB module (PRD canopy-db-comparacion): generates the same CSV with
    the SSTable pair and the CanopyDB pair, runs the fixed query set with both query apps, alone and
    in a batch, and writes a report with the figures of both pairs.

.DESCRIPTION
    Builds the two generators and the two query apps in release mode (unless -SkipBuild), then:

    1. Generates the CSV -Repetitions times with each generator, alternating them, each time in an
       empty folder, with the temp folder inside -WorkDir and the same options; it keeps each pair's
       last generation that ended well.
    2. Runs every query of the fixed query set (canopy-fixed-queries.txt, F1 to F11) alone, as a new
       process: a first run (run 0), reported apart, then -Repetitions more, alternating the two query
       apps. Then the 11 queries in a batch, run 0 and -Repetitions more.
    3. Checks that both pairs return the same results (AC-M2) and writes the report and its data
       table in docs\canopy-db\comparisons\, named <yyyy-MM-dd-HHmmss>-<commit>.md and .csv.

    Times: a generation's duration and peak come from its final summary; a query's time from its stats
    line; the process time from the start to the end of the process; a query app's peak is its peak
    private committed memory, read when it ends. The reported figures are medians of runs 1 to
    -Repetitions; nothing is measured cold (the operating system's file cache holds the datasets).

    The bench never fails on a figure: it ends with 1 when a run fails or the pairs return different
    results, and with 0 otherwise. Figures over an inherited limit (AC-M4, AC-C7, AC-M8) are noted.

    Run it on Windows, with the machine without other load and a work folder of its own (it deletes
    its own comparison-* entries there; never share it with the acceptance checks). It needs about
    14 GB free in -WorkDir. With the reference extract and 5 repetitions it takes about 40 minutes.

.PARAMETER WorkDir
    Work folder for the datasets, logs and query outputs; created if missing.

.PARAMETER Csv
    The CSV to generate. Default: chicago-crime-data.csv at the repository root.

.PARAMETER Repetitions
    Measured repetitions of each generation, query and batch (at least 1). Default: 5.

.PARAMETER NoCompression
    Runs both generators with --no-compression (by default the SSTables use Snappy and CanopyDB LZ4).

.PARAMETER Baseline
    The data table (.csv) of an earlier report: adds a Baseline section with each median next to the
    earlier one, the change in percent and whether it is within the tolerance.

.PARAMETER SkipBuild
    Uses the applications already built in target\release.

.EXAMPLE
    .\scripts\acceptance\run-canopy-comparison.ps1 -WorkDir work\comparison

.EXAMPLE
    .\scripts\acceptance\run-canopy-comparison.ps1 -WorkDir work\comparison -Baseline docs\canopy-db\comparisons\2026-10-22-150000-abc1234.csv
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [string]$Csv,
    [int]$Repetitions = 5,
    [switch]$NoCompression,
    [string]$Baseline,
    [switch]$SkipBuild
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Csv) { $Csv = Join-Path $RepoRoot 'chicago-crime-data.csv' }
. (Join-Path $PSScriptRoot 'canopy-comparison.ps1')

$code = Invoke-CanopyComparison -RepoRoot $RepoRoot -WorkDir $WorkDir -Csv $Csv -Repetitions $Repetitions -NoCompression:$NoCompression `
    -Baseline $Baseline -SkipBuild:$SkipBuild -Environment (New-ComparisonEnvironment $RepoRoot)
exit $code
