<#
.SYNOPSIS
    Patterns and readers of the query apps' standard error, shared by the query app acceptance
    checks and the comparison bench.

.DESCRIPTION
    Dot-sourced. Defines $StatsPattern, $BatchHeaderPattern and $BatchCompletedPattern, and the
    functions Get-Lines, Get-Stats, Read-BatchStderr and Split-BatchWords. It holds no state.
#>

$StatsPattern = '^(?<count>[0-9.]+) resultados? en [0-9.]+,[0-9]{3} s\. Índices usados: (?<indexes>.*)\. Filtros sin índice: (?<filters>.*)\.$'
$BatchHeaderPattern = '^--- Consulta (?<index>[0-9]+) de (?<total>[0-9]+) \(línea (?<line>[0-9]+)\) ---$'
$BatchCompletedPattern = '^Lote completado: (?<total>[0-9.]+) consultas? en [0-9.]+,[0-9]{3} s\.$'

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
