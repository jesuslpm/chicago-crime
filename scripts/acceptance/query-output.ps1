<#
.SYNOPSIS
    Patterns and readers of the query apps' standard error, shared by the query app acceptance
    checks and the comparison bench.

.DESCRIPTION
    Dot-sourced. Defines $StatsPattern, $BatchHeaderPattern and $BatchCompletedPattern, and the
    functions ConvertFrom-SpanishSeconds, Get-Lines, Get-Stats, Read-BatchStderr, Split-BatchWords,
    Read-JsonDocument, Read-JsonFile, Compare-JsonDocuments and Read-FixedQueries. It holds no state.
#>

$StatsPattern = '^(?<count>[0-9.]+) resultados? en (?<seconds>[0-9.]+,[0-9]{3}) s\. Índices usados: (?<indexes>.*)\. Filtros sin índice: (?<filters>.*)\.$'
$BatchHeaderPattern = '^--- Consulta (?<index>[0-9]+) de (?<total>[0-9]+) \(línea (?<line>[0-9]+)\) ---$'
$BatchCompletedPattern = '^Lote completado: (?<total>[0-9.]+) consultas? en (?<seconds>[0-9.]+,[0-9]{3}) s\.$'

# A time as the query apps print it ("1.234,567") in seconds.
function ConvertFrom-SpanishSeconds([string]$Text) {
    [decimal]::Parse($Text.Replace('.', '').Replace(',', '.'), [System.Globalization.CultureInfo]::InvariantCulture)
}

function Get-Lines([string]$Text) { @($Text -split "`r?`n" | Where-Object { $_ -ne '' }) }

# The stats line of a completed query (decision C5), or $null.
function Get-Stats([string]$Stderr) {
    foreach ($line in Get-Lines $Stderr) {
        $match = [regex]::Match($line, $StatsPattern)
        if ($match.Success) {
            return [pscustomobject]@{
                Line    = $line
                Count   = [long]$match.Groups['count'].Value.Replace('.', '')
                Indexes = $match.Groups['indexes'].Value
                Filters = $match.Groups['filters'].Value
                Seconds = ConvertFrom-SpanishSeconds $match.Groups['seconds'].Value
            }
        }
    }
    $null
}

# The stderr of a batch: the lines before the first header, one section per header with the
# lines under it, and the final line with what follows it.
function Read-BatchStderr([string]$Stderr) {
    $sections = New-Object System.Collections.Generic.List[object]
    $before = New-Object System.Collections.Generic.List[string]
    $final = $null
    $afterFinal = 0
    foreach ($line in Get-Lines $Stderr) {
        $header = [regex]::Match($line, $BatchHeaderPattern)
        $completed = [regex]::Match($line, $BatchCompletedPattern)
        if ($null -ne $final) { $afterFinal++ }
        elseif ($header.Success) {
            $sections.Add([pscustomobject]@{
                    Header = $line
                    Index  = [int]$header.Groups['index'].Value
                    Total  = [int]$header.Groups['total'].Value
                    Line   = [int]$header.Groups['line'].Value
                    Lines  = New-Object System.Collections.Generic.List[string]
                })
        }
        elseif ($completed.Success) { $final = $completed }
        elseif ($sections.Count -gt 0) { $sections[$sections.Count - 1].Lines.Add($line) }
        else { $before.Add($line) }
    }
    [pscustomobject]@{ Sections = $sections; Before = $before; Final = $final; AfterFinal = $afterFinal }
}

# The words of a batch line as the query app splits them (decision B2): spaces and tabs separate
# them, double or single quotes group a text, without escapes.
function Split-BatchWords([string]$Line) {
    $words = New-Object System.Collections.Generic.List[string]
    $word = New-Object System.Text.StringBuilder
    $inWord = $false
    $quote = [char]0
    foreach ($c in $Line.ToCharArray()) {
        if ($quote -ne [char]0) { if ($c -ceq $quote) { $quote = [char]0 } else { [void]$word.Append($c) } }
        elseif ($c -ceq ' ' -or $c -ceq "`t") { if ($inWord) { $words.Add($word.ToString()); [void]$word.Clear(); $inWord = $false } }
        elseif ($c -ceq '"' -or $c -ceq "'") { $quote = $c; $inWord = $true }
        else { [void]$word.Append($c); $inWord = $true }
    }
    if ($quote -ne [char]0) { throw "A quote is left open in the batch line: $Line" }
    if ($inWord) { $words.Add($word.ToString()) }
    , $words.ToArray()
}

# The parsed JSON document of a text, or $null.
function Read-JsonDocument([string]$Text) { try { $Text | ConvertFrom-Json } catch { $null } }

function Read-JsonFile([string]$Path) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    Read-JsonDocument ([System.IO.File]::ReadAllText($Path, (New-Object System.Text.UTF8Encoding $false)))
}

# The problem of two JSON outputs of the same query, or $null: the same interpreted query and the same
# records in any order.
function Compare-JsonDocuments($Left, $Right) {
    if ($null -eq $Left -or $null -eq $Right) { return 'a JSON document is missing or does not parse' }
    if (($Left.query | ConvertTo-Json -Depth 6 -Compress) -cne ($Right.query | ConvertTo-Json -Depth 6 -Compress)) { return 'the interpreted queries differ' }
    $leftRecords = @(@($Left.results) | ForEach-Object { $_ | ConvertTo-Json -Depth 4 -Compress } | Sort-Object)
    $rightRecords = @(@($Right.results) | ForEach-Object { $_ | ConvertTo-Json -Depth 4 -Compress } | Sort-Object)
    if ($leftRecords.Count -ne $rightRecords.Count) { return ('{0} records against {1}' -f $leftRecords.Count, $rightRecords.Count) }
    if (($leftRecords -join "`n") -cne ($rightRecords -join "`n")) { return 'the records differ' }
    $null
}

# The fixed query set of canopy-fixed-queries.txt (KC7), with <número> replaced by the CSV's first
# shared case number: the lines of the batch file (comments kept, so that the line numbers hold) and
# each query's id (F1, F2, ...), line, line number, words, kind (count, json, csv or table), output
# extension and limit (-1 without one).
function Read-FixedQueries([string]$Path, [string]$SharedCase) {
    $lines = @([System.IO.File]::ReadAllText($Path, (New-Object System.Text.UTF8Encoding $false)) -split "`r?`n")
    if ($lines.Count -gt 0 -and $lines[-1] -eq '') { $lines = @($lines | Select-Object -First ($lines.Count - 1)) }
    $batchLines = New-Object System.Collections.Generic.List[string]
    $queries = New-Object System.Collections.Generic.List[object]
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        $rest = $line.TrimStart(' ', "`t")
        if ($rest -eq '' -or $rest.StartsWith('#')) { $batchLines.Add($line); continue }
        $line = $line.Replace('<número>', $SharedCase)
        $words = Split-BatchWords $line
        $formatAt = [array]::IndexOf($words, '--format')
        $limitAt = [array]::IndexOf($words, '--limit')
        $kind = if ($words -ccontains '--count') { 'count' } elseif ($formatAt -ge 0) { $words[$formatAt + 1] } else { 'table' }
        $queries.Add([pscustomobject]@{
                Id         = 'F{0}' -f ($queries.Count + 1)
                Line       = $line
                LineNumber = $i + 1
                Arguments  = $words
                Kind       = $kind
                Extension  = if ($kind -eq 'json' -or $kind -eq 'csv') { $kind } else { 'txt' }
                Limit      = if ($limitAt -ge 0) { [long]$words[$limitAt + 1] } else { [long]-1 }
            })
        $batchLines.Add($line)
    }
    [pscustomobject]@{ Queries = $queries.ToArray(); BatchLines = $batchLines.ToArray() }
}
