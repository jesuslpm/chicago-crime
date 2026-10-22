<#
.SYNOPSIS
    The comparison bench of the CanopyDB module (PRD canopy-db-comparacion): it runs both pairs,
    measures them, checks that they return the same results and writes the report and its data table.

.DESCRIPTION
    Dot-sourced by run-canopy-comparison.ps1 and by its tests. Invoke-CanopyComparison does a whole
    run. Every contact with the machine (starting a program, building, free space, the clock, git, the
    machine description, the CSV's shared case number and fingerprint) goes through the environment it
    receives, a hashtable of script blocks, so that the tests can replace the applications with fakes;
    New-ComparisonEnvironment builds the real one. The report and the data table follow the PRD's
    "Salida" section literally.
#>

. (Join-Path $PSScriptRoot 'query-output.ps1')
. (Join-Path $PSScriptRoot 'reference-extract.ps1')
# Compiled once per session: after editing a .cs file, run the bench in a new session.
if (-not ('QueryAcceptance' -as [type])) { Add-Type -Path (Join-Path $PSScriptRoot 'QueryAcceptance.cs') }
if (-not ('CanopyQueryAcceptance' -as [type])) { Add-Type -Path (Join-Path $PSScriptRoot 'CanopyQueryAcceptance.cs') }

$ComparisonInvariant = [System.Globalization.CultureInfo]::InvariantCulture
$ComparisonUtf8 = New-Object System.Text.UTF8Encoding $false
$ComparisonDataHeader = 'phase,pair,query,run,exit_code,metric,value'
$ComparisonPairs = @('sstable', 'canopydb')
$ComparisonPairNames = @{ sstable = 'SSTables'; canopydb = 'CanopyDB' }
$ComparisonGenerationMetrics = @('duration_s', 'peak_mib', 'size_bytes', 'records_valid', 'records_rejected')
$ComparisonSingleMetrics = @('query_s', 'process_s', 'peak_mib', 'results')
$ComparisonBatchQueryMetrics = @('query_s', 'results')
$ComparisonBatchTotalMetrics = @('batch_total_s', 'process_s', 'peak_mib')
$ComparisonReportFolder = 'docs\canopy-db\comparisons'
# Free space a run needs: the kept dataset of each pair, the generation in progress, its sort files
# and the query outputs (measured: see the README).
$ComparisonRequiredGb = 14
$ComparisonQueryCount = 11
$ComparisonNone = [string][char]0x2014
# CanopyDB settings of the applications, read from the Rust sources of the measured commit. The
# compression of the trees is left out: the Options line of the header shows it.
$ComparisonSettingSources = [ordered]@{
    'generator' = 'crates\crime-canopy-generator\src\lib.rs'
    'query app' = 'crates\crime-canopy-query\src\open.rs'
}
$ComparisonIgnoredSettings = @('compress_overflow_values')

# --- Numbers ---

function ConvertTo-ComparisonDecimal([string]$Text) { [decimal]::Parse($Text, $ComparisonInvariant) }

function Get-MetricDecimals([string]$Metric) {
    if ($Metric.EndsWith('_s')) { 3 } elseif ($Metric -eq 'peak_mib') { 1 } else { 0 }
}

function Get-MetricKind([string]$Metric) {
    if ($Metric.EndsWith('_s')) { 'time' } elseif ($Metric -eq 'peak_mib') { 'memory' } elseif ($Metric -eq 'size_bytes') { 'size' } else { 'count' }
}

function Get-RoundedAway([decimal]$Value, [int]$Decimals) { [decimal]::Round($Value, $Decimals, [MidpointRounding]::AwayFromZero) }

# A value of a metric as the data table writes it; '' when there is none.
function Format-MetricValue([string]$Metric, $Value) {
    if ($null -eq $Value) { return '' }
    $decimals = Get-MetricDecimals $Metric
    (Get-RoundedAway ([decimal]$Value) $decimals).ToString('F' + $decimals, $ComparisonInvariant)
}

# A value of a metric as the report shows it.
function Format-ReportValue([string]$Metric, $Value) {
    if ($null -eq $Value) { return $ComparisonNone }
    $text = Format-MetricValue $Metric $Value
    switch (Get-MetricKind $Metric) {
        'time' { "$text s" }
        'memory' { "$text MiB" }
        'size' { '{0} bytes ({1} MiB)' -f $text, (Get-RoundedAway ([decimal]$Value / 1MB) 1).ToString('F1', $ComparisonInvariant) }
        default { $text }
    }
}

# The median of the values, rounded as the metric's values: the middle one, or the mean of the two
# middle ones, in decimal and with ties away from zero; $null without values.
function Get-Median([string]$Metric, [decimal[]]$Values) {
    if ($null -eq $Values -or $Values.Count -eq 0) { return $null }
    $sorted = @($Values | Sort-Object)
    $middle = [int][math]::Floor($sorted.Count / 2)
    $median = if ($sorted.Count % 2 -eq 1) { [decimal]$sorted[$middle] } else { ([decimal]$sorted[$middle - 1] + [decimal]$sorted[$middle]) / 2 }
    Get-RoundedAway $median (Get-MetricDecimals $Metric)
}

# KM5: a measure is within the tolerance of a reference when it meets at least one condition.
function Test-WithinTolerance([string]$Metric, [decimal]$Value, [decimal]$Reference) {
    $difference = [math]::Abs($Value - $Reference)
    $base = [math]::Abs($Reference)
    switch (Get-MetricKind $Metric) {
        'time' { return ($difference -le $base * 0.2d) -or ($difference -le 0.050d) }
        'memory' { return ($difference -le $base * 0.1d) -or ($difference -le 2d) }
        'size' { return $difference -le $base * 0.01d }
        default { return $difference -eq 0 }
    }
}

# --- Data table ---

function New-DataRow([string]$Phase, [string]$Pair, [string]$Query, [int]$Run, [int]$ExitCode, [string]$Metric, $Value) {
    [pscustomobject]@{ phase = $Phase; pair = $Pair; query = $Query; run = $Run; exit_code = $ExitCode; metric = $Metric; value = (Format-MetricValue $Metric $Value) }
}

# The rows of one run, in the order of the PRD's metric lists.
function Get-RunRows($Run) {
    $rows = New-Object System.Collections.Generic.List[object]
    switch ($Run.Phase) {
        'generation' { foreach ($metric in $ComparisonGenerationMetrics) { $rows.Add((New-DataRow 'generation' $Run.Pair '' $Run.Run $Run.ExitCode $metric $Run.Values[$metric])) } }
        'single' { foreach ($metric in $ComparisonSingleMetrics) { $rows.Add((New-DataRow 'single' $Run.Pair $Run.Query $Run.Run $Run.ExitCode $metric $Run.Values[$metric])) } }
        'batch' {
            for ($i = 1; $i -le $ComparisonQueryCount; $i++) {
                foreach ($metric in $ComparisonBatchQueryMetrics) { $rows.Add((New-DataRow 'batch' $Run.Pair "F$i" $Run.Run $Run.ExitCode $metric $Run.Values["F$i $metric"])) }
            }
            foreach ($metric in $ComparisonBatchTotalMetrics) { $rows.Add((New-DataRow 'batch' $Run.Pair '' $Run.Run $Run.ExitCode $metric $Run.Values[$metric])) }
        }
    }
    $rows.ToArray()
}

function Write-ComparisonData([string]$Path, $Rows) {
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add($ComparisonDataHeader)
    foreach ($row in $Rows) { $lines.Add(('{0},{1},{2},{3},{4},{5},{6}' -f $row.phase, $row.pair, $row.query, $row.run, $row.exit_code, $row.metric, $row.value)) }
    [System.IO.File]::WriteAllText($Path, (($lines -join "`n") + "`n"), $ComparisonUtf8)
}

# The rows of a data table, or $null when its first line is not the header. A carriage return at the
# end of a line is ignored.
function Read-ComparisonData([string]$Path) {
    $lines = @([System.IO.File]::ReadAllText($Path, $ComparisonUtf8) -split "`n" | ForEach-Object { $_.TrimEnd("`r") })
    if ($lines.Count -eq 0 -or $lines[0].TrimStart([char]0xFEFF) -cne $ComparisonDataHeader) { return $null }
    $rows = New-Object System.Collections.Generic.List[object]
    foreach ($line in ($lines | Select-Object -Skip 1)) {
        if ($line -eq '') { continue }
        $fields = $line -split ',', 7
        if ($fields.Count -ne 7) { continue }
        $rows.Add([pscustomobject]@{ phase = $fields[0]; pair = $fields[1]; query = $fields[2]; run = [int]$fields[3]; exit_code = [int]$fields[4]; metric = $fields[5]; value = $fields[6] })
    }
    , $rows.ToArray()
}

# A run is identified by its phase, pair and run number, and in the single queries by its query too.
function Get-RunKey($Row) {
    if ($Row.phase -eq 'single') { '{0}|{1}|{2}|{3}' -f $Row.phase, $Row.pair, $Row.query, $Row.run }
    else { '{0}|{1}|{2}' -f $Row.phase, $Row.pair, $Row.run }
}

# The rows indexed for the report: which runs ended well (exit 0 and every measure), and the values of
# each series (phase, pair, query and metric) by run.
function New-RowIndex($Rows) {
    $ok = @{}
    $series = @{}
    foreach ($row in $Rows) {
        $key = Get-RunKey $row
        if (-not $ok.ContainsKey($key)) { $ok[$key] = $true }
        if ($row.exit_code -ne 0 -or $row.value -eq '') { $ok[$key] = $false }
        $seriesKey = '{0}|{1}|{2}|{3}' -f $row.phase, $row.pair, $row.query, $row.metric
        if (-not $series.ContainsKey($seriesKey)) { $series[$seriesKey] = New-Object System.Collections.Generic.List[object] }
        $series[$seriesKey].Add($row)
    }
    @{ Ok = $ok; Series = $series }
}

function Test-RunOk($Index, $Row) { [bool]$Index.Ok[(Get-RunKey $Row)] }

# The values of a series from the runs that ended well: runs 1 to R, or with -First only run 0.
function Get-SeriesValues($Index, [string]$Phase, [string]$Pair, [string]$Query, [string]$Metric, [switch]$First) {
    $values = New-Object System.Collections.Generic.List[decimal]
    $rows = $Index.Series[('{0}|{1}|{2}|{3}' -f $Phase, $Pair, $Query, $Metric)]
    if ($null -eq $rows) { return , $values.ToArray() }
    foreach ($row in $rows) {
        if ([bool]$First -ne ($row.run -eq 0)) { continue }
        if (Test-RunOk $Index $row) { $values.Add((ConvertTo-ComparisonDecimal $row.value)) }
    }
    , $values.ToArray()
}

function Get-SeriesMedian($Index, [string]$Phase, [string]$Pair, [string]$Query, [string]$Metric) {
    Get-Median $Metric (Get-SeriesValues $Index $Phase $Pair $Query $Metric)
}

function Get-FirstValue($Index, [string]$Phase, [string]$Pair, [string]$Query, [string]$Metric) {
    $values = [decimal[]](Get-SeriesValues $Index $Phase $Pair $Query $Metric -First)
    if ($values.Length -gt 0) { $values[0] } else { $null }
}

# The number of runs 1 to R that ended well in a series' run group (the runs of its first metric).
function Get-OkRunCount($Index, [string]$Phase, [string]$Pair, [string]$Query, [string]$Metric) {
    ([decimal[]](Get-SeriesValues $Index $Phase $Pair $Query $Metric)).Length
}

# A value of the kept generation of a pair (its last generation that ended well), or $null.
function Get-KeptValue($Index, [string]$Pair, [string]$Metric) {
    $rows = $Index.Series[('generation|{0}||{1}' -f $Pair, $Metric)]
    if ($null -eq $rows) { return $null }
    $kept = @($rows | Where-Object { Test-RunOk $Index $_ } | Sort-Object run)
    if ($kept.Count -eq 0) { return $null }
    ConvertTo-ComparisonDecimal $kept[-1].value
}

# How many runs 1 to R that ended well have one of the metrics outside the tolerance of its median.
function Get-OutOfTolerance($Index, [string]$Phase, [string]$Pair, [string]$Query, [string[]]$Metrics) {
    $outside = @{}
    foreach ($metric in $Metrics) {
        $median = Get-SeriesMedian $Index $Phase $Pair $Query $metric
        if ($null -eq $median) { continue }
        $rows = $Index.Series[('{0}|{1}|{2}|{3}' -f $Phase, $Pair, $Query, $metric)]
        if ($null -eq $rows) { continue }
        foreach ($row in $rows) {
            if ($row.run -eq 0 -or -not (Test-RunOk $Index $row)) { continue }
            if (-not (Test-WithinTolerance $metric (ConvertTo-ComparisonDecimal $row.value) $median)) { $outside[(Get-RunKey $row)] = $true }
        }
    }
    $outside.Count
}

# Every median the Baseline section compares, in the order of the report: section, row, measure,
# metric and value ($null without one).
function Get-MedianEntries($Index) {
    $entries = New-Object System.Collections.Generic.List[object]
    $add = {
        param($Section, $Row, $Measure, $Metric, $Value)
        $entries.Add([pscustomobject]@{ Key = "$Section|$Row|$Measure"; Section = $Section; Row = $Row; Measure = $Measure; Metric = $Metric; Value = $Value })
    }
    foreach ($pair in $ComparisonPairs) {
        $name = $ComparisonPairNames[$pair]
        & $add 'Generation' $name 'Duration median' 'duration_s' (Get-SeriesMedian $Index 'generation' $pair '' 'duration_s')
        & $add 'Generation' $name 'Peak median' 'peak_mib' (Get-SeriesMedian $Index 'generation' $pair '' 'peak_mib')
        & $add 'Generation' $name 'Size' 'size_bytes' (Get-KeptValue $Index $pair 'size_bytes')
        & $add 'Generation' $name 'Valid' 'records_valid' (Get-KeptValue $Index $pair 'records_valid')
        & $add 'Generation' $name 'Rejected' 'records_rejected' (Get-KeptValue $Index $pair 'records_rejected')
    }
    for ($i = 1; $i -le $ComparisonQueryCount; $i++) {
        foreach ($pair in $ComparisonPairs) {
            $row = "F$i $($ComparisonPairNames[$pair])"
            & $add 'Single queries' $row 'Query time median' 'query_s' (Get-SeriesMedian $Index 'single' $pair "F$i" 'query_s')
            & $add 'Single queries' $row 'Process time median' 'process_s' (Get-SeriesMedian $Index 'single' $pair "F$i" 'process_s')
            & $add 'Single queries' $row 'Peak median' 'peak_mib' (Get-SeriesMedian $Index 'single' $pair "F$i" 'peak_mib')
        }
    }
    for ($i = 1; $i -le $ComparisonQueryCount; $i++) {
        foreach ($pair in $ComparisonPairs) {
            $name = $ComparisonPairNames[$pair]
            & $add 'Batch' "F$i $name" "$name query time median" 'query_s' (Get-SeriesMedian $Index 'batch' $pair "F$i" 'query_s')
        }
    }
    foreach ($measure in @(@('Batch total median', 'batch_total_s'), @('Process time median', 'process_s'), @('Peak median', 'peak_mib'))) {
        foreach ($pair in $ComparisonPairs) {
            & $add 'Batch' $ComparisonPairNames[$pair] $measure[0] $measure[1] (Get-SeriesMedian $Index 'batch' $pair '' $measure[1])
        }
    }
    $entries.ToArray()
}

# --- Runs ---

# The reason a run failed, or $null: its exit code, or the first measure it did not give.
function Get-FailureReason([int]$ExitCode, [string]$Stderr, [string[]]$Order, $Values) {
    if ($ExitCode -ne 0) {
        $last = @(Get-Lines $Stderr | Where-Object { $_.Trim() -ne '' }) | Select-Object -Last 1
        if (-not $last) { $last = 'no error output' }
        return "exit code ${ExitCode}: $last"
    }
    foreach ($name in $Order) { if ($null -eq $Values[$name]) { return "missing $name" } }
    $null
}

function ConvertTo-Mib([long]$Bytes) { if ($Bytes -lt 0) { $null } else { [decimal]$Bytes / 1MB } }

# A duration as the generator's summary writes it ("1 h 2 min 3,4 s") in seconds, or $null.
function ConvertFrom-SummaryDuration([string]$Text) {
    $match = [regex]::Match($Text, '^(?:(?<h>[0-9]+) h )?(?:(?<m>[0-9]+) min )?(?<s>[0-9]+,[0-9]) s$')
    if (-not $match.Success) { return $null }
    $seconds = ConvertFrom-SpanishSeconds $match.Groups['s'].Value
    if ($match.Groups['m'].Success) { $seconds += 60 * [decimal]$match.Groups['m'].Value }
    if ($match.Groups['h'].Success) { $seconds += 3600 * [decimal]$match.Groups['h'].Value }
    $seconds
}

function ConvertFrom-SpanishCount([string]$Text) { [decimal]$Text.Replace('.', '') }

# The measures of a generation from the final summary in its log.
function Read-GenerationSummary([string]$LogPath) {
    $values = @{}
    if (-not (Test-Path -LiteralPath $LogPath -PathType Leaf)) { return $values }
    $text = [System.IO.File]::ReadAllText($LogPath, $ComparisonUtf8)
    $completed = [regex]::Match($text, '(?m)^Generación completada: (?<valid>[0-9.]+) registros válidos, (?<rejected>[0-9.]+) rechazados')
    if ($completed.Success) {
        $values['records_valid'] = ConvertFrom-SpanishCount $completed.Groups['valid'].Value
        $values['records_rejected'] = ConvertFrom-SpanishCount $completed.Groups['rejected'].Value
    }
    $duration = [regex]::Match($text, '(?m)^Duración: (?<d>.+)\.\r?$')
    if ($duration.Success) { $values['duration_s'] = ConvertFrom-SummaryDuration $duration.Groups['d'].Value }
    $peak = [regex]::Match($text, '(?m)^Pico de memoria privada comprometida: (?<mib>[0-9.]+,[0-9]) MiB')
    if ($peak.Success) { $values['peak_mib'] = ConvertFrom-SpanishSeconds $peak.Groups['mib'].Value }
    $files = [regex]::Matches($text, '(?m)^Archivo .+: (?<bytes>[0-9.]+) bytes \(')
    if ($files.Count -gt 0) {
        $size = [decimal]0
        foreach ($file in $files) { $size += ConvertFrom-SpanishCount $file.Groups['bytes'].Value }
        $values['size_bytes'] = $size
    }
    $values
}

function New-GenerationRun([string]$Pair, [int]$Run, $Result, [string]$LogPath, [string]$Output) {
    $values = if ($Result.ExitCode -eq 0) { Read-GenerationSummary $LogPath } else { @{} }
    [pscustomobject]@{
        Phase = 'generation'; Pair = $Pair; Query = ''; Run = $Run; ExitCode = $Result.ExitCode; Values = $values; Output = $Output
        Reason = Get-FailureReason $Result.ExitCode $Result.Stderr $ComparisonGenerationMetrics $values
    }
}

function New-SingleRun([string]$Pair, $Query, [int]$Run, $Result, [string]$StdoutPath) {
    $values = @{ process_s = [decimal]$Result.ProcessSeconds; peak_mib = ConvertTo-Mib $Result.PeakBytes }
    $stats = Get-Stats $Result.Stderr
    if ($stats) { $values['query_s'] = $stats.Seconds; $values['results'] = [decimal]$stats.Count }
    [pscustomobject]@{
        Phase = 'single'; Pair = $Pair; Query = $Query.Id; Run = $Run; ExitCode = $Result.ExitCode; Values = $values; Stdout = $StdoutPath
        Reason = Get-FailureReason $Result.ExitCode $Result.Stderr $ComparisonSingleMetrics $values
    }
}

function New-BatchRun([string]$Pair, [int]$Run, $Result, [string]$StdoutPath) {
    $values = @{ process_s = [decimal]$Result.ProcessSeconds; peak_mib = ConvertTo-Mib $Result.PeakBytes }
    $parsed = Read-BatchStderr $Result.Stderr
    $order = New-Object System.Collections.Generic.List[string]
    for ($i = 1; $i -le $ComparisonQueryCount; $i++) {
        $order.Add("F$i query_s"); $order.Add("F$i results")
        if ($i -le $parsed.Sections.Count) {
            $stats = Get-Stats ($parsed.Sections[$i - 1].Lines -join "`n")
            if ($stats) { $values["F$i query_s"] = $stats.Seconds; $values["F$i results"] = [decimal]$stats.Count }
        }
    }
    if ($null -ne $parsed.Final) { $values['batch_total_s'] = ConvertFrom-SpanishSeconds $parsed.Final.Groups['seconds'].Value }
    foreach ($metric in $ComparisonBatchTotalMetrics) { $order.Add($metric) }
    [pscustomobject]@{
        Phase = 'batch'; Pair = $Pair; Query = ''; Run = $Run; ExitCode = $Result.ExitCode; Values = $values; Stdout = $StdoutPath
        Reason = Get-FailureReason $Result.ExitCode $Result.Stderr $order.ToArray() $values
    }
}

# --- Same results (KM6) ---

# The query that lists without limit the same conditions as a limited one (F11 for F10), or $null.
function Get-UnlimitedQuery($Queries, $Query) {
    $words = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $Query.Arguments.Count; $i++) {
        if ($Query.Arguments[$i] -ceq '--limit') { $i++; continue }
        $words.Add($Query.Arguments[$i])
    }
    $text = $words -join [char]0x1F
    @($Queries | Where-Object { $_.Limit -lt 0 -and (($_.Arguments -join [char]0x1F) -ceq $text) }) | Select-Object -First 1
}

# The record problems of one query between the two pairs' outputs, or $null (AC-M2 of the CanopyDB
# query app PRD, with F10 holding as many records as the smaller of its limit and F11's results).
# $Outputs maps pair -> query id -> output file; $Counts maps pair -> query id -> results.
function Get-RecordProblem($Queries, $Query, $Outputs, $Counts) {
    $left = $Outputs['sstable'][$Query.Id]
    $right = $Outputs['canopydb'][$Query.Id]
    if ($Query.Kind -eq 'json') { return Compare-JsonDocuments (Read-JsonFile $left) (Read-JsonFile $right) }
    if ($Query.Kind -ne 'csv') { return $null }
    if ($Query.Limit -lt 0) {
        if (-not $left -or -not $right) { return 'a listing is missing' }
        return [CanopyQueryAcceptance]::CompareRowSetsHashed($left, $right)
    }
    $unlimited = Get-UnlimitedQuery $Queries $Query
    if (-not $unlimited) { return 'no unlimited query with the same conditions' }
    foreach ($pair in $ComparisonPairs) {
        $full = $Outputs[$pair][$unlimited.Id]
        $count = $Counts[$pair][$unlimited.Id]
        if (-not $Outputs[$pair][$Query.Id] -or -not $full -or $null -eq $count) { return 'a listing is missing' }
        $problem = [CanopyQueryAcceptance]::CheckLimited($Outputs[$pair][$Query.Id], $full, [int][math]::Min([decimal]$Query.Limit, [decimal]$count))
        if ($problem) { return "$($ComparisonPairNames[$pair]): $problem" }
    }
    $null
}

# Splits the standard output of run 0 of a pair's batch into one file per query that gave results.
# Returns query id -> file.
function Split-FirstBatch($Run, $Queries, [string]$CsvHeader, [string]$OutDir) {
    $files = @{}
    $answered = @($Queries | Where-Object { $null -ne $Run.Values["$($_.Id) results"] })
    if ($answered.Count -eq 0 -or -not (Test-Path -LiteralPath $Run.Stdout -PathType Leaf)) { return $files }
    $targets = [string[]]@($answered | ForEach-Object { Join-Path $OutDir ('{0}-batch-first-{1}.{2}' -f $Run.Pair, $_.Id, $_.Extension) })
    $problem = [CanopyQueryAcceptance]::SplitBatchOutput($Run.Stdout, [string[]]@($answered | ForEach-Object { $_.Kind }), $targets, $CsvHeader)
    # A batch that failed may leave output of the query it failed on.
    if ($problem -and -not ($Run.Reason -and $problem.StartsWith('output after the last query'))) { return $files }
    for ($i = 0; $i -lt $answered.Count; $i++) { $files[$answered[$i].Id] = $targets[$i] }
    $files
}

# The Check of each query: NOT RUN, INCOMPLETE, DIFFERENT or SAME (KM6).
function Get-QueryChecks($Runs, $Queries, [bool]$QueriesRun, [string]$OutDir) {
    $checks = [ordered]@{}
    if (-not $QueriesRun) {
        foreach ($query in $Queries) { $checks[$query.Id] = 'NOT RUN' }
        return $checks
    }
    $singleFirst = @{}
    $batchFirst = @{}
    foreach ($pair in $ComparisonPairs) {
        $singleFirst[$pair] = @{}
        foreach ($run in @($Runs | Where-Object { $_.Phase -eq 'single' -and $_.Pair -eq $pair -and $_.Run -eq 0 })) { $singleFirst[$pair][$run.Query] = $run }
        $batchFirst[$pair] = @($Runs | Where-Object { $_.Phase -eq 'batch' -and $_.Pair -eq $pair -and $_.Run -eq 0 }) | Select-Object -First 1
    }
    $singleOutputs = @{}; $singleCounts = @{}; $batchOutputs = @{}; $batchCounts = @{}
    foreach ($pair in $ComparisonPairs) {
        $singleOutputs[$pair] = @{}; $singleCounts[$pair] = @{}; $batchCounts[$pair] = @{}
        foreach ($query in $Queries) {
            $run = $singleFirst[$pair][$query.Id]
            $singleCounts[$pair][$query.Id] = if ($run) { $run.Values['results'] } else { $null }
            if ($run -and $null -ne $run.Values['results']) { $singleOutputs[$pair][$query.Id] = $run.Stdout }
            $batchCounts[$pair][$query.Id] = if ($batchFirst[$pair]) { $batchFirst[$pair].Values["$($query.Id) results"] } else { $null }
        }
        $header = ''
        $listing = @($Queries | Where-Object { $_.Kind -eq 'csv' -and $singleOutputs[$pair][$_.Id] }) | Select-Object -First 1
        if ($listing) { $header = @(Get-Content -LiteralPath $singleOutputs[$pair][$listing.Id] -TotalCount 1 -Encoding UTF8) -join '' }
        $batchOutputs[$pair] = if ($batchFirst[$pair]) { Split-FirstBatch $batchFirst[$pair] $Queries $header $OutDir } else { @{} }
    }
    foreach ($query in $Queries) {
        $firsts = @($singleCounts['sstable'][$query.Id], $singleCounts['canopydb'][$query.Id], $batchCounts['sstable'][$query.Id], $batchCounts['canopydb'][$query.Id])
        if (@($firsts | Where-Object { $null -eq $_ }).Count -gt 0) { $checks[$query.Id] = 'INCOMPLETE'; continue }
        $counts = New-Object System.Collections.Generic.List[decimal]
        foreach ($run in $Runs) {
            if ($run.Reason) { continue }
            if ($run.Phase -eq 'single' -and $run.Query -eq $query.Id) { $counts.Add($run.Values['results']) }
            if ($run.Phase -eq 'batch') { $counts.Add($run.Values["$($query.Id) results"]) }
        }
        $counts.AddRange([decimal[]]$firsts)
        if (@($counts | Sort-Object -Unique).Count -ne 1) { $checks[$query.Id] = 'DIFFERENT'; continue }
        $problem = Get-RecordProblem $Queries $query $singleOutputs $singleCounts
        if (-not $problem) { $problem = Get-RecordProblem $Queries $query $batchOutputs $batchCounts }
        $checks[$query.Id] = if ($problem) { 'DIFFERENT' } else { 'SAME' }
    }
    $checks
}

# --- Header ---

# The value of a Rust expression of a setting, as the header shows it, or 'unknown'.
function Resolve-RustSetting([string]$Expression, [hashtable]$Constants) {
    $expression = $Expression.Trim()
    if ($Constants.ContainsKey($expression)) { $expression = $Constants[$expression].Trim() }
    $expression = $expression.Replace('_', '')
    $shift = [regex]::Match($expression, '^([0-9]+)\s*<<\s*([0-9]+)$')
    $product = [regex]::Match($expression, '^[0-9]+(\s*\*\s*[0-9]+)*$')
    $bytes = $null
    if ($shift.Success) { $bytes = [decimal]$shift.Groups[1].Value * [decimal][math]::Pow(2, [int]$shift.Groups[2].Value) }
    elseif ($product.Success) { $bytes = [decimal]1; foreach ($factor in $expression -split '\*') { $bytes *= [decimal]$factor.Trim() } }
    if ($null -ne $bytes) { return '{0} MiB' -f ($bytes / 1MB).ToString('0.#', $ComparisonInvariant) }
    if ($expression -ceq 'true' -or $expression -ceq 'false') { return $expression }
    if ($expression -ceq 'Duration::ZERO') { return '0 s' }
    $seconds = [regex]::Match($expression, '^Duration::from_secs\(([0-9]+)\)$')
    if ($seconds.Success) { return "$($seconds.Groups[1].Value) s" }
    if ($expression -match '^scratch\.') { return 'temp folder' }
    'unknown'
}

# The CanopyDB settings one application changes, from its Rust source: the page cache first, then the
# others in source order, as name=value.
function Get-RustSettings([string]$Path) {
    $settings = [ordered]@{ page_cache_size = 'unknown' }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $settings }
    $text = [System.IO.File]::ReadAllText($Path, $ComparisonUtf8)
    $constants = @{}
    foreach ($match in [regex]::Matches($text, '(?m)^\s*(?:pub\s+)?const\s+(?<name>[A-Z0-9_]+)\s*:\s*[A-Za-z0-9_]+\s*=\s*(?<expr>[^;]+);')) {
        $constants[$match.Groups['name'].Value] = $match.Groups['expr'].Value
    }
    foreach ($match in [regex]::Matches($text, '(?m)^\s*options\.(?<name>[a-z_]+)\s*=\s*(?<expr>[^;]+);')) {
        $name = $match.Groups['name'].Value
        if ($ComparisonIgnoredSettings -contains $name) { continue }
        $settings[$name] = Resolve-RustSetting $match.Groups['expr'].Value $constants
    }
    $settings
}

function Format-CanopySettings([string]$RepoRoot) {
    $parts = foreach ($app in $ComparisonSettingSources.Keys) {
        $settings = Get-RustSettings (Join-Path $RepoRoot $ComparisonSettingSources[$app])
        '{0}: {1}' -f $app, ((@($settings.Keys) | ForEach-Object { "$_=$($settings[$_])" }) -join ', ')
    }
    $parts -join '; '
}

# The manifest of the active version of a dataset folder, or $null.
function Read-ActiveManifest([string]$Output) {
    $current = Join-Path $Output 'CURRENT'
    if (-not $Output -or -not (Test-Path -LiteralPath $current -PathType Leaf)) { return $null }
    $name = ([System.IO.File]::ReadAllText($current, $ComparisonUtf8)).Trim()
    $manifest = Join-Path (Join-Path $Output $name) 'manifest.json'
    if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) { return $null }
    Read-JsonFile $manifest
}

function Format-CanopyVersion($Manifest) {
    if ($null -eq $Manifest -or $null -eq $Manifest.PSObject.Properties['engine']) { return 'unknown' }
    $engine = $Manifest.engine
    $fork = if ($engine.PSObject.Properties['fork_commit'] -and $engine.fork_commit) { $engine.fork_commit } else { 'none' }
    if (-not $engine.PSObject.Properties['version']) { return 'unknown' }
    '{0} (fork {1})' -f $engine.version, $fork
}

function Format-Invariant([string]$Format, [object[]]$Values) { [string]::Format($ComparisonInvariant, $Format, $Values) }

function Write-ComparisonStep([string]$Text) { Write-Host ('[{0:HH:mm:ss}] {1}' -f (Get-Date), $Text) }

# --- Report ---

function Format-Cell([string]$Metric, $Value) { Format-ReportValue $Metric $Value }

function Format-TableRow([string[]]$Cells) { '| ' + ($Cells -join ' | ') + ' |' }

function Get-ReportSections($Index, $Queries, $Checks, $Runs, [int]$Repetitions, $BaselineIndex, [string[]]$Notes) {
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('## Generation'); $lines.Add('')
    $lines.Add('| Pair | Runs | Duration median | Duration min | Duration max | Peak median | Size | Valid | Rejected | Out of tolerance |')
    $lines.Add('|---|---|---|---|---|---|---|---|---|---|')
    foreach ($pair in $ComparisonPairs) {
        $durations = [decimal[]](Get-SeriesValues $Index 'generation' $pair '' 'duration_s')
        [array]::Sort($durations)
        $lines.Add((Format-TableRow @(
                    $ComparisonPairNames[$pair]
                    ('{0} of {1}' -f (Get-OkRunCount $Index 'generation' $pair '' 'duration_s'), $Repetitions)
                    (Format-Cell 'duration_s' (Get-SeriesMedian $Index 'generation' $pair '' 'duration_s'))
                    (Format-Cell 'duration_s' $(if ($durations.Count) { $durations[0] } else { $null }))
                    (Format-Cell 'duration_s' $(if ($durations.Count) { $durations[-1] } else { $null }))
                    (Format-Cell 'peak_mib' (Get-SeriesMedian $Index 'generation' $pair '' 'peak_mib'))
                    (Format-Cell 'size_bytes' (Get-KeptValue $Index $pair 'size_bytes'))
                    (Format-Cell 'records_valid' (Get-KeptValue $Index $pair 'records_valid'))
                    (Format-Cell 'records_rejected' (Get-KeptValue $Index $pair 'records_rejected'))
                    [string](Get-OutOfTolerance $Index 'generation' $pair '' @('duration_s', 'peak_mib'))
                )))
    }
    $lines.Add('')
    $lines.Add('## Results'); $lines.Add('')
    $lines.Add('| Query | Line | SSTables single | CanopyDB single | SSTables batch | CanopyDB batch | Check |')
    $lines.Add('|---|---|---|---|---|---|---|')
    foreach ($query in $Queries) {
        $lines.Add((Format-TableRow @(
                    $query.Id
                    ('`{0}`' -f $query.Line.Replace('|', '\|'))
                    (Format-Cell 'results' (Get-FirstValue $Index 'single' 'sstable' $query.Id 'results'))
                    (Format-Cell 'results' (Get-FirstValue $Index 'single' 'canopydb' $query.Id 'results'))
                    (Format-Cell 'results' (Get-FirstBatchCount $Runs 'sstable' $query.Id))
                    (Format-Cell 'results' (Get-FirstBatchCount $Runs 'canopydb' $query.Id))
                    $Checks[$query.Id]
                )))
    }
    $lines.Add('')
    $lines.Add('## Single queries'); $lines.Add('')
    $lines.Add('| Query | Pair | Runs | Query time median | Process time median | Process time first run | Peak median | Out of tolerance |')
    $lines.Add('|---|---|---|---|---|---|---|---|')
    foreach ($query in $Queries) {
        foreach ($pair in $ComparisonPairs) {
            $lines.Add((Format-TableRow @(
                        $query.Id
                        $ComparisonPairNames[$pair]
                        ('{0} of {1}' -f (Get-OkRunCount $Index 'single' $pair $query.Id 'process_s'), $Repetitions)
                        (Format-Cell 'query_s' (Get-SeriesMedian $Index 'single' $pair $query.Id 'query_s'))
                        (Format-Cell 'process_s' (Get-SeriesMedian $Index 'single' $pair $query.Id 'process_s'))
                        (Format-Cell 'process_s' (Get-FirstValue $Index 'single' $pair $query.Id 'process_s'))
                        (Format-Cell 'peak_mib' (Get-SeriesMedian $Index 'single' $pair $query.Id 'peak_mib'))
                        [string](Get-OutOfTolerance $Index 'single' $pair $query.Id @('query_s', 'process_s', 'peak_mib'))
                    )))
        }
    }
    $lines.Add('')
    $lines.Add('## Batch'); $lines.Add('')
    $lines.Add('| Query | SSTables query time median | CanopyDB query time median |')
    $lines.Add('|---|---|---|')
    foreach ($query in $Queries) {
        $lines.Add((Format-TableRow @(
                    $query.Id
                    (Format-Cell 'query_s' (Get-SeriesMedian $Index 'batch' 'sstable' $query.Id 'query_s'))
                    (Format-Cell 'query_s' (Get-SeriesMedian $Index 'batch' 'canopydb' $query.Id 'query_s'))
                )))
    }
    $lines.Add('')
    $lines.Add('| Measure | SSTables | CanopyDB |')
    $lines.Add('|---|---|---|')
    $measures = [ordered]@{
        'Runs'                   = { param($pair) '{0} of {1}' -f (Get-OkRunCount $Index 'batch' $pair '' 'process_s'), $Repetitions }
        'Batch total median'     = { param($pair) Format-Cell 'batch_total_s' (Get-SeriesMedian $Index 'batch' $pair '' 'batch_total_s') }
        'Process time median'    = { param($pair) Format-Cell 'process_s' (Get-SeriesMedian $Index 'batch' $pair '' 'process_s') }
        'Process time first run' = { param($pair) Format-Cell 'process_s' (Get-FirstValue $Index 'batch' $pair '' 'process_s') }
        'Peak median'            = { param($pair) Format-Cell 'peak_mib' (Get-SeriesMedian $Index 'batch' $pair '' 'peak_mib') }
        'Peak first run'         = { param($pair) Format-Cell 'peak_mib' (Get-FirstValue $Index 'batch' $pair '' 'peak_mib') }
        'Out of tolerance'       = { param($pair) [string](Get-OutOfTolerance $Index 'batch' $pair '' $ComparisonBatchTotalMetrics) }
    }
    foreach ($measure in $measures.Keys) {
        $lines.Add((Format-TableRow @($measure, (& $measures[$measure] 'sstable'), (& $measures[$measure] 'canopydb'))))
    }
    $lines.Add('')
    if ($null -ne $BaselineIndex) {
        $lines.Add('## Baseline'); $lines.Add('')
        $lines.Add('| Section | Row | Measure | Baseline | This run | Change | Status |')
        $lines.Add('|---|---|---|---|---|---|---|')
        $reference = @{}
        foreach ($entry in (Get-MedianEntries $BaselineIndex)) { $reference[$entry.Key] = $entry.Value }
        foreach ($entry in (Get-MedianEntries $Index)) {
            $base = $reference[$entry.Key]
            $change = $ComparisonNone
            $status = $ComparisonNone
            if ($null -ne $base -and $null -ne $entry.Value -and $base -ne 0) {
                $percent = Get-RoundedAway (($entry.Value - $base) / $base * 100) 1
                $change = '{0}{1} %' -f $(if ($percent -ge 0) { '+' } else { '' }), $percent.ToString('F1', $ComparisonInvariant)
                $status = if (Test-WithinTolerance $entry.Metric $entry.Value $base) { 'inside tolerance' } else { 'outside tolerance' }
            }
            $lines.Add((Format-TableRow @($entry.Section, $entry.Row, $entry.Measure, (Format-Cell $entry.Metric $base), (Format-Cell $entry.Metric $entry.Value), $change, $status)))
        }
        $lines.Add('')
    }
    $lines.Add('## Notes'); $lines.Add('')
    if ($Notes.Count -eq 0) { $lines.Add('none') } else { foreach ($note in $Notes) { $lines.Add($note) } }
    $lines.ToArray()
}

function Get-FirstBatchCount($Runs, [string]$Pair, [string]$QueryId) {
    $run = @($Runs | Where-Object { $_.Phase -eq 'batch' -and $_.Pair -eq $Pair -and $_.Run -eq 0 }) | Select-Object -First 1
    if ($run) { $run.Values["$QueryId results"] } else { $null }
}

# The Notes lines (KM7, records that differ, failed runs), in the PRD's order.
function Get-ReportNotes($Index, $Runs) {
    $notes = New-Object System.Collections.Generic.List[string]
    foreach ($pair in $ComparisonPairs) {
        $peak = Get-SeriesMedian $Index 'generation' $pair '' 'peak_mib'
        if ($null -ne $peak -and $peak -gt 512) { $notes.Add(('- AC-M4: {0} generation peak median {1} MiB exceeds 512 MiB.' -f $ComparisonPairNames[$pair], (Format-MetricValue 'peak_mib' $peak))) }
    }
    foreach ($pair in $ComparisonPairs) {
        $time = Get-SeriesMedian $Index 'single' $pair 'F1' 'process_s'
        if ($null -ne $time -and $time -gt 1) { $notes.Add(('- AC-C7: {0} F1 process time median {1} s exceeds 1 s.' -f $ComparisonPairNames[$pair], (Format-MetricValue 'process_s' $time))) }
    }
    foreach ($pair in $ComparisonPairs) {
        $peaks = @((Get-SeriesMedian $Index 'single' $pair 'F3' 'peak_mib'), (Get-SeriesMedian $Index 'single' $pair 'F4' 'peak_mib'), (Get-SeriesMedian $Index 'batch' $pair '' 'peak_mib'))
        if (@($peaks | Where-Object { $null -eq $_ }).Count -gt 0) { continue }
        $sorted = @($peaks | Sort-Object)
        if ($sorted[0] -le 0) { continue }
        $ratio = [decimal]$sorted[-1] / [decimal]$sorted[0]
        if ($ratio -gt 3) {
            $notes.Add(('- AC-M8: {0} peak ratio {1} exceeds 3 (F3 {2} MiB, F4 {3} MiB, batch {4} MiB).' -f $ComparisonPairNames[$pair], (Get-RoundedAway $ratio 2).ToString('F2', $ComparisonInvariant),
                    (Format-MetricValue 'peak_mib' $peaks[0]), (Format-MetricValue 'peak_mib' $peaks[1]), (Format-MetricValue 'peak_mib' $peaks[2])))
        }
    }
    $valid = @($ComparisonPairs | ForEach-Object { Get-KeptValue $Index $_ 'records_valid' })
    $rejected = @($ComparisonPairs | ForEach-Object { Get-KeptValue $Index $_ 'records_rejected' })
    if (@($valid + $rejected | Where-Object { $null -eq $_ }).Count -eq 0 -and ($valid[0] -ne $valid[1] -or $rejected[0] -ne $rejected[1])) {
        $notes.Add(('- Records differ: SSTables {0} valid and {1} rejected, CanopyDB {2} valid and {3} rejected.' -f `
                (Format-MetricValue 'records_valid' $valid[0]), (Format-MetricValue 'records_rejected' $rejected[0]), (Format-MetricValue 'records_valid' $valid[1]), (Format-MetricValue 'records_rejected' $rejected[1])))
    }
    foreach ($run in $Runs) {
        if (-not $run.Reason) { continue }
        $what = if ($run.Phase -eq 'single') { '{0} {1} {2}' -f $run.Phase, $ComparisonPairNames[$run.Pair], $run.Query } else { '{0} {1}' -f $run.Phase, $ComparisonPairNames[$run.Pair] }
        $notes.Add(('- Failed run: {0} run {1}: {2}.' -f $what, $run.Run, $run.Reason))
    }
    $notes.ToArray()
}

# --- The run ---

function Get-ComparisonExecutables([string]$RepoRoot) {
    $release = Join-Path $RepoRoot 'target\release'
    [ordered]@{
        'sstable generator'  = Join-Path $release 'crime-generator.exe'
        'canopydb generator' = Join-Path $release 'crime-canopy-generator.exe'
        'sstable query'      = Join-Path $release 'crime-query.exe'
        'canopydb query'     = Join-Path $release 'crime-canopy-query.exe'
    }
}

# Runs the bench and writes its report and data table; returns the exit code (0 or 1). A failed
# preliminary check throws its message before any application runs or any file is written.
function Invoke-CanopyComparison {
    param(
        [string]$RepoRoot,
        [string]$WorkDir,
        [string]$Csv,
        [int]$Repetitions,
        [switch]$NoCompression,
        [string]$Baseline,
        [switch]$SkipBuild,
        [hashtable]$Environment
    )
    $machine = $Environment
    $started = & $machine.Now

    # Preliminary checks, in the PRD's order.
    if ($Repetitions -lt 1) { throw '-Repetitions must be at least 1.' }
    if (-not (Test-Path -LiteralPath $Csv -PathType Leaf)) { throw "Missing file: $Csv" }
    $baselineIndex = $null
    if ($Baseline) {
        if (-not (Test-Path -LiteralPath $Baseline -PathType Leaf)) { throw "Baseline file not found: $Baseline" }
        $baselineRows = Read-ComparisonData $Baseline
        if ($null -eq $baselineRows) { throw "Baseline file is not a comparison bench data file: $Baseline" }
        $baselineIndex = New-RowIndex $baselineRows
    }
    $sharedCase = & $machine.SharedCase $Csv
    if (-not $sharedCase) { throw 'The CSV has no shared case number, so F2 cannot be built.' }
    $git = & $machine.Git $RepoRoot
    $commit = if ($git.Commit) { $git.Commit } else { 'unknown' }
    $reportDir = Join-Path $RepoRoot $ComparisonReportFolder
    $name = '{0}-{1}' -f $started.ToString('yyyy-MM-dd-HHmmss', $ComparisonInvariant), $commit
    $reportPath = Join-Path $reportDir "$name.md"
    $dataPath = Join-Path $reportDir "$name.csv"
    foreach ($path in @($reportPath, $dataPath)) { if (Test-Path -LiteralPath $path) { throw "Report already exists: $path" } }
    New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
    $freeBytes = & $machine.FreeBytes $WorkDir
    if ($freeBytes -lt $ComparisonRequiredGb * 1GB) {
        throw ('Only {0} free in {1}; about {2:N0} GB are needed.' -f (Format-Invariant '{0:N1} MiB' @($freeBytes / 1MB)), $WorkDir, $ComparisonRequiredGb)
    }
    if (-not $SkipBuild) {
        Write-ComparisonStep 'Building the two generators and the two query apps in release mode'
        $build = & $machine.Build $RepoRoot
        if ($build.ExitCode -ne 0) { throw "cargo build failed:`n$($build.Stderr)" }
    }
    $executables = Get-ComparisonExecutables $RepoRoot
    foreach ($path in $executables.Values) { if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing binary: $path" } }

    $work = (Resolve-Path -LiteralPath $WorkDir).Path
    $csvPath = (Resolve-Path -LiteralPath $Csv).Path
    Get-ChildItem -LiteralPath $work -Filter 'comparison-*' -Force | Remove-Item -Recurse -Force
    $tempDir = Join-Path $work 'comparison-temp'
    $logDir = Join-Path $work 'comparison-logs'
    $inputDir = Join-Path $work 'comparison-inputs'
    $outDir = Join-Path $work 'comparison-out'
    foreach ($dir in @($tempDir, $logDir, $inputDir, $outDir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
    $fixed = Read-FixedQueries (Join-Path $PSScriptRoot 'canopy-fixed-queries.txt') $sharedCase
    $queries = @($fixed.Queries)
    if ($queries.Count -ne $ComparisonQueryCount) { throw "canopy-fixed-queries.txt holds $($queries.Count) queries, not F1 to F11." }
    $batchFile = Join-Path $inputDir 'fixed-queries.txt'
    [System.IO.File]::WriteAllText($batchFile, ((@($fixed.BatchLines) -join "`n") + "`n"), $ComparisonUtf8)

    $runs = New-Object System.Collections.Generic.List[object]
    $kept = @{}
    try {
        for ($r = 1; $r -le $Repetitions; $r++) {
            foreach ($pair in $ComparisonPairs) {
                $output = Join-Path $work "comparison-$pair-generation-$r"
                $log = Join-Path $logDir "$pair-generation-$r.log"
                $arguments = @('--input', $csvPath, '--output', $output, '--temp-dir', $tempDir, '--log', $log)
                if ($NoCompression) { $arguments += '--no-compression' }
                Write-ComparisonStep ('Generation {0} of {1}: {2}' -f $r, $Repetitions, $ComparisonPairNames[$pair])
                $result = & $machine.Run $executables["$pair generator"] $arguments $null
                $run = New-GenerationRun $pair $r $result $log $output
                $runs.Add($run)
                if (-not $run.Reason) {
                    if ($kept.ContainsKey($pair)) { Remove-Item -LiteralPath $kept[$pair] -Recurse -Force }
                    $kept[$pair] = $output
                }
                elseif (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
            }
        }
        $queriesRun = $kept.ContainsKey('sstable') -and $kept.ContainsKey('canopydb')
        if ($queriesRun) {
            foreach ($query in $queries) {
                Write-ComparisonStep ('{0} alone, run 0 to {1}: {2}' -f $query.Id, $Repetitions, $query.Line)
                for ($r = 0; $r -le $Repetitions; $r++) {
                    foreach ($pair in $ComparisonPairs) {
                        $stdout = Join-Path $outDir ('{0}-single-{1}-{2}.{3}' -f $pair, $query.Id, $(if ($r -eq 0) { 'first' } else { 'repeat' }), $query.Extension)
                        $result = & $machine.Run $executables["$pair query"] (@('--dataset', $kept[$pair]) + $query.Arguments) $stdout
                        $runs.Add((New-SingleRun $pair $query $r $result $stdout))
                    }
                }
            }
            Write-ComparisonStep ('The fixed query set in a batch, run 0 to {0}' -f $Repetitions)
            for ($r = 0; $r -le $Repetitions; $r++) {
                foreach ($pair in $ComparisonPairs) {
                    $stdout = Join-Path $outDir ('{0}-batch-{1}.txt' -f $pair, $(if ($r -eq 0) { 'first' } else { 'repeat' }))
                    $result = & $machine.Run $executables["$pair query"] @('--dataset', $kept[$pair], '--batch', $batchFile) $stdout
                    $runs.Add((New-BatchRun $pair $r $result $stdout))
                }
            }
        }
    }
    finally {
        if ($machine.ContainsKey('Stop')) { & $machine.Stop }
    }

    Write-ComparisonStep 'Checking the results and writing the report'
    $rows = New-Object System.Collections.Generic.List[object]
    foreach ($run in $runs) { $rows.AddRange([object[]]@(Get-RunRows $run)) }
    $index = New-RowIndex $rows
    $checks = Get-QueryChecks $runs $queries $queriesRun $outDir
    $failed = @($runs | Where-Object { $_.Reason }).Count
    $different = @($checks.Values | Where-Object { $_ -eq 'DIFFERENT' }).Count
    $same = @($checks.Values | Where-Object { $_ -eq 'SAME' }).Count
    $outcome = if ($failed -eq 0 -and $same -eq $ComparisonQueryCount) { 'PASS: {0} runs, same results in all 11 queries' -f $runs.Count }
    elseif (-not $queriesRun) { 'FAIL: {0} of {1} runs failed, queries not run' -f $failed, $runs.Count }
    else { 'FAIL: {0} of {1} runs failed, {2} of 11 queries returned different results' -f $failed, $runs.Count, $different }

    $sstableManifest = if ($kept.ContainsKey('sstable')) { Read-ActiveManifest $kept['sstable'] } else { $null }
    $sha256 = $null
    if ($sstableManifest -and $sstableManifest.PSObject.Properties['source'] -and $sstableManifest.source.PSObject.Properties['sha256']) { $sha256 = [string]$sstableManifest.source.sha256 }
    if (-not $sha256) { $sha256 = & $machine.Sha256 $csvPath }
    $canopyManifest = if ($kept.ContainsKey('canopydb')) { Read-ActiveManifest $kept['canopydb'] } else { $null }
    $ended = & $machine.Now
    $mode = if (Test-ReferenceExtract $sha256) { 'CSV: the reference extract (SHA-256 {0}).' -f $sha256 } else { 'CSV: not the reference extract (SHA-256 {0}).' -f $sha256 }
    $compression = if ($NoCompression) { 'compression none (SSTables) and none (CanopyDB)' } else { 'compression snappy (SSTables) and lz4 (CanopyDB)' }
    $header = @(
        '# CanopyDB comparison bench'
        ''
        ('- **Result**: {0}' -f $outcome)
        ('- **Date**: {0} (duration {1})' -f $started.ToString('yyyy-MM-dd HH:mm', $ComparisonInvariant), (Format-Invariant '{0:N1} s' @(($ended - $started).TotalSeconds)))
        ('- **Commit**: {0}' -f $(if (-not $git.Commit) { 'unknown' } elseif ($git.Dirty) { "$($git.Commit) with uncommitted changes" } else { $git.Commit }))
        ('- **Machine**: {0}' -f (& $machine.Machine))
        ('- **CSV**: `{0}`, {1} bytes' -f $csvPath, (Format-Invariant '{0:N0}' @((Get-Item -LiteralPath $csvPath).Length)))
        ('- {0}' -f $mode)
        ('- **CanopyDB**: {0}' -f (Format-CanopyVersion $canopyManifest))
        ('- **CanopyDB configuration**: {0}' -f (Format-CanopySettings $RepoRoot))
        ('- **Options**: {0} repetitions, {1}, temp folder `{2}`' -f $Repetitions, $compression, $tempDir)
        ('- **Build**: {0}' -f $(if ($SkipBuild) { 'skipped (-SkipBuild)' } else { 'release build' }))
        ('- **Baseline**: {0}' -f $(if ($Baseline) { '`{0}`' -f $Baseline } else { 'none' }))
        ('- **Work folder**: `{0}`' -f $work)
        ''
    )
    $notes = @(Get-ReportNotes $index $runs)
    $sections = @(Get-ReportSections $index $queries $checks $runs $Repetitions $baselineIndex $notes)
    New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
    [System.IO.File]::WriteAllText($reportPath, ((@($header) + $sections) -join "`n") + "`n", $ComparisonUtf8)
    Write-ComparisonData $dataPath $rows
    Write-Host "Result: $outcome"
    Write-Host "Report: $reportPath"
    if ($outcome.StartsWith('PASS')) { 0 } else { 1 }
}

# --- The real environment ---

$ComparisonLive = $null
$ComparisonRepoRoot = $null

# Starts a program with its standard output in a file (or discarded) and waits for it: the exit code,
# the process time from the start to the instant a waiting thread saw it end, its peak private
# committed bytes (read after the exit, -1 when Windows does not tell) and its standard error. The
# short sleeps only let Ctrl+C through; they do not take part in the time.
function Invoke-ComparisonProgram([string]$FilePath, [string[]]$Arguments, [string]$StdoutPath) {
    $quoted = (@($Arguments) | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $script:ComparisonLive = [LiveRun]::Start($FilePath, $quoted, $script:ComparisonRepoRoot, $StdoutPath)
    $live = $script:ComparisonLive
    while (-not $live.Process.HasExited) { Start-Sleep -Milliseconds 100 }
    $exitCode = $live.Wait()
    $result = [pscustomobject]@{
        ExitCode       = $exitCode
        ProcessSeconds = [decimal]($live.ExitedAt - $live.StartedAt)
        PeakBytes      = [QueryAcceptance]::PeakPrivateBytes($live.Process.Handle)
        Stderr         = (@($live.Lines()) -join "`n")
    }
    $script:ComparisonLive = $null
    $result
}

# Ctrl+C: ends the program the bench was waiting for.
function Stop-ComparisonProgram {
    $live = $script:ComparisonLive
    if ($live -and -not $live.Process.HasExited) { try { $live.Process.Kill() } catch { } }
    $script:ComparisonLive = $null
}

function New-ComparisonEnvironment([string]$RepoRoot) {
    $script:ComparisonRepoRoot = $RepoRoot
    @{
        Now        = { Get-Date }
        Run        = { param($FilePath, $Arguments, $StdoutPath) Invoke-ComparisonProgram $FilePath $Arguments $StdoutPath }
        Stop       = { Stop-ComparisonProgram }
        Build      = {
            param($Root)
            $cargo = (Get-Command cargo).Source
            Invoke-ComparisonProgram $cargo @('build', '--release', '-p', 'crime-generator', '-p', 'crime-canopy-generator', '-p', 'crime-query', '-p', 'crime-canopy-query') $null
        }
        FreeBytes  = { param($Path) (Get-Item -LiteralPath $Path).PSDrive.Free }
        Git        = {
            param($Root)
            # git writes warnings to stderr; they must not stop the bench.
            $ErrorActionPreference = 'Continue'
            if (-not (Get-Command git -ErrorAction SilentlyContinue)) { return [pscustomobject]@{ Commit = $null; Dirty = $false } }
            $head = (& git -C $Root rev-parse --short HEAD 2>$null | Out-String).Trim()
            $status = (& git -C $Root status --porcelain -- . ':(exclude)docs/canopy-db/comparisons' 2>$null | Out-String).Trim()
            [pscustomobject]@{ Commit = $(if ($head) { $head } else { $null }); Dirty = [bool]$status }
        }
        Machine    = {
            $processor = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
            $memoryGiB = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
            $os = (Get-CimInstance Win32_OperatingSystem).Caption
            '{0}, {1} logical processors, {2} GiB RAM, {3}' -f $processor, [Environment]::ProcessorCount, $memoryGiB, $os
        }
        SharedCase = { param($Path) [QueryAcceptance]::FirstSharedCaseNumber($Path) }
        Sha256     = { param($Path) (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
}
