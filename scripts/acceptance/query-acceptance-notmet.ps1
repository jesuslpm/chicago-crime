<#
.SYNOPSIS
    The -NotMetByCanopyDb associations and the NOT MET rule of the CanopyDB query app acceptance
    (PRD canopy-db-consultor, decision KC8, with the form of decision KG10 of canopy-db-generador).

.DESCRIPTION
    Dot-sourced by run-canopy-query-acceptance.ps1, which checks the associations before anything
    runs, and by run-query-acceptance.ps1 when it runs for CanopyDB, which classifies its rows and
    writes the overall result with these functions. AC-KC7 calls them with made-up figures.
    $RepoRoot must be set before it is dot-sourced.
#>

# The criteria that can be associated with a finding, each once (decision KC8).
$NotMetCriteria = @('AC-C7', 'AC-S8', 'AC-M8')
$FindingsRegister = Join-Path $RepoRoot 'docs\canopy-db\hallazgos.md'

# The findings of the register: the <finding> of every heading "## <finding> — <title>", in order.
function Get-RegisteredFindings {
    if (-not (Test-Path -LiteralPath $FindingsRegister -PathType Leaf)) { return @() }
    @(foreach ($line in [System.IO.File]::ReadAllLines($FindingsRegister, [System.Text.Encoding]::UTF8)) {
            $match = [regex]::Match($line, '^## (\S+) — ')
            if ($match.Success) { $match.Groups[1].Value }
        })
}

function Test-FindingInRegister([string]$Finding) { @(Get-RegisteredFindings) -ccontains $Finding }

# The reason an association is invalid, or $null; $Associated holds the criteria already associated.
function Get-AssociationError([string]$Association, [hashtable]$Associated) {
    $separator = $Association.IndexOf('=')
    if ($separator -le 0 -or $separator -eq $Association.Length - 1) { return 'expected <criterion>=<finding>' }
    $criterion = $Association.Substring(0, $separator)
    $finding = $Association.Substring($separator + 1)
    if ($NotMetCriteria -cnotcontains $criterion) { return 'only AC-C7, AC-S8 and AC-M8 can be associated' }
    if (-not (Test-FindingInRegister $finding)) { return "$finding is not in the findings register" }
    if ($Associated.ContainsKey($criterion)) { return "$criterion is already associated" }
    $null
}

# Reads the -NotMetByCanopyDb items, also split at commas so that a list passed through -File as one
# string reads the same. Returns Findings (criterion -> finding) and Error, the whole error line of
# the first invalid association or $null.
function Read-NotMetAssociations([string[]]$Items) {
    $findings = @{}
    foreach ($association in @(@($Items) | Where-Object { $null -ne $_ } | ForEach-Object { ([string]$_).Split(',') })) {
        $reason = Get-AssociationError $association $findings
        if ($reason) {
            return [pscustomobject]@{ Findings = $null; Error = "Invalid -NotMetByCanopyDb association '$association': $reason." }
        }
        $separator = $association.IndexOf('=')
        $findings[$association.Substring(0, $separator)] = $association.Substring($separator + 1)
    }
    [pscustomobject]@{ Findings = $findings; Error = $null }
}

function Format-Associations([hashtable]$Findings) {
    if ($Findings.Count -eq 0) { return 'none' }
    (@($NotMetCriteria | Where-Object { $Findings.ContainsKey($_) } | ForEach-Object { '`{0}={1}`' -f $_, $Findings[$_] }) -join ', ')
}

# The outcome of a row that can be associated: PASS when its runs and its measured figure are right;
# NOT MET only when every run ended with 0 and the expected result ($RunsOk), only the figure failed
# and $Criterion is associated; FAIL otherwise.
function Get-AssociatedOutcome([string]$Criterion, [bool]$RunsOk, [bool]$FigureOk, [hashtable]$Findings) {
    if ($RunsOk -and $FigureOk) { return 'PASS' }
    if ($RunsOk -and $Findings.ContainsKey($Criterion)) { return 'NOT MET' }
    'FAIL'
}

# The outcome of a memory row (AC-S8, AC-M8): the largest peak at most $Limit times the smallest.
# With -NotAssociable (AC-M8 (SSTables), a failure of the project, K8) it is never NOT MET.
function Get-MemoryOutcome([string]$Criterion, [double[]]$Peaks, [bool]$RunsOk, [double]$Limit, [hashtable]$Findings, [switch]$NotAssociable) {
    $min = ($Peaks | Measure-Object -Minimum).Minimum
    $max = ($Peaks | Measure-Object -Maximum).Maximum
    $ratio = if ($min -gt 0) { $max / $min } else { [double]::PositiveInfinity }
    $figureOk = $ratio -le $Limit
    $outcome = if ($NotAssociable) { if ($RunsOk -and $figureOk) { 'PASS' } else { 'FAIL' } }
    else { Get-AssociatedOutcome $Criterion $RunsOk $figureOk $Findings }
    [pscustomobject]@{ Outcome = $outcome; Ratio = $ratio; FigureOk = $figureOk }
}

# The evidence of a row, followed by "(not met by CanopyDB, <finding>)" when it is NOT MET.
function Format-NotMetEvidence([string]$Evidence, [string]$Outcome, [string]$Finding) {
    if ($Outcome -eq 'NOT MET') { '{0} (not met by CanopyDB, {1})' -f $Evidence, $Finding } else { $Evidence }
}

# The overall result line and exit code of rows in report order (each with Result and, when NOT
# MET, Finding): NOT MET rows are not failures; their findings are listed in row order, without
# repeats.
function Get-OverallResult($Rows) {
    $all = @($Rows)
    $failed = @($all | Where-Object { $_.Result -eq 'FAIL' })
    $notMet = @($all | Where-Object { $_.Result -eq 'NOT MET' })
    if ($failed.Count -gt 0) {
        return [pscustomobject]@{ Line = "FAIL: $($failed.Count) of $($all.Count) checks failed"; ExitCode = 1 }
    }
    if ($notMet.Count -gt 0) {
        $findings = New-Object System.Collections.Generic.List[string]
        foreach ($row in $notMet) { if (-not $findings.Contains([string]$row.Finding)) { $findings.Add([string]$row.Finding) } }
        $line = 'PASS: {0} of {1} checks passed, {2} not met by CanopyDB ({3})' -f ($all.Count - $notMet.Count), $all.Count, $notMet.Count, ($findings -join ', ')
        return [pscustomobject]@{ Line = $line; ExitCode = 0 }
    }
    [pscustomobject]@{ Line = "PASS: all $($all.Count) checks passed"; ExitCode = 0 }
}
