<#
.SYNOPSIS
    Acceptance of the crime generator on the reference CSV (child PRD, AC-G1 to AC-G10).

.DESCRIPTION
    Builds the generator and the independent verifier in release mode and, inside -WorkDir:

      Run A      a full generation into a fresh folder: exit code, wall time, the generator's own
                 peak memory line and an external PeakPagedMemorySize64 sample (AC-G2);
      Run B      a second full generation into another fresh folder: its six tables must be byte
                 identical to Run A's (AC-G10);
      verifier   crime-acceptance --reference on Run A (AC-G1, AC-G3, AC-G4, manifest);
      AC-G8      a rerun on Run A without --overwrite is refused and changes nothing;
      AC-G9      a CSV with an altered header aborts, the dataset stays intact and the log explains
                 the cause (also parent AC9, part of AC-G5);
      AC-G7      a real Ctrl+C ends a run with exit code 130, the exact operator message, no
                 temporary files and the dataset intact;
      AC-G6      a rerun with --overwrite killed with Stop-Process -Force during the merge leaves
                 the published dataset intact and unreferenced; the next run discards the leftovers
                 and completes (and so replaces the dataset: the second half of AC-G8);
      AC-G5      a CSV with a 21-column row, an invalid date and a repeated ID completes with 3
                 rejected (parent AC13).

    It writes acceptance-report.md to -WorkDir and exits with 1 when any check fails. The
    reference CSV is only read. Only the entries this script creates in -WorkDir are deleted when
    it starts.

.PARAMETER WorkDir
    Folder for the datasets, the logs and the report. It needs about 6 GB of free space.

.PARAMETER Csv
    The reference CSV. By default, chicago-crime-data.csv at the repository root.

.PARAMETER KillAfterSeconds
    When the run of AC-G6 is killed; 25 s falls in the merge of the primary table on the reference
    machine.

.PARAMETER CtrlCAfterSeconds
    When Ctrl+C is pressed in the run of AC-G7.

.PARAMETER SkipBuild
    Use the release binaries already built.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-acceptance.ps1 -WorkDir D:\acceptance
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [string]$Csv,
    [int]$KillAfterSeconds = 25,
    [int]$CtrlCAfterSeconds = 25,
    [switch]$SkipBuild
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Csv) { $Csv = Join-Path $RepoRoot 'chicago-crime-data.csv' }
$Csv = (Resolve-Path -LiteralPath $Csv).Path
$WorkDir = [System.IO.Path]::GetFullPath($WorkDir)
$GeneratorExe = Join-Path $RepoRoot 'target\release\crime-generator.exe'
$VerifierExe = Join-Path $RepoRoot 'target\release\crime-acceptance.exe'
$CtrlCHelper = Join-Path $PSScriptRoot 'send-ctrl-c.ps1'
$Invariant = [System.Globalization.CultureInfo]::InvariantCulture
$MemoryLimitBytes = 512MB
$TableFiles = @(
    'primary.sst', 'index-date.sst', 'index-primary_type.sst', 'index-district.sst',
    'index-community_area.sst', 'index-case_number.sst'
)

# Exact user-facing texts of the generator (PRDs).
$Messages = @{
    DatasetExists     = 'La carpeta de salida ya contiene un conjunto de datos. Usa la opción de sobrescritura para reemplazarlo.'
    Cancelled         = 'Generación cancelada. No se ha modificado el conjunto existente.'
    OperatorCancelled = 'Generación cancelada por el operador. No se ha modificado el conjunto existente.'
    Leftovers         = 'Se han descartado restos de una generación interrumpida anterior.'
    StaleSortDirs     = 'Se han descartado carpetas temporales de una generación interrumpida anterior.'
    HeaderMismatch    = 'La cabecera no coincide con el esquema: columna 5 esperaba "IUCR" y encontró "IUCR_ALTERADA".'
    ReferenceSummary  = 'Generación completada: 8.656.904 registros válidos, 0 rechazados (ver log).'
    Ac13Summary       = 'Generación completada: 999 registros válidos, 3 rechazados (ver log).'
    Ac13ColumnCount   = 'Fila 1001 omitida: se esperaban 22 columnas y hay 21.'
    Ac13InvalidDate   = 'Fila 1002 omitida, columna "Date": "02/30/2026 12:00:00 AM" no es una fecha válida (formato MM/DD/AAAA hh:mm:ss AM/PM).'
    Ac13DuplicateId   = 'Fila 1003 omitida: el ID 14348063 ya apareció en la fila 2.'
    MemoryWarning     = 'Aviso: el pico de memoria privada comprometida'
}

# --- Results and report ---

$Results = New-Object System.Collections.Generic.List[object]
$Sections = New-Object System.Collections.Generic.List[string]

function Add-Result([string]$Criterion, [bool]$Passed, [string]$Evidence) {
    $outcome = if ($Passed) { 'PASS' } else { 'FAIL' }
    $Results.Add([pscustomobject]@{ Criterion = $Criterion; Result = $outcome; Evidence = $Evidence })
    Write-Host ('{0}  {1}  {2}' -f $outcome, $Criterion, $Evidence)
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
# prove that nothing changed, byte for byte.
function Get-Snapshot([string]$Dir) {
    $base = (Get-Item -LiteralPath $Dir).FullName.TrimEnd('\')
    $lines = foreach ($item in Get-ChildItem -LiteralPath $Dir -Recurse -Force) {
        $relative = $item.FullName.Substring($base.Length).TrimStart('\')
        if ($item.PSIsContainer) { "$relative\" }
        else { '{0}|{1}|{2}' -f $relative, $item.Length, (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash }
    }
    (@($lines) | Sort-Object) -join "`n"
}

# The version folder that CURRENT names, as written by the generator: one name and a line feed.
function Get-CurrentName([string]$Output) {
    (Read-SharedText (Join-Path $Output 'CURRENT')).TrimEnd("`r", "`n")
}

function Get-TableHashes([string]$VersionDir) {
    $hashes = [ordered]@{}
    foreach ($file in $TableFiles) {
        $path = Join-Path $VersionDir $file
        $hashes[$file] = [pscustomobject]@{
            Size = (Get-Item -LiteralPath $path).Length
            Hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        }
    }
    $hashes
}

function Test-SameTables($Left, $Right) {
    foreach ($file in $TableFiles) {
        if ($Left[$file].Hash -ne $Right[$file].Hash) { return $false }
    }
    $true
}

# --- Processes ---

# Runs a program with its output captured. With -SampleMemory it samples PeakPagedMemorySize64
# (the peak private committed bytes) every 50 ms; with -KillAfterSeconds it kills the process with
# Stop-Process -Force at that moment, after listing the temporary and output folders.
function Invoke-Program {
    param(
        [string]$FilePath,
        [string[]]$Arguments,
        [switch]$SampleMemory,
        [int]$KillAfterSeconds = 0,
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
        if ($KillAfterSeconds -gt 0 -and $null -eq $atKill -and $watch.Elapsed.TotalSeconds -ge $KillAfterSeconds) {
            $atKill = [pscustomobject]@{
                Seconds       = $watch.Elapsed.TotalSeconds
                TempEntries   = @(Get-EntryNames $TempDir)
                OutputEntries = @(Get-EntryNames $OutputDir)
            }
            Stop-Process -Id $process.Id -Force
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

function Get-GeneratorArguments([string]$Source, [string]$Output, [string]$Log, [switch]$Overwrite) {
    $arguments = @('--input', $Source, '--output', $Output, '--temp-dir', $TempDir, '--log', $Log)
    if ($Overwrite) { $arguments += '--overwrite' }
    $arguments
}

function Invoke-Generator([string]$Source, [string]$Output, [string]$Log, [switch]$Overwrite, [int]$KillAfterSeconds = 0) {
    $arguments = Get-GeneratorArguments -Source $Source -Output $Output -Log $Log -Overwrite:$Overwrite
    Invoke-Program -FilePath $GeneratorExe -Arguments $arguments -SampleMemory `
        -KillAfterSeconds $KillAfterSeconds -TempDir $TempDir -OutputDir $Output
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

# --- Preparation ---

$started = Get-Date
foreach ($path in @($Csv, $CtrlCHelper)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing file: $path" }
}
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
$RunA = Join-Path $WorkDir 'run-a'
$RunB = Join-Path $WorkDir 'run-b'
$Ac13Output = Join-Path $WorkDir 'ac13-out'
$TempDir = Join-Path $WorkDir 'temp'
$LogDir = Join-Path $WorkDir 'logs'
$InputDir = Join-Path $WorkDir 'inputs'
$ReportPath = Join-Path $WorkDir 'acceptance-report.md'
Write-Step "Cleaning the entries of a previous acceptance in $WorkDir"
foreach ($path in @($RunA, $RunB, $Ac13Output, $TempDir, $LogDir, $InputDir, $ReportPath)) {
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
}
foreach ($dir in @($TempDir, $LogDir, $InputDir)) { New-Item -ItemType Directory -Path $dir | Out-Null }

$freeBytes = (Get-Item -LiteralPath $WorkDir).PSDrive.Free
if ($freeBytes -lt 6GB) { throw ('Only {0} free in {1}; about 6 GB are needed.' -f (Format-Mib $freeBytes), $WorkDir) }

if (-not $SkipBuild) {
    Write-Step 'Building crime-generator and crime-acceptance in release mode'
    $cargo = (Get-Command cargo).Source
    $build = Invoke-Program -FilePath $cargo -Arguments @('build', '--release', '-p', 'crime-generator', '-p', 'crime-acceptance')
    if ($build.ExitCode -ne 0) { throw "cargo build failed:`n$($build.Stderr)" }
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

# --- Run A and Run B: memory (AC-G2) and determinism (AC-G10) ---

$runs = [ordered]@{}
$tables = [ordered]@{}
foreach ($name in @('A', 'B')) {
    $output = if ($name -eq 'A') { $RunA } else { $RunB }
    $log = Join-Path $LogDir "run-$($name.ToLower()).log"
    Write-Step "Run $name into $output"
    $run = Invoke-Generator -Source $Csv -Output $output -Log $log
    $logText = Read-SharedText $log
    $run | Add-Member -NotePropertyName Log -NotePropertyValue $logText
    $run | Add-Member -NotePropertyName ReportedPeakBytes -NotePropertyValue (Get-ReportedPeakBytes $logText)
    $run | Add-Member -NotePropertyName TempAfter -NotePropertyValue @(Get-EntryNames $TempDir)
    $runs[$name] = $run
    if ($run.ExitCode -eq 0) { $tables[$name] = Get-TableHashes (Join-Path $output (Get-CurrentName $output)) }
    Write-Step ('Run {0}: exit {1}, {2}, external peak {3}' -f $name, $run.ExitCode, (Format-Seconds $run.WallSeconds), (Format-Mib $run.ExternalPeakBytes))
}

$firstRun = $runs['A']
$firstRunPeakLine = Get-LogLine $firstRun.Log 'Pico de memoria privada comprometida:*'
$memoryOk = $firstRun.ExitCode -eq 0 -and $firstRun.ExternalPeakBytes -gt 0 -and $firstRun.ExternalPeakBytes -le $MemoryLimitBytes `
    -and $null -ne $firstRun.ReportedPeakBytes -and $firstRun.ReportedPeakBytes -le $MemoryLimitBytes `
    -and -not $firstRun.Log.Contains($Messages.MemoryWarning)
Add-Result 'AC-G2' $memoryOk ('Run A exit {0} in {1}; summary line "{2}"; external PeakPagedMemorySize64 {3} ({4} bytes); limit 512 MiB; no memory warning in the log: {5}' -f `
        $firstRun.ExitCode, (Format-Seconds $firstRun.WallSeconds), $firstRunPeakLine, (Format-Mib $firstRun.ExternalPeakBytes), (Format-Count $firstRun.ExternalPeakBytes), (-not $firstRun.Log.Contains($Messages.MemoryWarning)))

$deterministic = $tables.Contains('A') -and $tables.Contains('B') -and (Test-SameTables $tables['A'] $tables['B'])
Add-Result 'AC-G10' $deterministic ('the six tables of Run B are byte-identical to Run A (SHA-256 per file); Run B exit {0} in {1}' -f `
        $runs['B'].ExitCode, (Format-Seconds $runs['B'].WallSeconds))

$runRows = foreach ($name in $runs.Keys) {
    $run = $runs[$name]
    $reported = if ($null -ne $run.ReportedPeakBytes) { Format-Mib $run.ReportedPeakBytes } else { 'missing' }
    '| {0} | {1} | {2} | {3} | {4} ({5} bytes) | {6} |' -f $name, $run.ExitCode, (Format-Seconds $run.WallSeconds), $reported, `
        (Format-Mib $run.ExternalPeakBytes), (Format-Count $run.ExternalPeakBytes), $(if ($run.TempAfter.Count -eq 0) { 'empty' } else { $run.TempAfter -join ', ' })
}
$hashRows = foreach ($file in $TableFiles) {
    $a = if ($tables.Contains('A')) { $tables['A'][$file] } else { $null }
    $b = if ($tables.Contains('B')) { $tables['B'][$file] } else { $null }
    $sizeText = if ($a) { Format-Count $a.Size } else { '-' }
    $hashText = if ($a) { $a.Hash } else { '-' }
    $same = $a -and $b -and $a.Hash -eq $b.Hash
    '| {0} | {1} | `{2}` | {3} |' -f $file, $sizeText, $hashText, $(if ($same) { 'yes' } else { 'NO' })
}
Add-Section (@(
        '## Run A and Run B'
        ''
        '| Run | Exit | Wall time | Peak reported by the generator | External PeakPagedMemorySize64 | Temp folder after |'
        '|---|---|---|---|---|---|'
        $runRows
        ''
        '### Tables (SHA-256)'
        ''
        '| File | Size (bytes) | SHA-256 of Run A | Run B identical |'
        '|---|---|---|---|'
        $hashRows
        ''
        '### Run A log'
        ''
        (Format-Code $firstRun.Log)
    ) -join "`n")

# --- The independent verifier on Run A: AC-G1, AC-G3, AC-G4 ---

Write-Step 'Independent verifier on Run A'
$verifier = Invoke-Program -FilePath $VerifierExe -Arguments @('--csv', $Csv, '--dataset', $RunA, '--reference')
$verdicts = @{}
foreach ($line in Get-Lines $verifier.Stdout) {
    $match = [regex]::Match($line, '^(PASS|FAIL)  (\S+)  (.*)$')
    if ($match.Success) {
        $verdicts[$match.Groups[2].Value] = [pscustomobject]@{ Passed = $match.Groups[1].Value -eq 'PASS'; Summary = $match.Groups[3].Value }
    }
}
function Get-VerdictResult([string[]]$Criteria) {
    $passed = $true
    $parts = foreach ($criterion in $Criteria) {
        if ($verdicts.ContainsKey($criterion)) {
            $passed = $passed -and $verdicts[$criterion].Passed
            '{0} {1}: {2}' -f $criterion, $(if ($verdicts[$criterion].Passed) { 'PASS' } else { 'FAIL' }), $verdicts[$criterion].Summary
        }
        else {
            $passed = $false
            "$criterion missing"
        }
    }
    [pscustomobject]@{ Passed = $passed; Evidence = ($parts -join ' / ') }
}

$summaryLine = $firstRun.Log.Contains($Messages.ReferenceSummary)
$g1 = Get-VerdictResult @('AC-G1')
Add-Result 'AC-G1' ($g1.Passed -and $summaryLine) ('{0} / Run A summary "{1}": {2}' -f $g1.Evidence, $Messages.ReferenceSummary, $summaryLine)
$g3 = Get-VerdictResult @('AC-G3', 'AC-G3-SAMPLE', 'AC-G3-DST')
Add-Result 'AC-G3' $g3.Passed $g3.Evidence
$g4 = Get-VerdictResult @('AC-G4-date', 'AC-G4-primary_type', 'AC-G4-district', 'AC-G4-community_area', 'AC-G4-case_number', 'AC-G4-THEFT', 'AC-G4-COMMUNITY-NIL', 'AC-G4-SHARED-CASES')
Add-Result 'AC-G4' $g4.Passed $g4.Evidence
$manifest = Get-VerdictResult @('LAYOUT', 'MANIFEST-SCHEMA', 'CSV', 'MANIFEST-SOURCE', 'MANIFEST-TABLES')
Add-Result 'Manifest' $manifest.Passed $manifest.Evidence
Add-Section (@(
        '## Independent verifier on Run A'
        ''
        ('`crime-acceptance --csv <CSV> --dataset run-a --reference`: exit {0}.' -f $verifier.ExitCode)
        ''
        (Format-Code $verifier.Stdout)
        ''
        'Progress (standard error):'
        ''
        (Format-Code $verifier.Stderr)
    ) -join "`n")

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
$firstLines[0] = $firstLines[0].Replace('"IUCR"', '"IUCR_ALTERADA"')
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

# --- AC-G6: a hard kill during the merge, then a normal run ---

Write-Step "AC-G6: Stop-Process -Force after $KillAfterSeconds s"
$currentBefore = Get-CurrentName $RunA
$versionBefore = Get-Snapshot (Join-Path $RunA $currentBefore)
$killLog = Join-Path $LogDir 'ac-g6-killed.log'
$killed = Invoke-Generator -Source $Csv -Output $RunA -Log $killLog -Overwrite -KillAfterSeconds $KillAfterSeconds
$killText = Read-SharedText $killLog
$phase = @(Get-Lines $killText)[-1]
$currentAfterKill = Get-CurrentName $RunA
$versionAfterKill = Get-Snapshot (Join-Path $RunA $currentAfterKill)
$outputAfterKill = @(Get-EntryNames $RunA)
$tempAfterKill = @(Get-EntryNames $TempDir)
$unreferenced = @($outputAfterKill | Where-Object { $_ -like 'dataset-*' -and $_ -ne $currentBefore })
$intact = $null -ne $killed.AtKill -and $currentAfterKill -eq $currentBefore -and $versionAfterKill -eq $versionBefore
Add-Result 'AC-G6 (kill)' $intact ('killed at {0} with exit {1}; last log line then: "{2}"; output then: {3}; afterwards CURRENT still names {4} and its files are byte-identical: {5}; unreferenced version folders left: {6}; sort folders left: {7}' -f `
        $(if ($killed.AtKill) { Format-Seconds $killed.AtKill.Seconds } else { 'never (the run ended first)' }), $killed.ExitCode, $phase, `
    $(if ($killed.AtKill) { $killed.AtKill.OutputEntries -join ', ' } else { '-' }), $currentBefore, $intact, ($unreferenced -join ', '), ($tempAfterKill -join ', '))

Write-Step 'AC-G6: next run with --overwrite'
$recoveryLog = Join-Path $LogDir 'ac-g6-next-run.log'
$recovery = Invoke-Generator -Source $Csv -Output $RunA -Log $recoveryLog -Overwrite
$recoveryText = Read-SharedText $recoveryLog
$currentAfter = if ($recovery.ExitCode -eq 0) { Get-CurrentName $RunA } else { $currentBefore }
$outputAfter = @(Get-EntryNames $RunA)
$tempAfter = @(Get-EntryNames $TempDir)
$recoveredTables = if ($recovery.ExitCode -eq 0) { Get-TableHashes (Join-Path $RunA $currentAfter) } else { $null }
$discardedBoth = $recoveryText.Contains($Messages.StaleSortDirs) -and $recoveryText.Contains($Messages.Leftovers)
$clean = $tempAfter.Count -eq 0 -and $outputAfter.Count -eq 2 -and ($outputAfter -contains 'CURRENT') -and ($outputAfter -contains $currentAfter)
$recovered = $recovery.ExitCode -eq 0 -and $discardedBoth -and $clean -and $tempAfterKill.Count -gt 0 -and $unreferenced.Count -gt 0
Add-Result 'AC-G6 (next run)' $recovered ('exit {0} in {1}; log reads "{2}" and "{3}": {4}; afterwards the output holds {5} and the temp folder {6}' -f `
        $recovery.ExitCode, (Format-Seconds $recovery.WallSeconds), $Messages.StaleSortDirs, $Messages.Leftovers, $discardedBoth, `
    ($outputAfter -join ', '), $(if ($tempAfter.Count -eq 0) { 'is empty' } else { 'holds ' + ($tempAfter -join ', ') }))
$replaced = $recovery.ExitCode -eq 0 -and $currentAfter -ne $currentBefore -and -not ($outputAfter -contains $currentBefore) `
    -and $null -ne $recoveredTables -and $tables.Contains('A') -and (Test-SameTables $tables['A'] $recoveredTables)
Add-Result 'AC-G8 (replacement)' $replaced ('with --overwrite, CURRENT moved from {0} to {1}, the previous version is gone, and the new tables are byte-identical to Run A: {2}' -f `
        $currentBefore, $currentAfter, $replaced)
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
$sample = $firstLines[1]
$badRows = @(
    # 21 columns: the sample row with a new ID and without Location.
    ($sample.Replace('"14348063"', '"99000001"') -replace ',"\([^"]*\)"$', '')
    # An invalid date with a new ID.
    $sample.Replace('"14348063"', '"99000002"').Replace('"10/03/2026 12:00:00 AM"', '"02/30/2026 12:00:00 AM"')
    # The ID of line 2 again.
    $sample
)
$ac13Csv = Join-Path $InputDir 'ac13.csv'
$ac13Lines = @(Get-Content -LiteralPath $Csv -TotalCount 1000 -Encoding UTF8) + $badRows
[System.IO.File]::WriteAllText($ac13Csv, (($ac13Lines -join "`n") + "`n"), (New-Object System.Text.UTF8Encoding $false))
$ac13Log = Join-Path $LogDir 'ac-g5-ac13.log'
$ac13 = Invoke-Generator -Source $ac13Csv -Output $Ac13Output -Log $ac13Log
$ac13Text = Read-SharedText $ac13Log
$ac13Manifest = if ($ac13.ExitCode -eq 0) {
    Get-Content -LiteralPath (Join-Path (Join-Path $Ac13Output (Get-CurrentName $Ac13Output)) 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
}
else { $null }
$rowsLogged = $ac13Text.Contains($Messages.Ac13ColumnCount) -and $ac13Text.Contains($Messages.Ac13InvalidDate) -and $ac13Text.Contains($Messages.Ac13DuplicateId)
$manifestCounts = $null -ne $ac13Manifest -and $ac13Manifest.records.valid -eq 999 -and $ac13Manifest.records.rejected -eq 3
Add-Result 'AC-G5 (parent AC13)' ($ac13.ExitCode -eq 0 -and $rowsLogged -and $ac13Text.Contains($Messages.Ac13Summary) -and $manifestCounts) `
('exit {0}; the log names each row ({1}); summary "{2}"; manifest valid {3}, rejected {4}' -f $ac13.ExitCode, $rowsLogged, $Messages.Ac13Summary, `
    $(if ($ac13Manifest) { $ac13Manifest.records.valid } else { '-' }), $(if ($ac13Manifest) { $ac13Manifest.records.rejected } else { '-' }))
Add-Section (@('## AC-G5 (parent AC13): log', '', (Format-Code $ac13Text)) -join "`n")

# --- Report ---

$failed = @($Results | Where-Object { $_.Result -ne 'PASS' })
$resultRows = foreach ($result in $Results) {
    '| {0} | {1} | {2} |' -f $result.Criterion, $result.Result, $result.Evidence.Replace('|', '\|')
}
$overall = if ($failed.Count -eq 0) { "PASS: all $($Results.Count) checks passed" } else { "FAIL: $($failed.Count) of $($Results.Count) checks failed" }
$report = @(
    '# Acceptance of the crime generator'
    ''
    ('- **Result**: {0}' -f $overall)
    ('- **Date**: {0:yyyy-MM-dd HH:mm} (duration {1})' -f $started, (Format-Seconds ((Get-Date) - $started).TotalSeconds))
    ('- **Commit**: {0}' -f $commit)
    ('- **Machine**: {0}, {1} logical processors, {2} GiB RAM, {3}' -f $processor, [Environment]::ProcessorCount, $memoryGiB, $os)
    ('- **CSV**: `{0}`, {1} bytes' -f $Csv, (Format-Count $csvSize))
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
