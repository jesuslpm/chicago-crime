<#
.SYNOPSIS
    Tests of the comparison bench (PRD canopy-db-comparacion): AC-KM1 to AC-KM4 and the rules of its
    report and data table, with fake applications.

.DESCRIPTION
    Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\acceptance\tests\canopy-comparison.tests.ps1
    The fakes write what the real applications write (the generator's summary in its log, the query
    apps' stats lines, batch headers and outputs), so the bench runs end to end without building or
    generating anything. Each test runs in its own temporary repository folder, deleted at the end.
    Ends with 0 when every test passes and 1 otherwise.
#>
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot '..\canopy-comparison.ps1')

$RealRepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$Failures = New-Object System.Collections.Generic.List[string]
$TempRoots = New-Object System.Collections.Generic.List[string]
$FakeQueries = @((Read-FixedQueries (Join-Path $PSScriptRoot '..\canopy-fixed-queries.txt') 'JA100').Queries)
$FakeCounts = @{ F1 = 1; F2 = 2; F3 = 100; F4 = 3; F5 = 50; F6 = 40; F7 = 30; F8 = 5; F9 = 20; F10 = 3; F11 = 3 }
$Fake = $null
$CurrentTest = ''

# --- Assertions ---

function Assert-True([bool]$Condition, [string]$What) { if (-not $Condition) { $Failures.Add("${CurrentTest}: $What") } }

function Assert-Equal($Expected, $Actual, [string]$What) {
    if ([string]$Expected -cne [string]$Actual) { $Failures.Add(('{0}: {1}: expected <{2}>, got <{3}>' -f $CurrentTest, $What, $Expected, $Actual)) }
}

function Assert-Contains([string[]]$Lines, [string]$Line, [string]$What) {
    Assert-True ($Lines -ccontains $Line) "$What`: no line <$Line>"
}

function Test-Case([string]$Name, [scriptblock]$Body) {
    $script:CurrentTest = $Name
    $before = $Failures.Count
    try { & $Body }
    catch { $Failures.Add(('{0}: threw "{1}"`n{2}' -f $Name, $_.Exception.Message, $_.ScriptStackTrace)) }
    Write-Host ('{0}  {1}' -f $(if ($Failures.Count -eq $before) { 'PASS' } else { 'FAIL' }), $Name)
}

# --- Fakes ---

# A temporary repository: the four applications (empty files) and a CSV.
function New-FakeRepo {
    $root = Join-Path ([System.IO.Path]::GetTempPath()) ('canopy-comparison-test-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path (Join-Path $root 'target\release') | Out-Null
    foreach ($exe in (Get-ComparisonExecutables $root).Values) { [System.IO.File]::WriteAllText($exe, '') }
    $csv = Join-Path $root 'small.csv'
    [System.IO.File]::WriteAllText($csv, "x`n")
    $TempRoots.Add($root)
    [pscustomobject]@{ Root = $root; Csv = $csv; WorkDir = (Join-Path $root 'work'); Reports = (Join-Path $root 'docs\canopy-db\comparisons') }
}

function New-FakeEnvironment([hashtable]$Behavior = @{}) {
    $script:Fake = @{ Behavior = $Behavior; Calls = New-Object System.Collections.Generic.List[object]; Counters = @{}; Clock = $(if ($Behavior.ContainsKey('Clock')) { [datetime]$Behavior.Clock } else { [datetime]'2026-10-22T12:00:00' }) }
    @{
        Now        = { $script:Fake.Clock = $script:Fake.Clock.AddSeconds(1); $script:Fake.Clock }
        Run        = { param($FilePath, $Arguments, $StdoutPath) Invoke-FakeProgram $FilePath $Arguments $StdoutPath }
        Build      = { param($Root) $script:Fake.Calls.Add('build'); if ($script:Fake.Behavior.ContainsKey('Build')) { $script:Fake.Behavior.Build } else { [pscustomobject]@{ ExitCode = 0; Stderr = '' } } }
        FreeBytes  = { param($Path) if ($script:Fake.Behavior.ContainsKey('FreeBytes')) { $script:Fake.Behavior.FreeBytes } else { [long]100GB } }
        Git        = { param($Root) if ($script:Fake.Behavior.ContainsKey('Git')) { $script:Fake.Behavior.Git } else { [pscustomobject]@{ Commit = 'abc1234'; Dirty = $false } } }
        Machine    = { 'Fake CPU, 4 logical processors, 8 GiB RAM, Fake OS' }
        SharedCase = { param($Path) if ($script:Fake.Behavior.ContainsKey('SharedCase')) { $script:Fake.Behavior.SharedCase } else { 'JA100' } }
        Sha256     = { param($Path) 'f' * 64 }
    }
}

function Get-FakeCounter([string]$Key) {
    $value = if ($Fake.Counters.ContainsKey($Key)) { $Fake.Counters[$Key] } else { 0 }
    $Fake.Counters[$Key] = $value + 1
    $value
}

function Get-FakeOverride([string]$Name, [hashtable]$Context) {
    if ($Fake.Behavior.ContainsKey($Name)) { & $Fake.Behavior[$Name] $Context } else { $null }
}

function Write-FakeText([string]$Path, [string]$Text) { [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding $false)) }

function Invoke-FakeProgram([string]$FilePath, [string[]]$Arguments, [string]$StdoutPath) {
    $Fake.Calls.Add($FilePath)
    $name = [System.IO.Path]::GetFileNameWithoutExtension($FilePath)
    $pair = if ($name -like '*canopy*') { 'canopydb' } else { 'sstable' }
    if ($name -like '*generator') { Invoke-FakeGenerator $pair $Arguments } else { Invoke-FakeQuery $pair $Arguments $StdoutPath }
}

# A generation: the dataset folder with its manifest, and the final summary in the log.
function Invoke-FakeGenerator([string]$Pair, [string[]]$Arguments) {
    $run = (Get-FakeCounter "generation-$Pair") + 1
    $output = $Arguments[[array]::IndexOf($Arguments, '--output') + 1]
    $log = $Arguments[[array]::IndexOf($Arguments, '--log') + 1]
    $override = Get-FakeOverride 'Generation' @{ Pair = $Pair; Run = $run }
    if ($null -eq $override) { $override = @{} }
    if ($override.ContainsKey('ExitCode')) {
        return [pscustomobject]@{ ExitCode = $override.ExitCode; ProcessSeconds = 1; PeakBytes = 1MB; Stderr = 'Generación cancelada. No se ha modificado el conjunto existente.' }
    }
    $peak = if ($override.ContainsKey('Peak')) { $override.Peak } else { '93,4' }
    $valid = if ($override.ContainsKey('Valid')) { $override.Valid } else { '8.656.904' }
    $version = Join-Path $output 'dataset-1'
    New-Item -ItemType Directory -Force -Path $version | Out-Null
    Write-FakeText (Join-Path $output 'CURRENT') "dataset-1`n"
    $manifest = if ($Pair -eq 'sstable') { '{"source":{"sha256":"' + ('a' * 64) + '"}}' } else { '{"engine":{"name":"canopydb","version":"0.2.5","fork_commit":null}}' }
    Write-FakeText (Join-Path $version 'manifest.json') $manifest
    Write-FakeText $log ((@(
                "Generación completada: $valid registros válidos, 0 rechazados (ver log)."
                "Duración: 1 min 2,$run s."
                "Pico de memoria privada comprometida: $peak MiB (límite: 512 MiB)."
                "Conjunto de datos publicado en $version."
                'Archivo a.sst: 1.000 bytes (0,1 MiB).'
                'Archivo manifest.json: 234 bytes (0,1 MiB).'
                "Log de la ejecución: $log."
            ) -join "`n") + "`n")
    [pscustomobject]@{ ExitCode = 0; ProcessSeconds = 62; PeakBytes = 100MB; Stderr = '' }
}

function Get-FakeCount([string]$Pair, [string]$Id, [int]$Run, [string]$Mode) {
    $override = Get-FakeOverride 'Count' @{ Pair = $Pair; Query = $Id; Run = $Run; Mode = $Mode }
    if ($null -ne $override) { $override } else { $FakeCounts[$Id] }
}

function Get-FakeOffset([string]$Pair, [string]$Id) {
    $override = Get-FakeOverride 'IdOffset' @{ Pair = $Pair; Query = $Id }
    if ($null -ne $override) { [int]$override } else { 0 }
}

# What a query writes on its standard output, as the real query app writes it.
function Get-FakeOutput($Query, [int]$Count, [int]$Offset = 0) {
    switch ($Query.Kind) {
        'count' { "$Count`n" }
        'json' {
            if ($Count -eq 0) { return '{"query":{"id":"' + $Query.Id + '"},"dataset":{"version":"v"},"results":[]}' + "`n" }
            $records = @(1..$Count | ForEach-Object { '{"ID":' + ($_ + $Offset) + '}' }) -join ",`n"
            '{"query":{"id":"' + $Query.Id + '"},"dataset":{"version":"v"},"results":[' + "`n" + $records + "`n]}`n"
        }
        default { (@('"ID","Case Number"') + @(1..$Count | ForEach-Object { "$($_ + $Offset),C$_" }) -join "`n") + "`n" }
    }
}

function Format-FakeStats([int]$Count, [string]$Seconds) {
    '{0} {1} en {2} s. Índices usados: ID. Filtros sin índice: ninguno.' -f $Count, $(if ($Count -eq 1) { 'resultado' } else { 'resultados' }), $Seconds
}

function Invoke-FakeQuery([string]$Pair, [string[]]$Arguments, [string]$StdoutPath) {
    if ($Arguments -ccontains '--batch') {
        $run = Get-FakeCounter "batch-$Pair"
        $override = Get-FakeOverride 'Batch' @{ Pair = $Pair; Run = $run }
        if ($null -eq $override) { $override = @{} }
        $stderr = New-Object System.Collections.Generic.List[string]
        $stdout = New-Object System.Text.StringBuilder
        $exitCode = 0
        for ($i = 0; $i -lt $FakeQueries.Count; $i++) {
            $query = $FakeQueries[$i]
            $stderr.Add(('--- Consulta {0} de {1} (línea {2}) ---' -f ($i + 1), $FakeQueries.Count, $query.LineNumber))
            if ($override.ContainsKey('StopAt') -and $override.StopAt -eq $query.Id) {
                $stderr.Add('No se pudo leer el conjunto de datos en x.')
                $exitCode = 1
                break
            }
            $count = Get-FakeCount $Pair $query.Id $run 'batch'
            if (-not ($override.ContainsKey('NoStatsFor') -and $override.NoStatsFor -eq $query.Id)) { $stderr.Add((Format-FakeStats $count '0,041')) }
            [void]$stdout.Append((Get-FakeOutput $query $count (Get-FakeOffset $Pair $query.Id)))
        }
        if ($exitCode -eq 0 -and -not $override.ContainsKey('NoFinal')) { $stderr.Add('Lote completado: 11 consultas en 0,500 s.') }
        Write-FakeText $StdoutPath $stdout.ToString()
        $peak = if ($override.ContainsKey('PeakBytes')) { $override.PeakBytes } else { [long]12MB }
        return [pscustomobject]@{ ExitCode = $exitCode; ProcessSeconds = 0.6; PeakBytes = $peak; Stderr = ($stderr -join "`n") }
    }
    $words = @($Arguments | Select-Object -Skip 2) -join [char]0x1F
    $query = @($FakeQueries | Where-Object { ($_.Arguments -join [char]0x1F) -ceq $words })[0]
    $run = Get-FakeCounter "single-$Pair-$($query.Id)"
    $override = Get-FakeOverride 'Single' @{ Pair = $Pair; Query = $query.Id; Run = $run }
    if ($null -eq $override) { $override = @{} }
    if ($override.ContainsKey('ExitCode')) {
        return [pscustomobject]@{ ExitCode = $override.ExitCode; ProcessSeconds = 0.1; PeakBytes = [long]10MB; Stderr = $override.Stderr }
    }
    $count = Get-FakeCount $Pair $query.Id $run 'single'
    Write-FakeText $StdoutPath (Get-FakeOutput $query $count (Get-FakeOffset $Pair $query.Id))
    $stderr = if ($override.ContainsKey('NoStats')) { '' } else { Format-FakeStats $count '0,041' }
    [pscustomobject]@{
        ExitCode       = 0
        ProcessSeconds = $(if ($override.ContainsKey('ProcessSeconds')) { $override.ProcessSeconds } else { 0.1 })
        PeakBytes      = $(if ($override.ContainsKey('PeakBytes')) { $override.PeakBytes } else { [long]10MB })
        Stderr         = $stderr
    }
}

# Runs the bench on a fake repository; returns its exit code, report lines, data table and paths.
function Invoke-FakeBench($Repo, [hashtable]$Behavior = @{}, [int]$Repetitions = 2, [string]$Baseline, [switch]$SkipBuild) {
    $environment = New-FakeEnvironment $Behavior
    $before = @(Get-ChildItem -LiteralPath $Repo.Reports -Filter '*.md' -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
    $code = Invoke-CanopyComparison -RepoRoot $Repo.Root -WorkDir $Repo.WorkDir -Csv $Repo.Csv -Repetitions $Repetitions -Baseline $Baseline -SkipBuild:$SkipBuild -Environment $environment 6>$null
    $report = @(Get-ChildItem -LiteralPath $Repo.Reports -Filter '*.md' | Where-Object { $before -notcontains $_.FullName })[0].FullName
    $data = [System.IO.Path]::ChangeExtension($report, '.csv')
    [pscustomobject]@{
        Code       = $code
        Report     = $report
        Data       = $data
        Lines      = @([System.IO.File]::ReadAllText($report) -split "`n")
        DataText   = [System.IO.File]::ReadAllText($data)
        DataBytes  = [System.IO.File]::ReadAllBytes($data)
        Rows       = Read-ComparisonData $data
    }
}

function Get-Message([scriptblock]$Body) { try { & $Body; $null } catch { $_.Exception.Message } }

function Get-TableRow([string[]]$Lines, [string]$Prefix) { @($Lines | Where-Object { $_.StartsWith($Prefix) })[0] }

$None = [string][char]0x2014

# --- Tests ---

Test-Case 'AC-M2 shape: two repetitions, same results, header, sections and exit 0' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Single = { param($c) if ($c.Pair -eq 'sstable' -and $c.Query -eq 'F1') { @{ ProcessSeconds = @(0.5, 0.041, 0.042)[$c.Run] } } } }
    Assert-Equal 0 $run.Code 'exit code'
    Assert-Equal '2026-10-22-120001-abc1234.md' ([System.IO.Path]::GetFileName($run.Report)) 'report name'
    Assert-Equal '# CanopyDB comparison bench' $run.Lines[0] 'title'
    Assert-Contains $run.Lines '- **Result**: PASS: 76 runs, same results in all 11 queries' 'result'
    Assert-Contains $run.Lines '- **Date**: 2026-10-22 12:00 (duration 1.0 s)' 'date'
    Assert-Contains $run.Lines '- **Commit**: abc1234' 'commit'
    Assert-Contains $run.Lines '- **Machine**: Fake CPU, 4 logical processors, 8 GiB RAM, Fake OS' 'machine'
    Assert-Contains $run.Lines ('- **CSV**: `{0}`, 2 bytes' -f $repo.Csv) 'csv'
    Assert-Contains $run.Lines ('- CSV: not the reference extract (SHA-256 {0}).' -f ('a' * 64)) 'csv mode'
    Assert-Contains $run.Lines '- **CanopyDB**: 0.2.5 (fork none)' 'canopydb'
    Assert-Contains $run.Lines '- **CanopyDB configuration**: generator: page_cache_size=unknown; query app: page_cache_size=unknown' 'settings'
    Assert-Contains $run.Lines ('- **Options**: 2 repetitions, compression snappy (SSTables) and lz4 (CanopyDB), temp folder `{0}`' -f (Join-Path $repo.WorkDir 'comparison-temp')) 'options'
    Assert-Contains $run.Lines '- **Build**: release build' 'build'
    Assert-Contains $run.Lines '- **Baseline**: none' 'baseline'
    Assert-Contains $run.Lines ('- **Work folder**: `{0}`' -f $repo.WorkDir) 'work folder'
    $headings = @($run.Lines | Where-Object { $_.StartsWith('## ') })
    Assert-Equal '## Generation|## Results|## Single queries|## Batch|## Notes' ($headings -join '|') 'sections'
    Assert-Equal 11 @($run.Lines | Where-Object { $_ -match '^\| F[0-9]+ \| `.*\| SAME \|$' }).Count 'SAME rows'
    Assert-Contains $run.Lines '| F2 | `--where "Case Number=JA100" --format json` | 2 | 2 | 2 | 2 | SAME |' 'F2 row with the case number'
    Assert-Contains $run.Lines '| SSTables | 2 of 2 | 62.150 s | 62.100 s | 62.200 s | 93.4 MiB | 1234 bytes (0.0 MiB) | 8656904 | 0 | 0 |' 'generation row'
    Assert-Contains $run.Lines '| F1 | SSTables | 2 of 2 | 0.041 s | 0.042 s | 0.500 s | 10.0 MiB | 0 |' 'median of two, ties away from zero'
    Assert-Contains $run.Lines '| Runs | 2 of 2 | 2 of 2 |' 'batch runs'
    Assert-Contains $run.Lines '| Batch total median | 0.500 s | 0.500 s |' 'batch total'
    Assert-Contains $run.Lines '| Peak first run | 12.0 MiB | 12.0 MiB |' 'batch first peak'
    Assert-Equal 'none' $run.Lines[-2] 'notes'
}

Test-Case 'AC-KM4: the data table has one row per measure of every run, LF, no BOM, and the medians come from it' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo
    Assert-Equal 'phase,pair,query,run,exit_code,metric,value' (($run.DataText -split "`n")[0]) 'header'
    Assert-True ($run.DataBytes[0] -ne 0xEF) 'no BOM'
    Assert-True (-not $run.DataText.Contains("`r")) 'LF only'
    Assert-True ($run.DataText.EndsWith("`n")) 'final newline'
    Assert-Equal (4 * 5 + 66 * 4 + 6 * 25) $run.Rows.Count 'rows'
    Assert-Equal 'generation,sstable,,1,0,duration_s,62.100' (($run.DataText -split "`n")[1]) 'first row'
    Assert-Equal 'single,sstable,F1,0,0,query_s,0.041' (($run.DataText -split "`n")[21]) 'first single row'
    $batch = @($run.Rows | Where-Object { $_.phase -eq 'batch' -and $_.pair -eq 'sstable' -and $_.run -eq 0 })
    Assert-Equal 'F1 query_s|F1 results|F2 query_s' ((@($batch | Select-Object -First 3 | ForEach-Object { "$($_.query) $($_.metric)" })) -join '|') 'batch row order'
    Assert-Equal ' batch_total_s| process_s| peak_mib' ((@($batch | Select-Object -Last 3 | ForEach-Object { "$($_.query) $($_.metric)" })) -join '|') 'batch totals last'
    Assert-Equal 2 @($run.Rows | Where-Object { $_.phase -eq 'single' -and $_.pair -eq 'canopydb' -and $_.query -eq 'F3' -and $_.metric -eq 'results' -and $_.run -ge 1 }).Count 'runs 1 to R'
}

Test-Case 'AC-KM4: with -Repetitions 1 the median is the single run and nothing is out of tolerance' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo -Repetitions 1 -Behavior @{ Single = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F4') { @{ ProcessSeconds = @(0.3, 0.7)[$c.Run] } } } }
    Assert-Contains $run.Lines '- **Result**: PASS: 50 runs, same results in all 11 queries' 'result'
    Assert-Contains $run.Lines '| F4 | CanopyDB | 1 of 1 | 0.041 s | 0.700 s | 0.300 s | 10.0 MiB | 0 |' 'single row'
    Assert-Contains $run.Lines '| SSTables | 1 of 1 | 62.100 s | 62.100 s | 62.100 s | 93.4 MiB | 1234 bytes (0.0 MiB) | 8656904 | 0 | 0 |' 'generation row'
}

Test-Case 'Out of tolerance counts the repetitions away from the median of their series' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo -Repetitions 3 -Behavior @{ Single = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F9') { @{ ProcessSeconds = @(0.1, 1.0, 1.0, 2.0)[$c.Run] } } } }
    Assert-Contains $run.Lines '| F9 | CanopyDB | 3 of 3 | 0.041 s | 1.000 s | 0.100 s | 10.0 MiB | 1 |' 'one repetition out'
}

Test-Case 'AC-KM1: different numbers of results make the query DIFFERENT and the bench end with 1' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Count = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F7') { 31 } } }
    Assert-Equal 1 $run.Code 'exit code'
    Assert-Contains $run.Lines '- **Result**: FAIL: 0 of 76 runs failed, 1 of 11 queries returned different results' 'result'
    Assert-Contains $run.Lines '| F7 | `--where "Primary Type=THEFT" --where District=008 --count` | 30 | 31 | 30 | 31 | DIFFERENT |' 'F7 row'
    Assert-Equal 10 @($run.Lines | Where-Object { $_ -match '^\| F[0-9]+ \| `.*\| SAME \|$' }).Count 'other rows SAME'
    Assert-Contains $run.Lines '| F7 | CanopyDB | 2 of 2 | 0.041 s | 0.100 s | 0.100 s | 10.0 MiB | 0 |' 'figures still measured'
}

Test-Case 'AC-KM1: the same counts with different records are DIFFERENT too' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ IdOffset = { param($c) if ($c.Pair -eq 'canopydb' -and ($c.Query -eq 'F1' -or $c.Query -eq 'F11')) { 10 } } }
    Assert-Equal 1 $run.Code 'exit code'
    Assert-Contains $run.Lines '- **Result**: FAIL: 0 of 76 runs failed, 3 of 11 queries returned different results' 'result'
    Assert-Contains $run.Lines '| F1 | `--where ID=634 --format json` | 1 | 1 | 1 | 1 | DIFFERENT |' 'F1: other JSON records'
    Assert-Contains $run.Lines '| F11 | `--where "Primary Type=THEFT" --format csv` | 3 | 3 | 3 | 3 | DIFFERENT |' 'F11: other rows'
    Assert-Contains $run.Lines '| F10 | `--where "Primary Type=THEFT" --limit 1000 --format csv` | 3 | 3 | 3 | 3 | DIFFERENT |' 'F10: records outside its F11'
}

Test-Case 'AC-KM2: a failed run shows in Notes and the data table, stays out of the medians, and the bench ends with 1' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Single = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F5' -and $c.Run -eq 2) { @{ ExitCode = 3; Stderr = "first`nboom" } } } }
    Assert-Equal 1 $run.Code 'exit code'
    Assert-Contains $run.Lines '- **Result**: FAIL: 1 of 76 runs failed, 0 of 11 queries returned different results' 'result'
    Assert-Contains $run.Lines '- Failed run: single CanopyDB F5 run 2: exit code 3: boom.' 'note'
    Assert-Equal '3|3|3|3' ((@($run.Rows | Where-Object { $_.phase -eq 'single' -and $_.pair -eq 'canopydb' -and $_.query -eq 'F5' -and $_.run -eq 2 } | ForEach-Object { $_.exit_code })) -join '|') 'rows with its code'
    Assert-Contains $run.Lines '| F5 | CanopyDB | 1 of 2 | 0.041 s | 0.100 s | 0.100 s | 10.0 MiB | 0 |' 'one run left'
    Assert-Contains $run.Lines '| F5 | `--range Date=2019-08-01..2019-08-31 --count` | 50 | 50 | 50 | 50 | SAME |' 'check'
}

Test-Case 'AC-KM2: when every generation of a pair fails no query runs' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Generation = { param($c) if ($c.Pair -eq 'canopydb') { @{ ExitCode = 1 } } } }
    Assert-Equal 1 $run.Code 'exit code'
    Assert-Contains $run.Lines '- **Result**: FAIL: 2 of 4 runs failed, queries not run' 'result'
    Assert-Equal 11 @($run.Lines | Where-Object { $_ -match '^\| F[0-9]+ \| `.*\| NOT RUN \|$' }).Count 'NOT RUN rows'
    Assert-Contains $run.Lines "| F1 | SSTables | 0 of 2 | $None | $None | $None | $None | 0 |" 'single row without queries'
    Assert-Contains $run.Lines "| Runs | 0 of 2 | 0 of 2 |" 'batch runs'
    Assert-Contains $run.Lines "| CanopyDB | 0 of 2 | $None | $None | $None | $None | $None | $None | $None | 0 |" 'generation row'
    Assert-Contains $run.Lines '- **CanopyDB**: unknown' 'version'
    Assert-Contains $run.Lines '- Failed run: generation CanopyDB run 1: exit code 1: Generación cancelada. No se ha modificado el conjunto existente..' 'note'
    Assert-Equal 4 @($Fake.Calls | Where-Object { $_ -like '*generator.exe' }).Count 'only generations ran'
    Assert-Equal 0 @($run.Rows | Where-Object { $_.phase -ne 'generation' }).Count 'no query rows'
}

Test-Case 'A run that ends with 0 without a measure fails with the first missing measure' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{
        Single = { param($c) if ($c.Pair -eq 'sstable' -and $c.Query -eq 'F3' -and $c.Run -eq 1) { @{ NoStats = $true } } }
        Batch  = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Run -eq 1) { @{ NoStatsFor = 'F7' } } elseif ($c.Pair -eq 'sstable' -and $c.Run -eq 2) { @{ NoFinal = $true } } }
    }
    Assert-Contains $run.Lines '- Failed run: single SSTables F3 run 1: missing query_s.' 'single'
    Assert-Contains $run.Lines '- Failed run: batch CanopyDB run 1: missing F7 query_s.' 'batch query'
    Assert-Contains $run.Lines '- Failed run: batch SSTables run 2: missing batch_total_s.' 'batch total'
    Assert-Contains $run.Lines '- **Result**: FAIL: 3 of 76 runs failed, 0 of 11 queries returned different results' 'result'
}

Test-Case 'KM6: a batch run 0 that fails at F7 leaves F7 to F11 INCOMPLETE' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Batch = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Run -eq 0) { @{ StopAt = 'F7' } } } }
    $checks = @($run.Lines | Where-Object { $_ -match '^\| F[0-9]+ \| `' } | ForEach-Object { ($_ -split ' \| ')[-1].TrimEnd(' ', '|') })
    Assert-Equal 'SAME|SAME|SAME|SAME|SAME|SAME|INCOMPLETE|INCOMPLETE|INCOMPLETE|INCOMPLETE|INCOMPLETE' ($checks -join '|') 'checks'
    Assert-Contains $run.Lines '- Failed run: batch CanopyDB run 0: exit code 1: No se pudo leer el conjunto de datos en x..' 'note'
    Assert-Contains $run.Lines "| F7 | ``--where `"Primary Type=THEFT`" --where District=008 --count`` | 30 | 30 | 30 | $None | INCOMPLETE |" 'missing count'
    Assert-Contains $run.Lines '- **Result**: FAIL: 1 of 76 runs failed, 0 of 11 queries returned different results' 'result'
}

Test-Case 'KM7 and records: inherited limits and different records are noted without changing the result' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{
        Generation = { param($c) if ($c.Pair -eq 'sstable') { @{ Peak = '600,0' } } else { @{ Valid = '8.656.903' } } }
        Single     = { param($c) if ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F1') { @{ ProcessSeconds = 1.5 } } elseif ($c.Pair -eq 'canopydb' -and $c.Query -eq 'F3') { @{ PeakBytes = [long]40MB } } }
    }
    Assert-Equal 0 $run.Code 'exit code'
    $notes = @($run.Lines | Select-Object -Skip ([array]::IndexOf($run.Lines, '## Notes') + 2) | Where-Object { $_ -ne '' })
    Assert-Equal (@(
            '- AC-M4: SSTables generation peak median 600.0 MiB exceeds 512 MiB.'
            '- AC-C7: CanopyDB F1 process time median 1.500 s exceeds 1 s.'
            '- AC-M8: CanopyDB peak ratio 4.00 exceeds 3 (F3 40.0 MiB, F4 10.0 MiB, batch 12.0 MiB).'
            '- Records differ: SSTables 8656904 valid and 0 rejected, CanopyDB 8656903 valid and 0 rejected.'
        ) -join '|') ($notes -join '|') 'notes in order'
}

Test-Case 'Header: uncommitted changes and no git' {
    $repo = New-FakeRepo
    $run = Invoke-FakeBench $repo @{ Git = [pscustomobject]@{ Commit = 'abc1234'; Dirty = $true } } -Repetitions 1 -SkipBuild
    Assert-Contains $run.Lines '- **Commit**: abc1234 with uncommitted changes' 'dirty'
    Assert-Contains $run.Lines '- **Build**: skipped (-SkipBuild)' 'skip build'
    Assert-Equal 0 @($Fake.Calls | Where-Object { $_ -eq 'build' }).Count 'no build'
    $run = Invoke-FakeBench $repo @{ Git = [pscustomobject]@{ Commit = $null; Dirty = $false } } -Repetitions 1
    Assert-Equal '2026-10-22-120001-unknown.md' ([System.IO.Path]::GetFileName($run.Report)) 'name'
    Assert-Contains $run.Lines '- **Commit**: unknown' 'commit'
}

Test-Case 'KM10: -Baseline compares every median, in report order, inside and outside the tolerance' {
    $repo = New-FakeRepo
    $first = Invoke-FakeBench $repo -Repetitions 1
    $crlf = Join-Path $repo.Root 'baseline-crlf.csv'
    [System.IO.File]::WriteAllText($crlf, $first.DataText.Replace("`n", "`r`n"))
    $second = Invoke-FakeBench $repo -Repetitions 1 -Baseline $crlf -Behavior @{ Clock = '2026-10-22T13:00:00'; Single = { param($c) if ($c.Pair -eq 'sstable' -and $c.Query -eq 'F1' -and $c.Run -eq 1) { @{ ProcessSeconds = 0.5 } } } }
    Assert-Equal 0 $second.Code 'exit code unchanged'
    Assert-Contains $second.Lines ('- **Baseline**: `{0}`' -f $crlf) 'header'
    $rows = @($second.Lines | Select-Object -Skip ([array]::IndexOf($second.Lines, '## Baseline') + 4) | Where-Object { $_.StartsWith('| ') })
    Assert-Equal (10 + 66 + 22 + 6) $rows.Count 'rows'
    Assert-Equal '| Generation | SSTables | Duration median | 62.100 s | 62.100 s | +0.0 % | inside tolerance |' $rows[0] 'first row'
    Assert-Equal '| Generation | SSTables | Size | 1234 bytes (0.0 MiB) | 1234 bytes (0.0 MiB) | +0.0 % | inside tolerance |' $rows[2] 'size row'
    Assert-Equal '| Generation | SSTables | Rejected | 0 | 0 | — | — |' $rows[4] 'zero reference'
    Assert-Contains $rows '| Single queries | F1 SSTables | Process time median | 0.100 s | 0.500 s | +400.0 % | outside tolerance |' 'outside'
    Assert-Equal '| Batch | F1 SSTables | SSTables query time median | 0.041 s | 0.041 s | +0.0 % | inside tolerance |' $rows[76] 'batch per query'
    Assert-Equal '| Batch | CanopyDB | Peak median | 12.0 MiB | 12.0 MiB | +0.0 % | inside tolerance |' $rows[-1] 'last row'
    Assert-Equal 1 @($rows | Where-Object { $_.EndsWith('| outside tolerance |') }).Count 'only one outside'
}

Test-Case 'Relative -Csv and -Baseline follow the PowerShell location, not the process folder' {
    $repo = New-FakeRepo
    $first = Invoke-FakeBench $repo -Repetitions 1
    $relativeBaseline = Join-Path 'docs\canopy-db\comparisons' ([System.IO.Path]::GetFileName($first.Data))
    $script:Fake.Behavior = @{}
    $savedLocation = Get-Location
    $savedFolder = [Environment]::CurrentDirectory
    try {
        Set-Location -LiteralPath $repo.Root
        [Environment]::CurrentDirectory = [System.IO.Path]::GetTempPath()
        $script:SeenCsv = $null
        $environment = New-FakeEnvironment @{ Clock = '2026-10-22T13:00:00' }
        $environment.SharedCase = { param($Path) $script:SeenCsv = $Path; 'JA100' }
        $code = Invoke-CanopyComparison -RepoRoot $repo.Root -WorkDir $repo.WorkDir -Csv 'small.csv' -Repetitions 1 -Baseline $relativeBaseline -Environment $environment 6>$null
    }
    finally {
        Set-Location -LiteralPath $savedLocation
        [Environment]::CurrentDirectory = $savedFolder
    }
    Assert-Equal 0 $code 'exit code'
    Assert-Equal $repo.Csv $script:SeenCsv 'the CSV reaches the probes resolved'
    $report = Join-Path $repo.Reports '2026-10-22-130001-abc1234.md'
    $lines = @([System.IO.File]::ReadAllText($report) -split "`n")
    Assert-Contains $lines ('- **Baseline**: `{0}`' -f $relativeBaseline) 'baseline as passed'
    Assert-True ($lines -ccontains '## Baseline') 'baseline section'
}

Test-Case 'KM5: tolerance of each kind of measure, one condition is enough' {
    Assert-True (Test-WithinTolerance 'process_s' 1.2 1.0) 'time 20 %'
    Assert-True (-not (Test-WithinTolerance 'process_s' 1.3 1.0)) 'time over 20 % and 0.050 s'
    Assert-True (Test-WithinTolerance 'query_s' 0.010 0.050) 'time 0.050 s floor'
    Assert-True (Test-WithinTolerance 'peak_mib' 11.5 10) 'memory 2 MiB floor'
    Assert-True (Test-WithinTolerance 'peak_mib' 108 100) 'memory 10 %'
    Assert-True (-not (Test-WithinTolerance 'peak_mib' 15 10)) 'memory over both'
    Assert-True (Test-WithinTolerance 'size_bytes' 101 100) 'size 1 %'
    Assert-True (-not (Test-WithinTolerance 'size_bytes' 102 100)) 'size over 1 %'
    Assert-True (Test-WithinTolerance 'results' 5 5) 'count equal'
    Assert-True (-not (Test-WithinTolerance 'records_valid' 6 5)) 'count different'
    Assert-Equal '0.042' (Format-MetricValue 'query_s' (Get-Median 'query_s' @(0.041, 0.042))) 'tie away from zero'
    Assert-Equal '2.000' (Format-MetricValue 'query_s' (Get-Median 'query_s' @(3, 1, 2))) 'odd'
    Assert-Equal '1.3' (Format-MetricValue 'peak_mib' (Get-Median 'peak_mib' @(1.25, 1.35))) 'memory median'
}

Test-Case 'The CanopyDB settings come from the sources of the measured commit' {
    Assert-Equal ('generator: page_cache_size=64 MiB, use_wal=false, write_txn_memory_limit=32 MiB, checkpoint_target_size=32 MiB, ' +
        'throttle_memory_limit=128 MiB, stall_memory_limit=128 MiB, wal_write_batch_tempfile_dir=temp folder; query app: page_cache_size=1 MiB, file_lock_timeout=0 s') (Format-CanopySettings $RealRepoRoot) 'settings'
}

Test-Case 'AC-KM3: each preliminary check shows its message, runs nothing and writes no report' {
    $cases = [ordered]@{
        '-Repetitions must be at least 1.'                           = @{ Repetitions = 0 }
        'Missing file: {csv}'                                        = @{ Csv = 'missing.csv' }
        'Baseline file not found: {baseline}'                        = @{ Baseline = 'missing-baseline.csv' }
        'Baseline file is not a comparison bench data file: {bad}'   = @{ Baseline = 'bad' }
        'The CSV has no shared case number, so F2 cannot be built.'  = @{ Behavior = @{ SharedCase = $null } }
        'Report already exists: {report}'                            = @{ Existing = $true }
        'Only 1,024.0 MiB free in {work}; about 14 GB are needed.'  = @{ Behavior = @{ FreeBytes = [long]1GB } }
        "cargo build failed:`nboom"                                  = @{ Behavior = @{ Build = [pscustomobject]@{ ExitCode = 101; Stderr = 'boom' } } }
        'Missing binary: {binary}'                                   = @{ MissingBinary = $true }
    }
    foreach ($expected in $cases.Keys) {
        $case = $cases[$expected]
        $repo = New-FakeRepo
        $csv = if ($case.ContainsKey('Csv')) { Join-Path $repo.Root $case.Csv } else { $repo.Csv }
        $baseline = ''
        if ($case.ContainsKey('Baseline')) {
            $baseline = Join-Path $repo.Root $case.Baseline
            if ($case.Baseline -eq 'bad') { [System.IO.File]::WriteAllText($baseline, "phase,pair`n") }
        }
        $report = Join-Path $repo.Reports '2026-10-22-120001-abc1234.md'
        if ($case.ContainsKey('Existing')) { New-Item -ItemType Directory -Force -Path $repo.Reports | Out-Null; [System.IO.File]::WriteAllText($report, '') }
        $binary = (Get-ComparisonExecutables $repo.Root)['canopydb query']
        if ($case.ContainsKey('MissingBinary')) { Remove-Item -LiteralPath $binary }
        $environment = New-FakeEnvironment $(if ($case.ContainsKey('Behavior')) { $case.Behavior } else { @{} })
        $repetitions = if ($case.ContainsKey('Repetitions')) { $case.Repetitions } else { 1 }
        $message = Get-Message { Invoke-CanopyComparison -RepoRoot $repo.Root -WorkDir $repo.WorkDir -Csv $csv -Repetitions $repetitions -Baseline $baseline -Environment $environment 6>$null }
        $want = $expected.Replace('{csv}', $csv).Replace('{baseline}', $baseline).Replace('{bad}', $baseline).Replace('{report}', $report).Replace('{work}', $repo.WorkDir).Replace('{binary}', $binary)
        Assert-Equal $want $message 'message'
        Assert-Equal 0 @($Fake.Calls | Where-Object { $_ -ne 'build' }).Count "$expected`: applications run"
        $written = @(Get-ChildItem -LiteralPath $repo.Reports -ErrorAction SilentlyContinue | Where-Object { $_.FullName -ne $report })
        Assert-Equal 0 $written.Count "$expected`: report files"
    }
    $repo = New-FakeRepo
    $environment = New-FakeEnvironment
    Assert-Equal '-Repetitions must be at least 1.' (Get-Message { Invoke-CanopyComparison -RepoRoot $repo.Root -WorkDir $repo.WorkDir -Csv 'missing.csv' -Repetitions 0 -Environment $environment }) 'two cases: the lower order'
    Assert-Equal 'Missing file: missing.csv' (Get-Message { Invoke-CanopyComparison -RepoRoot $repo.Root -WorkDir $repo.WorkDir -Csv 'missing.csv' -Repetitions 1 -Baseline 'missing-baseline.csv' -Environment $environment }) 'CSV before baseline'
}

Test-Case 'AC-KM3: a -Repetitions that is not a number stops the script before anything runs' {
    $repo = New-FakeRepo
    $script = Join-Path $PSScriptRoot '..\run-canopy-comparison.ps1'
    # The parameter error comes on stderr; it must not stop this script.
    $ErrorActionPreference = 'Continue'
    $output = & powershell -NoProfile -ExecutionPolicy Bypass -File $script -WorkDir $repo.WorkDir -Repetitions abc 2>&1 | Out-String
    Assert-Equal 1 $LASTEXITCODE 'exit code'
    Assert-True ($output -match 'Repetitions') "message names the parameter: $output"
    Assert-True (-not (Test-Path -LiteralPath $repo.WorkDir)) 'no work folder'
}

# --- End ---

foreach ($root in $TempRoots) { if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force } }
if ($Failures.Count -gt 0) {
    Write-Host ''
    foreach ($failure in $Failures) { Write-Host "FAILED: $failure" }
    exit 1
}
Write-Host 'All tests passed.'
exit 0
