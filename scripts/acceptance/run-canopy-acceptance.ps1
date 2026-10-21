<#
.SYNOPSIS
    Acceptance of the CanopyDB crime generator on any compatible CSV (PRD canopy-db-generador,
    AC-KG1, AC-M3, AC-M4, AC-KG4 and AC-KG13).

.DESCRIPTION
    The cases of run-acceptance.ps1 against crime-canopy-generator, checked with the CanopyDB
    verifier crime-canopy-acceptance. On the reference extract, recognized by the source SHA-256 in
    Run A's manifest, it requires the exact figures of the PRDs; on any other CSV it counts the
    expected figures on that CSV.

    Builds crime-canopy-generator, crime-canopy-acceptance and the original crime-generator in
    release mode and, inside -WorkDir:

      Run A      a full CanopyDB generation (default compression) into a fresh folder: exit code,
                 wall time, the generator's own peak memory line and an external
                 PeakPagedMemorySize64 sample (AC-M4); right after it, before anything opens its
                 base, a copy of its whole output folder (AC-KG4);
      Run B      a second full CanopyDB generation, with --no-compression;
      Run S      a full generation with the original crime-generator (SSTables);
      verifier   crime-canopy-acceptance on Run A, with --reference on the reference extract, run
                 twice (AC-G1, AC-G3, AC-G4, manifest); afterwards Run A's version folder has the
                 copy's file names, the same manifest bytes and the same entries as the copy
                 (AC-KG4);
      AC-M3      --compare finds the same keys and values in Run A (lz4) and Run B (none), and in
                 Run A and Run S (AC-M3 (SSTables));
      AC-G8      a rerun on Run A without --overwrite is refused and changes nothing;
      AC-G9      a CSV with an altered header aborts, the dataset stays intact and the log explains
                 the cause (also parent AC9, part of AC-G5);
      AC-G7      a real Ctrl+C ends a run with exit code 130, the exact operator message, no
                 temporary files and the dataset intact;
      AC-G6      a rerun with --overwrite killed with Stop-Process -Force once the primary sort has
                 finished leaves the published dataset intact and unreferenced; the next run
                 discards the leftovers and completes, and so replaces the dataset with the same
                 entries as Run A (the second half of AC-G8, checked with --compare against the
                 copy, since two CanopyDB generations are not byte-identical);
      AC-G5      a CSV with a 21-column row, an invalid date and a repeated ID, all made from the
                 first data row of the CSV, completes with 3 rejected (parent AC13).

    It writes canopy-acceptance-report.md to -WorkDir and exits with 0 when the overall result is
    PASS and 1 when it is FAIL. The CSV is only read. Only the entries this script creates in
    -WorkDir (all named canopy-*) are deleted when it starts, so it can share -WorkDir with
    run-acceptance.ps1.

.PARAMETER WorkDir
    Folder for the datasets, the logs and the report. It needs about 18 GB of free space on the
    reference extract: Run A, its copy and Run B (a CanopyDB base of about 3.3 GB each), Run S
    (SSTables, about 1.1 GB), the new version that the AC-G6 and AC-G8 runs write next to Run A
    (3.3 GB) and the sort folder of a run (under 3 GB, as for the original generator).

.PARAMETER Csv
    The CSV: the reference extract or any later download. By default, chicago-crime-data.csv at
    the repository root.

.PARAMETER KillTimeoutSeconds
    How long AC-G6 waits for its run to reach the end of the primary sort. If the run has not
    reached it by then, it is killed anyway and AC-G6 fails; if it ends first, AC-G6 fails too.

.PARAMETER CtrlCAfterSeconds
    When Ctrl+C is pressed in the run of AC-G7.

.PARAMETER SkipBuild
    Use the release binaries already built.

.PARAMETER NotMetByCanopyDb
    Associations <criterion>=<finding> (decision KG10). Only AC-M4 can be associated, once, with a
    finding that has its section ("## <finding> — ...") in docs/canopy-db/hallazgos.md. The AC-M4
    row then reads NOT MET instead of FAIL, and does not count as a failure, when the generation
    exited with 0, the summary has the peak line and the only failure is a peak, external or in the
    summary, above 512 MiB. Items are also split at commas, so a list passed through -File as one
    string reads the same. An invalid association ends the script with 1 before anything runs.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-canopy-acceptance.ps1 -WorkDir work\acceptance

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-canopy-acceptance.ps1 -WorkDir work\acceptance -NotMetByCanopyDb AC-M4=H5
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [string]$Csv,
    [int]$KillTimeoutSeconds = 600,
    [int]$CtrlCAfterSeconds = 25,
    [switch]$SkipBuild,
    [string[]]$NotMetByCanopyDb = @()
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$FindingsRegister = Join-Path $RepoRoot 'docs\canopy-db\hallazgos.md'
# The only criterion that can be associated with a finding (decision KG10).
$NotMetCriterion = 'AC-M4'

# --- The -NotMetByCanopyDb associations, checked before anything runs ---

# Whether the findings register has a section for $Finding: a heading "## <finding> — <title>".
function Test-FindingInRegister([string]$Finding) {
    if (-not (Test-Path -LiteralPath $FindingsRegister -PathType Leaf)) { return $false }
    $pattern = '^## ' + [regex]::Escape($Finding) + ' — '
    foreach ($line in [System.IO.File]::ReadAllLines($FindingsRegister, [System.Text.Encoding]::UTF8)) {
        if ($line -cmatch $pattern) { return $true }
    }
    $false
}

# The reason an association is invalid, or $null; $Associated holds the criteria already associated.
function Get-AssociationError([string]$Association, [hashtable]$Associated) {
    $separator = $Association.IndexOf('=')
    if ($separator -le 0 -or $separator -eq $Association.Length - 1) { return 'expected <criterion>=<finding>' }
    $criterion = $Association.Substring(0, $separator)
    $finding = $Association.Substring($separator + 1)
    if ($criterion -cne $NotMetCriterion) { return "only $NotMetCriterion can be associated" }
    if (-not (Test-FindingInRegister $finding)) { return "$finding is not in the findings register" }
    if ($Associated.ContainsKey($criterion)) { return "$criterion is already associated" }
    $null
}

$NotMetFindings = @{}
foreach ($association in @($NotMetByCanopyDb | ForEach-Object { ([string]$_).Split(',') })) {
    $reason = Get-AssociationError $association $NotMetFindings
    if ($reason) {
        [Console]::Error.WriteLine("Invalid -NotMetByCanopyDb association '$association': $reason.")
        exit 1
    }
    $NotMetFindings[$association.Substring(0, $association.IndexOf('='))] = $association.Substring($association.IndexOf('=') + 1)
}

if (-not $Csv) { $Csv = Join-Path $RepoRoot 'chicago-crime-data.csv' }
$Csv = (Resolve-Path -LiteralPath $Csv).Path
$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
$GeneratorExe = Join-Path $RepoRoot 'target\release\crime-canopy-generator.exe'
$VerifierExe = Join-Path $RepoRoot 'target\release\crime-canopy-acceptance.exe'
$SstGeneratorExe = Join-Path $RepoRoot 'target\release\crime-generator.exe'
$CtrlCHelper = Join-Path $PSScriptRoot 'send-ctrl-c.ps1'
. (Join-Path $PSScriptRoot 'reference-extract.ps1')
$Invariant = [System.Globalization.CultureInfo]::InvariantCulture
$MemoryLimitBytes = 512MB
# Free space this script needs in -WorkDir (see the help of -WorkDir).
$RequiredFreeBytes = 18GB
# Valid records of the reference extract (generator PRD, AC-G1).
$ReferenceValidRecords = 8656904
# The six tables of a dataset, as --compare names its COMPARE-<table> rows.
$CompareTables = @('primary', 'date', 'primary_type', 'district', 'community_area', 'case_number')
# The sort folders of the CanopyDB generator in the temp folder (decision KG1).
$SortDirPattern = 'crime-canopy-generator-sort-*'

# Exact user-facing texts of the generator (PRDs).
$Messages = @{
    DatasetExists     = 'La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.'
    Cancelled         = 'Generación cancelada. No se ha modificado el conjunto existente.'
    OperatorCancelled = 'Generación cancelada por el operador. No se ha modificado el conjunto existente.'
    Leftovers         = 'Se han descartado restos de una generación interrumpida anterior.'
    StaleSortDirs     = 'Se han descartado carpetas temporales de una generación interrumpida anterior.'
    # The start of the line written when the primary sort ends, before the base is written.
    SortCompleted     = 'Ordenación completada.'
    HeaderMismatch    = 'La cabecera no coincide con el esquema: columna 5 esperaba "IUCR" y encontró "IUCR_ALTERADA".'
    # {0}: the valid records with Spanish thousands separators.
    Summary           = 'Generación completada: {0} registros válidos, 0 rechazados (ver log).'
    Ac13Summary       = 'Generación completada: 999 registros válidos, 3 rechazados (ver log).'
    Ac13ColumnCount   = 'Fila 1001 omitida: se esperaban 22 columnas y hay 21.'
    Ac13InvalidDate   = 'Fila 1002 omitida, columna "Date": "02/30/2026 12:00:00 AM" no es una fecha válida (formato MM/DD/AAAA hh:mm:ss AM/PM).'
    # {0}: the ID of the first data row of the CSV.
    Ac13DuplicateId   = 'Fila 1003 omitida: el ID {0} ya apareció en la fila 2.'
    MemoryWarning     = 'Aviso: el pico de memoria privada comprometida'
}

# --- Results and report ---

$Results = New-Object System.Collections.Generic.List[object]
$Sections = New-Object System.Collections.Generic.List[string]

function Add-Outcome([string]$Criterion, [string]$Outcome, [string]$Evidence) {
    $Results.Add([pscustomobject]@{ Criterion = $Criterion; Result = $Outcome; Evidence = $Evidence })
    Write-Host ('{0}  {1}  {2}' -f $Outcome, $Criterion, $Evidence)
}

function Add-Result([string]$Criterion, [bool]$Passed, [string]$Evidence) {
    Add-Outcome $Criterion $(if ($Passed) { 'PASS' } else { 'FAIL' }) $Evidence
}

function Add-Section([string]$Markdown) { $Sections.Add($Markdown) }

function Write-Step([string]$Text) { Write-Host ('[{0:HH:mm:ss}] {1}' -f (Get-Date), $Text) }

function Format-Invariant([string]$Format, [object[]]$Values) { [string]::Format($Invariant, $Format, $Values) }

function Format-Mib([double]$Bytes) { Format-Invariant '{0:N1} MiB' @($Bytes / 1MB) }

function Format-Seconds([double]$Seconds) { Format-Invariant '{0:N1} s' @($Seconds) }

function Format-Count([long]$Value) { Format-Invariant '{0:N0}' @($Value) }

function Get-Lines([string]$Text) { @($Text -split "`r?`n" | Where-Object { $_ -ne '' }) }

function Format-Code([string]$Text) { "``````text`n$($Text.TrimEnd())`n``````" }

# --- Files ---

# The fields of one CSV line as written, quotes included: the reference extract quotes every non-empty
# field, later downloads quote only the fields that need it (such as Location).
function Split-CsvLine([string]$Line) {
    @([regex]::Matches($Line, '(?<=^|,)("(?:[^"]|"")*"|[^,"]*)') | ForEach-Object { $_.Value })
}

# Reads a text file that another process may still have open for writing.
function Read-SharedText([string]$Path) {
    $stream = [System.IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite, Delete')
    try {
        $reader = New-Object System.IO.StreamReader($stream, [System.Text.Encoding]::UTF8)
        return $reader.ReadToEnd()
    }
    finally { $stream.Dispose() }
}

function Get-EntryNames([string]$Dir) {
    if (-not (Test-Path -LiteralPath $Dir)) { return @() }
    @(Get-ChildItem -LiteralPath $Dir -Force | Sort-Object Name | ForEach-Object { $_.Name })
}

# Every folder and file under $Dir with the size and SHA-256 of each file: two equal snapshots
# prove that nothing changed, byte for byte. With -NamesOnly, only the relative paths.
function Get-Snapshot([string]$Dir, [switch]$NamesOnly) {
    $base = (Get-Item -LiteralPath $Dir).FullName.TrimEnd('\')
    $lines = foreach ($item in Get-ChildItem -LiteralPath $Dir -Recurse -Force) {
        $relative = $item.FullName.Substring($base.Length).TrimStart('\')
        if ($item.PSIsContainer) { "$relative\" }
        elseif ($NamesOnly) { $relative }
        else { '{0}|{1}|{2}' -f $relative, $item.Length, (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash }
    }
    (@($lines) | Sort-Object) -join "`n"
}

# The version folder that CURRENT names, as written by the generator: one name and a line feed.
function Get-CurrentName([string]$Output) {
    (Read-SharedText (Join-Path $Output 'CURRENT')).TrimEnd("`r", "`n")
}

function Get-CurrentDir([string]$Output) { Join-Path $Output (Get-CurrentName $Output) }

function Read-Manifest([string]$Output) {
    Get-Content -LiteralPath (Join-Path (Get-CurrentDir $Output) 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
}

# --- Processes ---

# Runs a program with its output captured. With -SampleMemory it samples PeakPagedMemorySize64
# (the peak private committed bytes) every 50 ms. With -KillWhen it asks that script block every
# 50 ms whether to kill: once it returns a description of the state it waited for, or once
# -KillTimeoutSeconds have passed, the process is killed with Stop-Process -Force after listing
# the temporary and output folders. AtKill.Observed holds that description, or $null on timeout.
function Invoke-Program {
    param(
        [string]$FilePath,
        [string[]]$Arguments,
        [switch]$SampleMemory,
        [scriptblock]$KillWhen,
        [int]$KillTimeoutSeconds = 600,
        [string]$TempDir,
        [string]$OutputDir
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ($Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $psi.WorkingDirectory = $RepoRoot
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $peak = [long]0
    $atKill = $null
    while (-not $process.HasExited) {
        if ($SampleMemory) {
            try {
                $process.Refresh()
                if ($process.PeakPagedMemorySize64 -gt $peak) { $peak = $process.PeakPagedMemorySize64 }
            }
            catch { }
        }
        if ($KillWhen -and $null -eq $atKill) {
            # A read that fails while the run changes its files only means "not yet".
            $observed = try { & $KillWhen } catch { $null }
            if ($observed -or $watch.Elapsed.TotalSeconds -ge $KillTimeoutSeconds) {
                $atKill = [pscustomobject]@{
                    Seconds       = $watch.Elapsed.TotalSeconds
                    Observed      = $observed
                    TempEntries   = @(Get-EntryNames $TempDir)
                    OutputEntries = @(Get-EntryNames $OutputDir)
                }
                # The process may have ended meanwhile; the checks after the kill then fail.
                Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            }
        }
        Start-Sleep -Milliseconds 50
    }
    $process.WaitForExit()
    $watch.Stop()
    [pscustomobject]@{
        ExitCode          = $process.ExitCode
        WallSeconds       = $watch.Elapsed.TotalSeconds
        ExternalPeakBytes = $peak
        Stdout            = $stdout.Result
        Stderr            = $stderr.Result
        AtKill            = $atKill
    }
}

function Get-GeneratorArguments([string]$Source, [string]$Output, [string]$Log, [switch]$Overwrite, [switch]$NoCompression) {
    $arguments = @('--input', $Source, '--output', $Output, '--temp-dir', $TempDir, '--log', $Log)
    if ($Overwrite) { $arguments += '--overwrite' }
    if ($NoCompression) { $arguments += '--no-compression' }
    $arguments
}

# Runs the CanopyDB generator, or with -Exe another generator with the same options.
function Invoke-Generator([string]$Source, [string]$Output, [string]$Log, [switch]$Overwrite, [switch]$NoCompression,
    [scriptblock]$KillWhen, [string]$Exe = $GeneratorExe) {
    $arguments = Get-GeneratorArguments -Source $Source -Output $Output -Log $Log -Overwrite:$Overwrite -NoCompression:$NoCompression
    Invoke-Program -FilePath $Exe -Arguments $arguments -SampleMemory `
        -KillWhen $KillWhen -KillTimeoutSeconds $KillTimeoutSeconds -TempDir $TempDir -OutputDir $Output
}

# The state in which AC-G6 kills its run, so that the kill lands while the base is written: the log
# shows the primary sort finished, the run's sort folder is in the temp folder and its new version
# folder exists in the output folder (the next run must find it there). Returns that state in
# words, or $null while the run has not reached it.
function Get-MergeState([string]$Log, [string]$Output, [string]$Temp, [string[]]$OutputBefore, [string[]]$TempBefore) {
    $newVersions = @(Get-EntryNames $Output | Where-Object { $_ -like 'dataset-*' -and $OutputBefore -notcontains $_ })
    $sortDirs = @(Get-EntryNames $Temp | Where-Object { $_ -like $SortDirPattern -and $TempBefore -notcontains $_ })
    if ($newVersions.Count -eq 0 -or $sortDirs.Count -eq 0) { return $null }
    $sortLine = @(Get-Lines (Read-SharedText $Log) |
            Where-Object { $_.StartsWith($Messages.SortCompleted, [System.StringComparison]::Ordinal) }) | Select-Object -First 1
    if (-not $sortLine) { return $null }
    'log line "{0}", sort folders {1}, new version folder {2}' -f $sortLine, ($sortDirs -join ', '), ($newVersions -join ', ')
}

# The peak the generator reports in its summary, in bytes ("Pico de memoria privada comprometida:
# 93,1 MiB (límite: 512 MiB).").
function Get-ReportedPeakBytes([string]$LogText) {
    $match = [regex]::Match($LogText, 'Pico de memoria privada comprometida: ([0-9.]+,[0-9]) MiB')
    if (-not $match.Success) { return $null }
    $mib = [double]::Parse($match.Groups[1].Value.Replace('.', '').Replace(',', '.'), $Invariant)
    $mib * 1MB
}

function Get-LogLine([string]$LogText, [string]$Pattern) {
    @(Get-Lines $LogText | Where-Object { $_ -like $Pattern }) | Select-Object -First 1
}

# The PASS/FAIL/SKIP rows of a verifier report, by criterion.
function Get-Verdicts([string]$Stdout) {
    $verdicts = [ordered]@{}
    foreach ($line in Get-Lines $Stdout) {
        $match = [regex]::Match($line, '^(PASS|FAIL|SKIP)  (\S+)  (.*)$')
        if ($match.Success) {
            $verdicts[$match.Groups[2].Value] = [pscustomobject]@{ Outcome = $match.Groups[1].Value; Summary = $match.Groups[3].Value }
        }
    }
    $verdicts
}

# crime-canopy-acceptance --compare <A> <B>: passed when it exits with 0 and each of the six tables
# has its COMPARE-<table> row PASS.
function Invoke-Compare([string]$Left, [string]$Right) {
    $run = Invoke-Program -FilePath $VerifierExe -Arguments @('--compare', $Left, $Right)
    $verdicts = Get-Verdicts $run.Stdout
    $passed = $run.ExitCode -eq 0
    $parts = foreach ($table in $CompareTables) {
        $id = "COMPARE-$table"
        if ($verdicts.Contains($id)) {
            $passed = $passed -and $verdicts[$id].Outcome -eq 'PASS'
            '{0} {1}' -f $table, $verdicts[$id].Summary
        }
        else {
            $passed = $false
            "$table missing"
        }
    }
    # A COMPARE row is the reason the bases could not be opened.
    if ($verdicts.Contains('COMPARE')) { $parts = @($parts) + ('COMPARE {0}: {1}' -f $verdicts['COMPARE'].Outcome, $verdicts['COMPARE'].Summary) }
    [pscustomobject]@{
        Passed   = $passed
        ExitCode = $run.ExitCode
        Evidence = ('--compare {0} {1}: exit {2}; {3}' -f (Split-Path $Left -Leaf), (Split-Path $Right -Leaf), $run.ExitCode, (@($parts) -join ', '))
        Stdout   = $run.Stdout
        Stderr   = $run.Stderr
    }
}

function Format-CompareSection([string]$Title, $Compare) {
    @("### $Title", '', $Compare.Evidence, '', (Format-Code $Compare.Stdout)) -join "`n"
}

# --- Preparation ---

$started = Get-Date
foreach ($path in @($Csv, $CtrlCHelper)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing file: $path" }
}
# AC-G5 makes its bad rows from the first data row of the CSV, whatever it is: 22 fields, an ID first
# and the date third.
$headLines = @(Get-Content -LiteralPath $Csv -TotalCount 2 -Encoding UTF8)
$firstDataRow = [string]$headLines[1]
$SampleFields = Split-CsvLine $firstDataRow
if ($SampleFields.Count -ne 22 -or $SampleFields[0].Trim('"') -notmatch '^[0-9]+$') {
    throw "The first data row of the CSV does not have 22 fields starting with a numeric ID: $firstDataRow"
}
# AC-G9 alters the fifth header field, so it must be IUCR: otherwise the "altered" CSV would carry the
# unchanged header.
$headerFieldNames = @(Split-CsvLine ([string]$headLines[0]) | ForEach-Object { $_.Trim('"') })
if ($headerFieldNames.Count -ne 22 -or $headerFieldNames[4] -cne 'IUCR') {
    throw "The CSV header does not have 22 fields with IUCR fifth: $($headLines[0])"
}
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
# Every entry is named canopy-*, so none is an entry of run-acceptance.ps1 in a shared -WorkDir.
$RunA = Join-Path $WorkDir 'canopy-run-a'
$RunACopy = Join-Path $WorkDir 'canopy-run-a-copy'
$RunB = Join-Path $WorkDir 'canopy-run-b'
$RunS = Join-Path $WorkDir 'canopy-run-s'
$Ac13Output = Join-Path $WorkDir 'canopy-ac13-out'
$TempDir = Join-Path $WorkDir 'canopy-temp'
$LogDir = Join-Path $WorkDir 'canopy-logs'
$InputDir = Join-Path $WorkDir 'canopy-inputs'
$ReportPath = Join-Path $WorkDir 'canopy-acceptance-report.md'
Write-Step "Cleaning the entries of a previous CanopyDB acceptance in $WorkDir"
foreach ($path in @($RunA, $RunACopy, $RunB, $RunS, $Ac13Output, $TempDir, $LogDir, $InputDir, $ReportPath)) {
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
}
foreach ($dir in @($TempDir, $LogDir, $InputDir)) { New-Item -ItemType Directory -Path $dir | Out-Null }

$freeBytes = (Get-Item -LiteralPath $WorkDir).PSDrive.Free
if ($freeBytes -lt $RequiredFreeBytes) {
    throw ('Only {0} free in {1}; about {2:N0} GB are needed.' -f (Format-Mib $freeBytes), $WorkDir, ($RequiredFreeBytes / 1GB))
}

if (-not $SkipBuild) {
    Write-Step 'Building crime-canopy-generator, crime-canopy-acceptance and crime-generator in release mode'
    $cargo = (Get-Command cargo).Source
    $build = Invoke-Program -FilePath $cargo -Arguments @('build', '--release', '-p', 'crime-canopy-generator', '-p', 'crime-canopy-acceptance', '-p', 'crime-generator')
    if ($build.ExitCode -ne 0) { throw "cargo build failed:`n$($build.Stderr)" }
}
foreach ($path in @($GeneratorExe, $VerifierExe, $SstGeneratorExe)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing binary: $path" }
}

$git = Get-Command git -ErrorAction SilentlyContinue
$commit = 'unknown'
if ($git) {
    $head = Invoke-Program -FilePath $git.Source -Arguments @('-C', $RepoRoot, 'rev-parse', '--short', 'HEAD')
    $status = Invoke-Program -FilePath $git.Source -Arguments @('-C', $RepoRoot, 'status', '--porcelain')
    $commit = $head.Stdout.Trim()
    if ($status.Stdout.Trim()) { $commit += ' with uncommitted changes' }
}
$processor = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
$memoryGiB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
$os = (Get-CimInstance Win32_OperatingSystem).Caption
$csvSize = (Get-Item -LiteralPath $Csv).Length

# --- Run A, its copy, Run B and Run S ---

# Not $runs: PowerShell names ignore case, so it would be $RunS.
$generations = [ordered]@{}
$manifests = @{}
foreach ($name in @('A', 'B', 'S')) {
    $output = switch ($name) { 'A' { $RunA } 'B' { $RunB } 'S' { $RunS } }
    $log = Join-Path $LogDir "run-$($name.ToLower()).log"
    $what = switch ($name) {
        'A' { 'CanopyDB generator, default compression' }
        'B' { 'CanopyDB generator, --no-compression' }
        'S' { 'original crime-generator, SSTables' }
    }
    Write-Step "Run $name ($what) into $output"
    $exe = if ($name -eq 'S') { $SstGeneratorExe } else { $GeneratorExe }
    $run = Invoke-Generator -Source $Csv -Output $output -Log $log -NoCompression:($name -eq 'B') -Exe $exe
    $logText = Read-SharedText $log
    $run | Add-Member -NotePropertyName Log -NotePropertyValue $logText
    $run | Add-Member -NotePropertyName ReportedPeakBytes -NotePropertyValue (Get-ReportedPeakBytes $logText)
    $run | Add-Member -NotePropertyName TempAfter -NotePropertyValue @(Get-EntryNames $TempDir)
    $run | Add-Member -NotePropertyName What -NotePropertyValue $what
    $generations[$name] = $run
    if ($run.ExitCode -eq 0) { $manifests[$name] = Read-Manifest $output }
    Write-Step ('Run {0}: exit {1}, {2}, external peak {3}' -f $name, $run.ExitCode, (Format-Seconds $run.WallSeconds), (Format-Mib $run.ExternalPeakBytes))
    if ($name -eq 'A' -and $run.ExitCode -eq 0) {
        # AC-KG4: the copy is made before anything opens Run A's base.
        Write-Step "Copying Run A's output folder to $RunACopy before anything opens its base"
        Copy-Item -LiteralPath $RunA -Destination $RunACopy -Recurse
    }
}

# --- AC-M4: memory of Run A (AC-G2 and AC-L8 of the original) ---

$firstRun = $generations['A']
$firstRunPeakLine = Get-LogLine $firstRun.Log 'Pico de memoria privada comprometida:*'
$exitOk = $firstRun.ExitCode -eq 0
$peakLinePresent = $null -ne $firstRun.ReportedPeakBytes
$sampled = $firstRun.ExternalPeakBytes -gt 0
$externalOver = $sampled -and $firstRun.ExternalPeakBytes -gt $MemoryLimitBytes
$reportedOver = $peakLinePresent -and $firstRun.ReportedPeakBytes -gt $MemoryLimitBytes
$memoryWarning = $firstRun.Log.Contains($Messages.MemoryWarning)
$memoryOk = $exitOk -and $sampled -and -not $externalOver -and $peakLinePresent -and -not $reportedOver -and -not $memoryWarning
$memoryEvidence = 'Run A exit {0} in {1}; summary line "{2}"; external PeakPagedMemorySize64 {3} ({4} bytes); limit 512 MiB; no memory warning in the log: {5}' -f `
    $firstRun.ExitCode, (Format-Seconds $firstRun.WallSeconds), $firstRunPeakLine, (Format-Mib $firstRun.ExternalPeakBytes), (Format-Count $firstRun.ExternalPeakBytes), (-not $memoryWarning)
# Decision KG10: with an associated finding, NOT MET only when the generation exited with 0, the
# summary has the peak line and the only failure is a peak above 512 MiB, external or in the
# summary (the generator's memory warning goes with a summary peak above the limit). Any other
# failure stays FAIL.
$onlyPeakFails = $exitOk -and $peakLinePresent -and $sampled -and ($externalOver -or $reportedOver) -and (-not $memoryWarning -or $reportedOver)
if ($memoryOk) { Add-Outcome 'AC-M4' 'PASS' $memoryEvidence }
elseif ($NotMetFindings.ContainsKey('AC-M4') -and $onlyPeakFails) {
    Add-Outcome 'AC-M4' 'NOT MET' ('{0} (not met by CanopyDB, {1})' -f $memoryEvidence, $NotMetFindings['AC-M4'])
}
else { Add-Outcome 'AC-M4' 'FAIL' $memoryEvidence }

$runRows = foreach ($name in $generations.Keys) {
    $run = $generations[$name]
    $reported = if ($null -ne $run.ReportedPeakBytes) { Format-Mib $run.ReportedPeakBytes } else { 'missing' }
    $compression = if ($manifests.ContainsKey($name)) { [string]$manifests[$name].compression } else { '-' }
    '| {0} | {1} | {2} | {3} | {4} | {5} ({6} bytes) | {7} | {8} |' -f $name, $run.What, $run.ExitCode, (Format-Seconds $run.WallSeconds), $reported, `
        (Format-Mib $run.ExternalPeakBytes), (Format-Count $run.ExternalPeakBytes), $compression, $(if ($run.TempAfter.Count -eq 0) { 'empty' } else { $run.TempAfter -join ', ' })
}
$fileRows = if ($manifests.ContainsKey('A')) {
    foreach ($file in @($manifests['A'].files)) { '| `{0}` | {1} |' -f $file.path, (Format-Count $file.size_bytes) }
}
else { @('| - | - |') }
Add-Section (@(
        '## Run A, Run B and Run S'
        ''
        '| Run | Generator | Exit | Wall time | Peak reported by the generator | External PeakPagedMemorySize64 | Manifest compression | Temp folder after |'
        '|---|---|---|---|---|---|---|---|'
        $runRows
        ''
        '### Files of Run A (manifest)'
        ''
        '| Path | Size at publication (bytes) |'
        '|---|---|'
        $fileRows
        ''
        '### Run A log'
        ''
        (Format-Code $firstRun.Log)
    ) -join "`n")

# --- Which CSV: the reference extract or another one ---

# The source fingerprint that Run A's manifest records. Without a manifest Run A failed anyway, and
# the CSV is hashed here only for the report header.
$runAManifest = if ($manifests.ContainsKey('A')) { $manifests['A'] } else { $null }
if ($runAManifest) { $SourceSha256 = [string]$runAManifest.source.sha256 }
else { $SourceSha256 = (Get-FileHash -LiteralPath $Csv -Algorithm SHA256).Hash.ToLowerInvariant() }
$IsReference = Test-ReferenceExtract $SourceSha256
Write-Step (Format-CsvMode $SourceSha256)
if ($runAManifest -and -not (Test-Sha256 $SourceSha256)) {
    Add-Result 'Run A: source SHA-256' $false ('the manifest records no valid source SHA-256: "{0}"' -f $SourceSha256)
}

# --- The CanopyDB verifier on Run A, twice: AC-G1, AC-G3, AC-G4, manifest ---

$verifierArguments = @('--csv', $Csv, '--dataset', $RunA)
if ($IsReference) { $verifierArguments += '--reference' }
$verifierRuns = foreach ($pass in 1, 2) {
    Write-Step "CanopyDB verifier on Run A, read $pass of 2"
    Invoke-Program -FilePath $VerifierExe -Arguments $verifierArguments
}
$verifier = $verifierRuns[0]
$verdicts = Get-Verdicts $verifier.Stdout
# Only a PASS proves a criterion: with --reference the verifier never skips, and on another CSV a
# check skipped for lack of data still fails (PRD aceptacion-cualquier-csv, decision A3).
function Get-VerdictResult([string[]]$Criteria) {
    $passed = $true
    $parts = foreach ($criterion in $Criteria) {
        if ($verdicts.Contains($criterion)) {
            $passed = $passed -and $verdicts[$criterion].Outcome -eq 'PASS'
            '{0} {1}: {2}' -f $criterion, $verdicts[$criterion].Outcome, $verdicts[$criterion].Summary
        }
        else {
            $passed = $false
            "$criterion missing"
        }
    }
    [pscustomobject]@{ Passed = $passed; Evidence = ($parts -join ' / ') }
}

# On another CSV the valid records are those of the manifest; the verifier's AC-G1 proves that they
# are the CSV's data rows minus the rejected ones, and the summary requires 0 rejected.
$expectedValid = if ($IsReference) { $ReferenceValidRecords } elseif ($runAManifest) { [long]$runAManifest.records.valid } else { -1 }
$expectedSummary = $Messages.Summary -f (Format-Count $expectedValid).Replace(',', '.')
$summaryLine = $expectedValid -ge 0 -and $firstRun.Log.Contains($expectedSummary)
$g1 = Get-VerdictResult @('AC-G1')
Add-Result 'AC-G1' ($g1.Passed -and $summaryLine) ('{0} / Run A summary "{1}": {2}' -f $g1.Evidence, $expectedSummary, $summaryLine)
$g3 = Get-VerdictResult @('AC-G3', 'AC-G3-SAMPLE', 'AC-G3-DST')
Add-Result 'AC-G3' $g3.Passed $g3.Evidence
$g4 = Get-VerdictResult @('AC-G4-date', 'AC-G4-primary_type', 'AC-G4-district', 'AC-G4-community_area', 'AC-G4-case_number', 'AC-G4-THEFT', 'AC-G4-COMMUNITY-NIL', 'AC-G4-SHARED-CASES')
Add-Result 'AC-G4' $g4.Passed $g4.Evidence
# The CanopyDB layout and manifest (PRD canopy-db-generador, "Informe del verificador").
$manifest = Get-VerdictResult @('LAYOUT', 'MANIFEST-SCHEMA', 'MANIFEST-FILES', 'CSV', 'MANIFEST-SOURCE', 'MANIFEST-TABLES')
Add-Result 'Manifest' $manifest.Passed $manifest.Evidence
# Schema 2 (query app PRD, decision C1): the text indexes hold their values in lowercase.
$lowercase = Get-VerdictResult @('AC-C18-LOWERCASE')
Add-Result 'Lowercase text indexes (query app AC-C18)' $lowercase.Passed $lowercase.Evidence
Add-Section (@(
        '## CanopyDB verifier on Run A'
        ''
        ('`crime-canopy-acceptance --csv <CSV> --dataset canopy-run-a{0}`, read 1: exit {1}; read 2: exit {2}.' -f `
            $(if ($IsReference) { ' --reference' } else { '' }), $verifierRuns[0].ExitCode, $verifierRuns[1].ExitCode)
        ''
        'Read 1:'
        ''
        (Format-Code $verifierRuns[0].Stdout)
        ''
        'Progress of read 1 (standard error):'
        ''
        (Format-Code $verifierRuns[0].Stderr)
        ''
        'Read 2:'
        ''
        (Format-Code $verifierRuns[1].Stdout)
    ) -join "`n")

# --- AC-KG4: reading the base adds no file, keeps the manifest and the entries ---

Write-Step 'AC-KG4: Run A after the two reads against its copy'
# Each read went through every tree: its report has the MANIFEST-TABLES row, which counts them.
$readsComplete = @($verifierRuns | Where-Object { (Get-Verdicts $_.Stdout).Contains('MANIFEST-TABLES') }).Count -eq 2
$copied = Test-Path -LiteralPath (Join-Path $RunACopy 'CURRENT') -PathType Leaf
$sameNames = $false
$sameManifest = $false
$namesText = '-'
if ($copied -and $runAManifest) {
    $versionName = Get-CurrentName $RunA
    $sameCurrent = $versionName -eq (Get-CurrentName $RunACopy)
    $namesAfter = Get-Snapshot (Join-Path $RunA $versionName) -NamesOnly
    $namesCopy = Get-Snapshot (Join-Path $RunACopy $versionName) -NamesOnly
    $sameNames = $sameCurrent -and $namesAfter -eq $namesCopy
    $namesText = ($namesAfter -split "`n") -join ', '
    $manifestHash = (Get-FileHash -LiteralPath (Join-Path (Join-Path $RunA $versionName) 'manifest.json') -Algorithm SHA256).Hash
    $copyManifestHash = (Get-FileHash -LiteralPath (Join-Path (Join-Path $RunACopy $versionName) 'manifest.json') -Algorithm SHA256).Hash
    $sameManifest = $manifestHash -eq $copyManifestHash
}
$kg4Compare = if ($copied) { Invoke-Compare $RunA $RunACopy } else { $null }
$kg4Ok = $readsComplete -and $sameNames -and $sameManifest -and $null -ne $kg4Compare -and $kg4Compare.Passed
Add-Result 'AC-KG4' $kg4Ok ('copy of the output folder made before the first open: {0}; both reads went through every tree: {1}; afterwards the version folder holds the same names as the copy ({2}): {3}; manifest.json byte-identical: {4}; {5}' -f `
        $copied, $readsComplete, $namesText, $sameNames, $sameManifest, $(if ($kg4Compare) { $kg4Compare.Evidence } else { 'no comparison' }))

# --- AC-M3: the same keys and values with and without compression, and in the SSTables ---

Write-Step 'AC-M3: comparing Run A with Run B and with Run S'
$compressionA = if ($manifests.ContainsKey('A')) { [string]$manifests['A'].compression } else { '-' }
$compressionB = if ($manifests.ContainsKey('B')) { [string]$manifests['B'].compression } else { '-' }
$m3Compare = if ($manifests.ContainsKey('A') -and $manifests.ContainsKey('B')) { Invoke-Compare $RunA $RunB } else { $null }
$m3Ok = $compressionA -eq 'lz4' -and $compressionB -eq 'none' -and $null -ne $m3Compare -and $m3Compare.Passed
Add-Result 'AC-M3' $m3Ok ('Run A compression {0}, Run B compression {1}; Run B exit {2} in {3}; {4}' -f `
        $compressionA, $compressionB, $generations['B'].ExitCode, (Format-Seconds $generations['B'].WallSeconds), $(if ($m3Compare) { $m3Compare.Evidence } else { 'no comparison' }))
$sstCompare = if ($manifests.ContainsKey('A') -and $manifests.ContainsKey('S')) { Invoke-Compare $RunA $RunS } else { $null }
Add-Result 'AC-M3 (SSTables)' ($null -ne $sstCompare -and $sstCompare.Passed) ('Run S (crime-generator) exit {0} in {1}; {2}' -f `
        $generations['S'].ExitCode, (Format-Seconds $generations['S'].WallSeconds), $(if ($sstCompare) { $sstCompare.Evidence } else { 'no comparison' }))
$compareSections = @('## Comparisons (`crime-canopy-acceptance --compare`)')
foreach ($pair in @(@('AC-KG4: Run A after the reads and its copy', $kg4Compare), @('AC-M3: Run A and Run B', $m3Compare), @('AC-M3 (SSTables): Run A and Run S', $sstCompare))) {
    if ($pair[1]) { $compareSections += Format-CompareSection $pair[0] $pair[1] }
}
Add-Section ($compareSections -join "`n`n")

# --- AC-G8: a rerun without --overwrite is refused and changes nothing ---

Write-Step 'AC-G8: rerun without --overwrite'
$before = Get-Snapshot $RunA
$refusedLog = Join-Path $LogDir 'ac-g8-refused.log'
$refused = Invoke-Generator -Source $Csv -Output $RunA -Log $refusedLog
$refusedText = Read-SharedText $refusedLog
$unchanged = (Get-Snapshot $RunA) -eq $before
Add-Result 'AC-G8 (refusal)' ($refused.ExitCode -eq 1 -and $refusedText.Contains($Messages.DatasetExists) -and $unchanged) `
('exit {0} in {1}; log: "{2}"; Run A byte-identical afterwards: {3}' -f $refused.ExitCode, (Format-Seconds $refused.WallSeconds), $Messages.DatasetExists, $unchanged)

# --- AC-G9 and parent AC9: an altered header aborts and the log explains why ---

Write-Step 'AC-G9: CSV with an altered header'
$alteredCsv = Join-Path $InputDir 'header-altered.csv'
$firstLines = @(Get-Content -LiteralPath $Csv -TotalCount 1000 -Encoding UTF8)
# The fifth header field, quoted or not, becomes IUCR_ALTERADA.
$headerFields = Split-CsvLine $firstLines[0]
$headerFields[4] = $headerFields[4].Replace('IUCR', 'IUCR_ALTERADA')
$firstLines[0] = $headerFields -join ','
[System.IO.File]::WriteAllText($alteredCsv, (($firstLines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding $false))
$before = Get-Snapshot $RunA
$headerLog = Join-Path $LogDir 'ac-g9-header.log'
$header = Invoke-Generator -Source $alteredCsv -Output $RunA -Log $headerLog -Overwrite
$headerLogExists = Test-Path -LiteralPath $headerLog -PathType Leaf
$headerText = if ($headerLogExists) { Read-SharedText $headerLog } else { '' }
$unchanged = (Get-Snapshot $RunA) -eq $before
$explains = $headerText.Contains($Messages.HeaderMismatch) -and $headerText.Contains($Messages.Cancelled)
Add-Result 'AC-G9' ($header.ExitCode -eq 1 -and $headerLogExists -and $explains) `
('exit {0}; the log {1} exists: {2}; it reads "{3}" then "{4}": {5}' -f $header.ExitCode, (Split-Path $headerLog -Leaf), $headerLogExists, $Messages.HeaderMismatch, $Messages.Cancelled, $explains)
Add-Result 'AC-G5 (parent AC9)' ($header.ExitCode -eq 1 -and $explains -and $unchanged) `
('aborted with the header message; Run A byte-identical afterwards: {0}' -f $unchanged)
Add-Section (@('## AC-G9: log of the run with an altered header', '', (Format-Code $headerText)) -join "`n")

# --- AC-G7: a real Ctrl+C ---

Write-Step "AC-G7: Ctrl+C after $CtrlCAfterSeconds s"
$before = Get-Snapshot $RunA
$ctrlCLog = Join-Path $LogDir 'ac-g7-ctrl-c.log'
$requestFile = Join-Path $InputDir 'ctrl-c-request.json'
$resultFile = Join-Path $InputDir 'ctrl-c-result.json'
[pscustomobject]@{
    Exe          = $GeneratorExe
    Arguments    = @(Get-GeneratorArguments -Source $Csv -Output $RunA -Log $ctrlCLog -Overwrite)
    DelaySeconds = $CtrlCAfterSeconds
    TempDir      = $TempDir
    ResultFile   = $resultFile
} | ConvertTo-Json | Set-Content -LiteralPath $requestFile -Encoding UTF8
# A console of its own, so the Ctrl+C reaches the generator and nothing else.
$helperArguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}" -RequestFile "{1}"' -f $CtrlCHelper, $requestFile
Start-Process -FilePath 'powershell.exe' -ArgumentList $helperArguments -WindowStyle Hidden -Wait
$ctrlC = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
$ctrlCText = Read-SharedText $ctrlCLog
$lastLine = @(Get-Lines $ctrlCText)[-1]
$tempAfter = @(Get-EntryNames $TempDir)
$unchanged = (Get-Snapshot $RunA) -eq $before
$ctrlCOk = $ctrlC.RunningWhenSent -and $ctrlC.Sent -and $ctrlC.ExitCode -eq 130 -and $lastLine -eq $Messages.OperatorCancelled `
    -and -not $ctrlCText.Contains($Messages.Cancelled) -and $tempAfter.Count -eq 0 -and $unchanged
Add-Result 'AC-G7' $ctrlCOk ('Ctrl+C sent at {0} s while running ({1}; sort folders then: {2}); exit {3} after {4} s; last log line "{5}"; temp folder afterwards: {6}; Run A byte-identical: {7}' -f `
        $CtrlCAfterSeconds, $ctrlC.Sent, (@($ctrlC.TempDuring) -join ', '), $ctrlC.ExitCode, $ctrlC.SecondsToExit, $lastLine, `
        $(if ($tempAfter.Count -eq 0) { 'empty' } else { $tempAfter -join ', ' }), $unchanged)
Add-Section (@('## AC-G7: log of the run cancelled with Ctrl+C', '', (Format-Code $ctrlCText)) -join "`n")

# --- AC-G6: a hard kill once the primary sort has finished, then a normal run ---

Write-Step 'AC-G6: Stop-Process -Force once the primary sort has finished'
$currentBefore = Get-CurrentName $RunA
$versionBefore = Get-Snapshot (Join-Path $RunA $currentBefore)
$killLog = Join-Path $LogDir 'ac-g6-killed.log'
$outputBeforeKill = @(Get-EntryNames $RunA)
$tempBeforeKill = @(Get-EntryNames $TempDir)
$inMerge = {
    Get-MergeState -Log $script:killLog -Output $script:RunA -Temp $script:TempDir `
        -OutputBefore $script:outputBeforeKill -TempBefore $script:tempBeforeKill
}
$killed = Invoke-Generator -Source $Csv -Output $RunA -Log $killLog -Overwrite -KillWhen $inMerge
$killText = Read-SharedText $killLog
$phase = @(Get-Lines $killText)[-1]
$currentAfterKill = Get-CurrentName $RunA
$versionAfterKill = Get-Snapshot (Join-Path $RunA $currentAfterKill)
$outputAfterKill = @(Get-EntryNames $RunA)
$tempAfterKill = @(Get-EntryNames $TempDir)
$unreferenced = @($outputAfterKill | Where-Object { $_ -like 'dataset-*' -and $_ -ne $currentBefore })
# Only a kill in the state waited for proves anything: a run that ended first, or a timeout, fails.
$killedInMerge = $null -ne $killed.AtKill -and $null -ne $killed.AtKill.Observed
$intact = $killedInMerge -and $currentAfterKill -eq $currentBefore -and $versionAfterKill -eq $versionBefore
$killedWhen = if ($killedInMerge) { 'at {0}, once it observed {1}' -f (Format-Seconds $killed.AtKill.Seconds), $killed.AtKill.Observed }
elseif ($null -ne $killed.AtKill) { "at the timeout of $KillTimeoutSeconds s, without observing the primary sort finished (FAIL)" }
else { 'never: the run ended before the primary sort was observed finished (FAIL)' }
Add-Result 'AC-G6 (kill)' $intact ('killed {0}; exit {1}; last log line then: "{2}"; output then: {3}; afterwards CURRENT still names {4} and its files are byte-identical: {5}; unreferenced version folders left: {6}; sort folders left: {7}' -f `
        $killedWhen, $killed.ExitCode, $phase, `
    $(if ($killed.AtKill) { $killed.AtKill.OutputEntries -join ', ' } else { '-' }), $currentBefore, $intact, ($unreferenced -join ', '), ($tempAfterKill -join ', '))

Write-Step 'AC-G6: next run with --overwrite'
$recoveryLog = Join-Path $LogDir 'ac-g6-next-run.log'
$recovery = Invoke-Generator -Source $Csv -Output $RunA -Log $recoveryLog -Overwrite
$recoveryText = Read-SharedText $recoveryLog
$currentAfter = if ($recovery.ExitCode -eq 0) { Get-CurrentName $RunA } else { $currentBefore }
$outputAfter = @(Get-EntryNames $RunA)
$tempAfter = @(Get-EntryNames $TempDir)
$discardedBoth = $recoveryText.Contains($Messages.StaleSortDirs) -and $recoveryText.Contains($Messages.Leftovers)
$clean = $tempAfter.Count -eq 0 -and $outputAfter.Count -eq 2 -and ($outputAfter -contains 'CURRENT') -and ($outputAfter -contains $currentAfter)
$recovered = $recovery.ExitCode -eq 0 -and $discardedBoth -and $clean -and $tempAfterKill.Count -gt 0 -and $unreferenced.Count -gt 0
Add-Result 'AC-G6 (next run)' $recovered ('exit {0} in {1}; log reads "{2}" and "{3}": {4}; afterwards the output holds {5} and the temp folder {6}' -f `
        $recovery.ExitCode, (Format-Seconds $recovery.WallSeconds), $Messages.StaleSortDirs, $Messages.Leftovers, $discardedBoth, `
    ($outputAfter -join ', '), $(if ($tempAfter.Count -eq 0) { 'is empty' } else { 'holds ' + ($tempAfter -join ', ') }))
# Two CanopyDB generations are not byte-identical (decision K2): the new version must hold the same
# keys and values as Run A, whose copy keeps its first generation.
$replacedCompare = if ($recovery.ExitCode -eq 0 -and $copied) { Invoke-Compare $RunACopy $RunA } else { $null }
$replaced = $recovery.ExitCode -eq 0 -and $currentAfter -ne $currentBefore -and -not ($outputAfter -contains $currentBefore) `
    -and $null -ne $replacedCompare -and $replacedCompare.Passed
Add-Result 'AC-G8 (replacement)' $replaced ('with --overwrite, CURRENT moved from {0} to {1}, the previous version is gone, and the new version holds the same entries as Run A: {2}; {3}' -f `
        $currentBefore, $currentAfter, $replaced, $(if ($replacedCompare) { $replacedCompare.Evidence } else { 'no comparison' }))
Add-Section (@(
        '## AC-G6: logs of the killed run and of the next run'
        ''
        'Killed run:'
        ''
        (Format-Code $killText)
        ''
        'Next run:'
        ''
        (Format-Code $recoveryText)
    ) -join "`n")

# --- AC-G5 (parent AC13): three bad rows are rejected and the run completes ---

Write-Step 'AC-G5: CSV with a 21-column row, an invalid date and a repeated ID'
$sample = $firstDataRow
$sampleId = $SampleFields[0].Trim('"')
$sampleDate = $SampleFields[2].Trim('"')
# The sample row with another ID and date, both written as the sample writes them, cut to $FieldCount fields.
function New-SampleRow([string]$Id, [string]$Date, [int]$FieldCount = 22) {
    $fields = [string[]]$SampleFields.Clone()
    $fields[0] = $SampleFields[0].Replace($sampleId, $Id)
    $fields[2] = $SampleFields[2].Replace($sampleDate, $Date)
    $fields[0..($FieldCount - 1)] -join ','
}
$badRows = @(
    # 21 columns: the sample row with a new ID and without its last field (Location).
    (New-SampleRow '99000001' $sampleDate 21)
    # An invalid date with a new ID.
    (New-SampleRow '99000002' '02/30/2026 12:00:00 AM')
    # The ID of line 2 again.
    $sample
)
$ac13DuplicateId = $Messages.Ac13DuplicateId -f $sampleId
$ac13Csv = Join-Path $InputDir 'ac13.csv'
$ac13Lines = @(Get-Content -LiteralPath $Csv -TotalCount 1000 -Encoding UTF8) + $badRows
[System.IO.File]::WriteAllText($ac13Csv, (($ac13Lines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding $false))
$ac13Log = Join-Path $LogDir 'ac-g5-ac13.log'
$ac13 = Invoke-Generator -Source $ac13Csv -Output $Ac13Output -Log $ac13Log
$ac13Text = Read-SharedText $ac13Log
$ac13Manifest = if ($ac13.ExitCode -eq 0) { Read-Manifest $Ac13Output } else { $null }
$rowsLogged = $ac13Text.Contains($Messages.Ac13ColumnCount) -and $ac13Text.Contains($Messages.Ac13InvalidDate) -and $ac13Text.Contains($ac13DuplicateId)
$manifestCounts = $null -ne $ac13Manifest -and $ac13Manifest.records.valid -eq 999 -and $ac13Manifest.records.rejected -eq 3
Add-Result 'AC-G5 (parent AC13)' ($ac13.ExitCode -eq 0 -and $rowsLogged -and $ac13Text.Contains($Messages.Ac13Summary) -and $manifestCounts) `
('exit {0}; the log names each row ({1}); summary "{2}"; manifest valid {3}, rejected {4}' -f $ac13.ExitCode, $rowsLogged, $Messages.Ac13Summary, `
    $(if ($ac13Manifest) { $ac13Manifest.records.valid } else { '-' }), $(if ($ac13Manifest) { $ac13Manifest.records.rejected } else { '-' }))
Add-Section (@('## AC-G5 (parent AC13): log', '', (Format-Code $ac13Text)) -join "`n")

# --- Report ---

$failed = @($Results | Where-Object { $_.Result -eq 'FAIL' })
$notMet = @($Results | Where-Object { $_.Result -eq 'NOT MET' })
$resultRows = foreach ($result in $Results) {
    '| {0} | {1} | {2} |' -f $result.Criterion, $result.Result, $result.Evidence.Replace('|', '\|')
}
$total = $Results.Count
# NOT MET rows are not failures (decision KG10).
$overall = if ($failed.Count -gt 0) { "FAIL: $($failed.Count) of $total checks failed" }
elseif ($notMet.Count -gt 0) { 'PASS: {0} of {1} checks passed, 1 not met by CanopyDB ({2})' -f ($total - $notMet.Count), $total, $NotMetFindings['AC-M4'] }
else { "PASS: all $total checks passed" }
$associations = if ($NotMetFindings.Count -gt 0) { ($NotMetFindings.Keys | ForEach-Object { '`{0}={1}`' -f $_, $NotMetFindings[$_] }) -join ', ' } else { 'none' }
$report = @(
    '# Acceptance of the CanopyDB crime generator'
    ''
    ('- **Result**: {0}' -f $overall)
    ('- **Date**: {0:yyyy-MM-dd HH:mm} (duration {1})' -f $started, (Format-Seconds ((Get-Date) - $started).TotalSeconds))
    ('- **Commit**: {0}' -f $commit)
    ('- **Machine**: {0}, {1} logical processors, {2} GiB RAM, {3}' -f $processor, [Environment]::ProcessorCount, $memoryGiB, $os)
    ('- **CSV**: `{0}`, {1} bytes' -f $Csv, (Format-Count $csvSize))
    ('- {0}' -f (Format-CsvMode $SourceSha256))
    ('- **Not met by CanopyDB** (`-NotMetByCanopyDb`): {0}' -f $associations)
    ('- **Work folder**: `{0}`' -f $WorkDir)
    ''
    '## Results'
    ''
    '| Criterion | Result | Evidence |'
    '|---|---|---|'
    $resultRows
    ''
    ($Sections -join "`n`n")
) -join "`n"
[System.IO.File]::WriteAllText($ReportPath, $report + "`n", (New-Object System.Text.UTF8Encoding $false))
Write-Step "Report written to $ReportPath"
Write-Host $overall
if ($failed.Count -gt 0) { exit 1 }
exit 0
