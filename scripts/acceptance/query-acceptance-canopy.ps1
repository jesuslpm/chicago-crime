<#
.SYNOPSIS
    The CanopyDB part of the query app acceptance (PRD canopy-db-consultor, decision KC8).

.DESCRIPTION
    Dot-sourced by run-query-acceptance.ps1 when run-canopy-query-acceptance.ps1 starts it with
    -CanopyDb, after its own functions and before its preparation. It holds what only the CanopyDB
    run does: the second generator and query app (the original ones, for AC-M2, AC-M8 (SSTables) and
    the SSTable datasets that must be rejected), the fixed query set (canopy-fixed-queries.txt, KC7),
    the memory runs of AC-M8 and the rows of the PRD's own criteria: AC-M2-F1..F11, AC-M5, AC-M6,
    AC-M7, AC-M8, AC-M8 (SSTables) and AC-KC2..AC-KC7. It uses the original script's variables and
    functions ($DatasetDir, Invoke-Program, Test-BatchShape, ...) as they are when it runs.
#>

$SstGeneratorExe = Join-Path $RepoRoot 'target\release\crime-generator.exe'
$SstQueryExe = Join-Path $RepoRoot 'target\release\crime-query.exe'
$CanopyPackages = @('crime-canopy-generator', 'crime-canopy-query', 'crime-canopy-acceptance', 'crime-generator', 'crime-query', 'crime-acceptance')
# Free space the CanopyDB run needs (see the help of run-canopy-query-acceptance.ps1).
$CanopyRequiredGb = 16
$FixedQueriesTemplate = Join-Path $PSScriptRoot 'canopy-fixed-queries.txt'
$FixedPlaceholder = '<número>'
# The select on the CSV that counts the expected results of F1 to F11 (F10: at most its limit).
$FixedSelects = @('fixed-f1', 'case', 'theft', 'rare', 'month', 'theft-2005-2015', 'theft-008', 'theft-month-arrest', 'prefix-ja', 'theft', 'theft')
# AC-M5: the batch that holds the base lasts about this long (more than 45 s), the short one about
# this long (less than 30 s), and Ctrl+C is pressed this long after the waiting query starts.
$HolderTargetSeconds = 90
$ShortHolderSeconds = 8
$LockCtrlCDelaySeconds = 3
# The six tables of a dataset, as --compare names its COMPARE-<table> rows.
$CompareTables = @('primary', 'date', 'primary_type', 'district', 'community_area', 'case_number')
$DetailPrefix = 'Detalle del error de CanopyDB: '

function Get-LockWarning([string]$Folder) { "Aviso: otro proceso tiene abierto el conjunto de datos en $Folder; se esperará hasta 30 s." }
function Get-StillOpenMessage([string]$Folder) { "El conjunto de datos en $Folder sigue abierto por otro proceso. Inténtalo de nuevo cuando termine." }
function Get-SchemaMessage([string]$Digits) { "El conjunto usa el esquema v$Digits y este consultor admite v2. Regenera el conjunto." }
function Get-EngineVersionMessage([string]$DatasetVersion, [string]$OwnVersion) {
    "El conjunto se escribió con CanopyDB $DatasetVersion y este consultor usa CanopyDB $OwnVersion. Regenera el conjunto."
}

# The entries of -WorkDir only the CanopyDB run creates, all canopy-*.
$CanopyCopyDir = Join-Path $WorkDir 'canopy-full-copy'
$SstDatasetDir = Join-Path $WorkDir 'canopy-full-sst'
$SmallSstDir = Join-Path $WorkDir 'canopy-small-sst'
$SmallSstSchema1Dir = Join-Path $WorkDir 'canopy-small-sst-schema1'
$FixedSwapDir = Join-Path $WorkDir 'canopy-fixed-swap'
$KcManifestDir = Join-Path $WorkDir 'canopy-kc-manifest'
$KcNoBaseDir = Join-Path $WorkDir 'canopy-kc-no-base'
$KcMissingFileDir = Join-Path $WorkDir 'canopy-kc-missing-file'
$KcFolderEntryDir = Join-Path $WorkDir 'canopy-kc-folder-entry'
$KcExtraFileDir = Join-Path $WorkDir 'canopy-kc-extra-file'
$KcReadOnlyDir = Join-Path $WorkDir 'canopy-kc-read-only'
# A -WorkDir that AC-KC7's runs with invalid associations must never create.
$KcNeverDir = Join-Path $WorkDir 'canopy-kc7-never'
$CanopyEntries = @($CanopyCopyDir, $SstDatasetDir, $SmallSstDir, $SmallSstSchema1Dir, $FixedSwapDir, $KcManifestDir, $KcNoBaseDir,
    $KcMissingFileDir, $KcFolderEntryDir, $KcExtraFileDir, $KcReadOnlyDir, $KcNeverDir)

# Set while the run goes.
$SstGeneration = $null
$SstCurrent = ''
$FixedOutDir = $null
$FixedBatchFile = $null
$FixedQueries = @()
$FixedMemoryRuns = @{}
$FixedBatchRuns = @{}
$FixedAlone = @{}

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;

public static class CanopyQueryAcceptance {
    static readonly Encoding Utf8 = new UTF8Encoding(false);

    // 64-bit FNV-1a of a line.
    static ulong Hash(string line) {
        ulong hash = 14695981039346656037UL;
        for (int i = 0; i < line.Length; i++) { hash ^= line[i]; hash *= 1099511628211UL; }
        return hash;
    }

    static List<ulong> HashRows(string path, out string header) {
        var hashes = new List<ulong>();
        using (var reader = new StreamReader(path, Utf8, true, 1 << 20)) {
            header = reader.ReadLine();
            string line;
            while ((line = reader.ReadLine()) != null) hashes.Add(Hash(line));
        }
        return hashes;
    }

    // Compares two CSV files as sets of rows without holding them: the same first line, the same
    // number of other lines and the same sorted hashes of them. Returns null when equal, otherwise
    // the difference in words.
    public static string CompareRowSetsHashed(string left, string right) {
        if (!File.Exists(left) || !File.Exists(right)) return "a file is missing";
        string leftHeader, rightHeader;
        List<ulong> a = HashRows(left, out leftHeader);
        List<ulong> b = HashRows(right, out rightHeader);
        if (leftHeader == null || rightHeader == null) return "a file is empty";
        if (leftHeader != rightHeader) return "the header lines differ";
        if (a.Count != b.Count) return String.Format("{0} rows against {1}", a.Count, b.Count);
        a.Sort();
        b.Sort();
        for (int i = 0; i < a.Count; i++) { if (a[i] != b[i]) return "the rows differ"; }
        return null;
    }

    // The IDs of a CSV listing of the query app: the first field of every row after the header.
    public static List<long> ReadCsvIds(string path) {
        var ids = new List<long>();
        using (var reader = new StreamReader(path, Utf8, true, 1 << 20)) {
            reader.ReadLine();
            string line;
            while ((line = reader.ReadLine()) != null) {
                if (line.Length == 0) continue;
                int start = line[0] == '"' ? 1 : 0;
                int end = line.IndexOf(start == 1 ? '"' : ',', start);
                if (end < 0) end = line.Length;
                ids.Add(Int64.Parse(line.Substring(start, end - start)));
            }
        }
        return ids;
    }

    // The problem of a limited listing against the unlimited one, or null: exactly limit distinct
    // IDs, all in the unlimited listing.
    public static string CheckLimited(string limitedPath, string fullPath, int limit) {
        if (!File.Exists(limitedPath) || !File.Exists(fullPath)) return "a listing is missing";
        List<long> limited = ReadCsvIds(limitedPath);
        var full = new HashSet<long>(ReadCsvIds(fullPath));
        var distinct = new HashSet<long>(limited);
        int outside = 0;
        foreach (long id in distinct) { if (!full.Contains(id)) outside++; }
        if (limited.Count != limit || distinct.Count != limit || outside > 0) {
            return String.Format("{0} records ({1} distinct, {2} outside the {3} of the unlimited listing), expected {4}", limited.Count, distinct.Count, outside, full.Count, limit);
        }
        return null;
    }

    static bool IsDigits(string line) {
        if (line.Length == 0) return false;
        foreach (char c in line) { if (c < '0' || c > '9') return false; }
        return true;
    }

    static bool StartsQuery(string line, string kind, string csvHeader) {
        if (line == null) return false;
        if (kind == "json") return line.StartsWith("{\"query\":", StringComparison.Ordinal);
        if (kind == "count") return IsDigits(line);
        if (kind == "csv") return line == csvHeader;
        return false;
    }

    static string Shorten(string line) { return line.Length > 80 ? line.Substring(0, 80) + "..." : line; }

    // Splits the standard output of a batch into one file per query, by the kind of each query: a
    // JSON document (its first line up to a line "]}", or one line ending with "[]}"), a count (one
    // line of digits) or a CSV listing (the header, then every line up to where the next query
    // starts). Returns null, or the first problem in words.
    public static string SplitBatchOutput(string path, string[] kinds, string[] targets, string csvHeader) {
        using (var reader = new StreamReader(path, Utf8, true, 1 << 20)) {
            string line = reader.ReadLine();
            for (int q = 0; q < kinds.Length; q++) {
                string kind = kinds[q];
                if (!StartsQuery(line, kind, csvHeader)) {
                    return String.Format("query {0} ({1}) does not start where expected: {2}", q + 1, kind, line == null ? "the output ends" : Shorten(line));
                }
                using (var writer = new StreamWriter(targets[q], false, Utf8, 1 << 16)) {
                    writer.NewLine = "\n";
                    writer.WriteLine(line);
                    if (kind == "json") {
                        if (!line.EndsWith("[]}", StringComparison.Ordinal)) {
                            while (true) {
                                line = reader.ReadLine();
                                if (line == null) return String.Format("query {0}: its JSON document does not end", q + 1);
                                writer.WriteLine(line);
                                if (line == "]}") break;
                            }
                        }
                        line = reader.ReadLine();
                    }
                    else if (kind == "count") {
                        line = reader.ReadLine();
                    }
                    else {
                        string next = q + 1 < kinds.Length ? kinds[q + 1] : null;
                        line = reader.ReadLine();
                        while (line != null && !(next != null && StartsQuery(line, next, csvHeader))) {
                            writer.WriteLine(line);
                            line = reader.ReadLine();
                        }
                    }
                }
            }
            if (line != null) return "output after the last query: " + Shorten(line);
        }
        return null;
    }

    // Writes head, then everything reader still gives, to path (UTF-8 without BOM).
    public static void DrainToFile(TextReader reader, string path, string head) {
        using (var writer = new StreamWriter(path, false, Utf8, 1 << 20)) {
            writer.Write(head);
            var buffer = new char[1 << 16];
            int read;
            while ((read = reader.Read(buffer, 0, buffer.Length)) > 0) writer.Write(buffer, 0, read);
        }
    }
}

// A program started with its standard input closed, its standard output copied to a file (or
// discarded) and each line of its standard error kept with the instant it arrived, on a clock
// shared by every run: what AC-M5 needs to time a wait from outside.
public sealed class LiveRun {
    static readonly Stopwatch Clock = Stopwatch.StartNew();
    public static double Now { get { return Clock.Elapsed.TotalSeconds; } }

    readonly Process process;
    readonly List<string> errorLines = new List<string>();
    readonly List<double> errorTimes = new List<double>();
    readonly object gate = new object();
    readonly Thread outputThread, errorThread, exitThread;
    readonly double startedAt;
    double exitedAt = -1;

    LiveRun(string fileName, string arguments, string workingDirectory, string stdoutPath) {
        var psi = new ProcessStartInfo(fileName, arguments);
        psi.WorkingDirectory = workingDirectory;
        psi.UseShellExecute = false;
        psi.CreateNoWindow = true;
        psi.RedirectStandardInput = true;
        psi.RedirectStandardOutput = true;
        psi.RedirectStandardError = true;
        psi.StandardOutputEncoding = new UTF8Encoding(false);
        psi.StandardErrorEncoding = new UTF8Encoding(false);
        startedAt = Now;
        process = Process.Start(psi);
        IntPtr handle = process.Handle;  // keeps the exit code and the memory counters readable
        process.StandardInput.Close();
        outputThread = new Thread(() => CopyOutput(stdoutPath));
        outputThread.IsBackground = true;
        outputThread.Start();
        errorThread = new Thread(ReadErrors);
        errorThread.IsBackground = true;
        errorThread.Start();
        exitThread = new Thread(WaitExit);
        exitThread.IsBackground = true;
        exitThread.Start();
    }

    public static LiveRun Start(string fileName, string arguments, string workingDirectory, string stdoutPath) {
        return new LiveRun(fileName, arguments, workingDirectory, stdoutPath);
    }

    void CopyOutput(string path) {
        Stream source = process.StandardOutput.BaseStream;
        if (String.IsNullOrEmpty(path)) { source.CopyTo(Stream.Null); return; }
        using (var target = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.Read, 1 << 20)) {
            source.CopyTo(target, 1 << 20);
        }
    }

    void ReadErrors() {
        string line;
        while ((line = process.StandardError.ReadLine()) != null) {
            double now = Now;
            lock (gate) { errorLines.Add(line); errorTimes.Add(now); }
        }
    }

    void WaitExit() {
        process.WaitForExit();
        double now = Now;
        lock (gate) { exitedAt = now; }
    }

    public Process Process { get { return process; } }
    public double StartedAt { get { return startedAt; } }
    // When the process ended, or -1 while it runs.
    public double ExitedAt { get { lock (gate) { return exitedAt; } } }
    public string[] Lines() { lock (gate) { return errorLines.ToArray(); } }
    public double[] Times() { lock (gate) { return errorTimes.ToArray(); } }

    // Waits until a line of standard error starts with prefix (true), or until the process ends or
    // the timeout passes without one (false).
    public bool WaitForLine(string prefix, double timeoutSeconds) {
        double deadline = Now + timeoutSeconds;
        while (true) {
            bool ended;
            lock (gate) {
                foreach (string line in errorLines) { if (line.StartsWith(prefix, StringComparison.Ordinal)) return true; }
                ended = exitedAt >= 0;
            }
            if (ended) { errorThread.Join(); }
            if (ended || Now > deadline) {
                lock (gate) { foreach (string line in errorLines) { if (line.StartsWith(prefix, StringComparison.Ordinal)) return true; } }
                return false;
            }
            Thread.Sleep(10);
        }
    }

    // Waits for the end of the process and of its output, and gives its exit code.
    public int Wait() {
        exitThread.Join();
        outputThread.Join();
        errorThread.Join();
        return process.ExitCode;
    }
}
"@

# --- Rows ---

# A row with any outcome (PASS, FAIL or NOT MET), and for NOT MET its finding.
function Add-Outcome([string]$Criterion, [string]$Outcome, [string]$Evidence, [string]$Finding) {
    $Results.Add([pscustomobject]@{ Order = $Results.Count; Criterion = $Criterion; Result = $Outcome; Evidence = $Evidence; Finding = $Finding })
    Write-Host ('{0}  {1}  {2}' -f $Outcome, $Criterion, $Evidence)
}

# A row of a criterion that can be associated with a finding ($Criterion, the key of
# -NotMetByCanopyDb), labelled $Label.
function Add-AssociableResult([string]$Label, [string]$Criterion, [string]$Outcome, [string]$Evidence) {
    $finding = if ($Outcome -eq 'NOT MET') { $NotMetFindings[$Criterion] } else { $null }
    Add-Outcome $Label $Outcome (Format-NotMetEvidence $Evidence $Outcome $finding) $finding
}

# Whether stderr ends with an engine error of the CanopyDB query app (KC5): one detail line, then
# the message, as the last two lines; before them nothing, or with -Batch only batch headers.
function Test-EngineErrorLines([string[]]$Lines, [string]$Message, [switch]$Batch) {
    $all = @($Lines)
    $count = $all.Count
    if ($count -lt 2 -or $all[$count - 1] -cne $Message -or -not $all[$count - 2].StartsWith($DetailPrefix, [System.StringComparison]::Ordinal)) { return $false }
    if (@($all | Where-Object { $_.StartsWith($DetailPrefix, [System.StringComparison]::Ordinal) }).Count -ne 1) { return $false }
    $before = @($all | Select-Object -First ($count - 2))
    if ($Batch) { return @($before | Where-Object { $_ -notmatch $BatchHeaderPattern }).Count -eq 0 }
    $before.Count -eq 0
}

function Format-Work([string]$Text) { $Text.Replace($WorkDir, '<work>') }

function Remove-Outputs([string[]]$Paths) {
    foreach ($path in $Paths) { if ($path -and (Test-Path -LiteralPath $path)) { Remove-Item -LiteralPath $path -Force } }
}

# --- Datasets ---

function Assert-CanopyBinaries {
    foreach ($path in @($GeneratorExe, $QueryExe, $VerifierExe, $AcceptanceExe, $SstGeneratorExe, $SstQueryExe)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing binary: $path" }
    }
}

function Get-CurrentDir([string]$Output) { Join-Path $Output (Get-CurrentName $Output) }

# The largest file of a version folder's base: the one that holds the data.
function Get-CanopyDataFile([string]$VersionDir) {
    $file = Get-ChildItem -LiteralPath (Join-Path $VersionDir 'canopydb') -Recurse -File | Sort-Object Length -Descending | Select-Object -First 1
    if (-not $file) { throw "No file in $VersionDir\canopydb." }
    $file.FullName
}

# Right after the CanopyDB generation: AC-M6's copy of the output folder, made before anything opens
# its base, and the SSTable dataset of the same CSV (AC-M2, AC-M8 (SSTables)).
function Initialize-CanopyFullDatasets {
    Write-Step "AC-M6: copying the CanopyDB output folder to $CanopyCopyDir before anything opens its base"
    Copy-Item -LiteralPath $DatasetDir -Destination $CanopyCopyDir -Recurse
    Write-Step "Generating the SSTable dataset with crime-generator into $SstDatasetDir"
    $script:SstGeneration = Invoke-Generator -Source $Csv -Output $SstDatasetDir -Log (Join-Path $LogDir 'full-sst.log') -Exe $SstGeneratorExe
    if ($SstGeneration.ExitCode -ne 0) { throw "crime-generator failed (exit $($SstGeneration.ExitCode)):`n$($SstGeneration.Stderr)" }
    $script:SstCurrent = Get-CurrentName $SstDatasetDir
}

# The small SSTable dataset (schema 2) and a copy of it with "schema_version": 1 (AC-C18, AC-KC2).
function Initialize-CanopySmallDatasets {
    $run = Invoke-Generator -Source $SmallCsv -Output $SmallSstDir -Log (Join-Path $LogDir 'small-sst.log') -Exe $SstGeneratorExe
    if ($run.ExitCode -ne 0) { throw "crime-generator failed on $SmallCsv`:`n$($run.Stderr)" }
    $version = Copy-Dataset $SmallSstDir $SmallSstSchema1Dir
    $path = Join-Path $version 'manifest.json'
    $text = [System.IO.File]::ReadAllText($path, $Utf8)
    $edited = [regex]::Replace($text, '"schema_version":\s*2', '"schema_version": 1')
    if ($edited -eq $text) { throw 'The SSTable manifest has no "schema_version": 2 to edit.' }
    Write-Utf8 $path $edited
}

# --- The fixed query set (KC7) ---

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

# Reads canopy-fixed-queries.txt, puts the first shared case number of the CSV in place of
# <número>, writes it as a batch file of the inputs folder and keeps each query: its id, its line
# and line number, its words, its kind (count, json or csv), its limit and its select.
function Initialize-FixedQueries {
    $script:FixedOutDir = Join-Path $OutDir 'fixed'
    New-Item -ItemType Directory -Force -Path $FixedOutDir | Out-Null
    $lines = @([System.IO.File]::ReadAllText($FixedQueriesTemplate, $Utf8) -split "`r?`n")
    if ($lines.Count -gt 0 -and $lines[-1] -eq '') { $lines = @($lines | Select-Object -First ($lines.Count - 1)) }
    $batchLines = New-Object System.Collections.Generic.List[string]
    $queries = New-Object System.Collections.Generic.List[object]
    for ($i = 0; $i -lt $lines.Count; $i++) {
        $line = $lines[$i]
        $rest = $line.TrimStart(' ', "`t")
        if ($rest -eq '' -or $rest.StartsWith('#')) { $batchLines.Add($line); continue }
        $line = $line.Replace($FixedPlaceholder, $sharedCase)
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
                Select     = if ($queries.Count -lt $FixedSelects.Count) { $FixedSelects[$queries.Count] } else { $null }
            })
        $batchLines.Add($line)
    }
    if ($queries.Count -ne $FixedSelects.Count) { throw "$FixedQueriesTemplate holds $($queries.Count) queries, not F1 to F11." }
    $script:FixedQueries = $queries.ToArray()
    $script:FixedBatchFile = New-BatchFile 'canopy-fixed-queries.txt' $batchLines.ToArray()
}

function Get-FixedQuery([string]$Id) { @($FixedQueries | Where-Object { $_.Id -eq $Id })[0] }

# The options of a query that a select takes: its conditions, without --count, --format, --limit
# and --output.
function Get-SelectArguments([string[]]$Arguments) {
    $kept = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $Arguments.Count; $i++) {
        if ($Arguments[$i] -ceq '--count') { continue }
        if (@('--format', '--limit', '--output') -ccontains $Arguments[$i]) { $i++; continue }
        $kept.Add($Arguments[$i])
    }
    , $kept.ToArray()
}

# The selects of the fixed queries that the original script does not start, and of AC-S8's second
# prefix.
function Start-CanopySelects {
    foreach ($query in $FixedQueries) {
        if (-not $Selects.Contains($query.Select)) { Start-Select $query.Select (Get-SelectArguments $query.Arguments) }
    }
    Start-Select 'prefix-jk438266' @('--startswith', 'Case Number=JK438266')
}

# The expected number of results of a fixed query: its select's count, at most its limit.
function Get-FixedExpected($Query) {
    $count = [long](Get-SelectCount $Query.Select)
    if ($Query.Limit -ge 0 -and $count -ge 0) { [math]::Min($Query.Limit, $count) } else { $count }
}

# The number of results of a fixed query run alone, from its stats line; for a count, the number on
# its standard output must be the same. -1 when it failed or they disagree.
function Get-FixedCount($Run, $Query) {
    $stats = Get-Stats $Run.Stderr
    if ($Run.ExitCode -ne 0 -or -not $stats) { return [long]-1 }
    if ($Query.Kind -eq 'count') {
        $text = if (Test-Path -LiteralPath $Run.StdoutPath) { [System.IO.File]::ReadAllText($Run.StdoutPath, $Utf8).Trim() } else { '' }
        if ($text -notmatch '^[0-9]+$' -or [long]$text -ne $stats.Count) { return [long]-1 }
    }
    $stats.Count
}

# The problems of a batch of the fixed set: its exit code, its headers and final line, and each
# query's count against its select.
function Get-FixedBatchProblems($Run) {
    $problems = New-Object System.Collections.Generic.List[string]
    if ($Run.ExitCode -ne 0) { $problems.Add("exit $($Run.ExitCode)") }
    $parsed = Read-BatchStderr $Run.Stderr
    foreach ($problem in @(Test-BatchShape $parsed $FixedQueries.Count @($FixedQueries | ForEach-Object { $_.LineNumber }) -Completed)) { $problems.Add($problem) }
    for ($i = 0; $i -lt [math]::Min($parsed.Sections.Count, $FixedQueries.Count); $i++) {
        $stats = Get-Stats ($parsed.Sections[$i].Lines -join "`n")
        $expected = Get-FixedExpected $FixedQueries[$i]
        if (-not $stats -or $stats.Count -ne $expected) {
            $problems.Add(('{0}: {1} results, expected {2}' -f $FixedQueries[$i].Id, $(if ($stats) { Format-Count $stats.Count } else { 'no stats line' }), (Format-Count $expected)))
        }
    }
    $problems.ToArray()
}

# The two query apps of AC-M2 and AC-M8, each on its dataset of the same CSV.
function Get-QueryApps {
    @(
        [pscustomobject]@{ Key = 'canopy'; Name = 'CanopyDB'; Exe = $QueryExe; Dataset = $DatasetDir; Criterion = 'AC-M8' }
        [pscustomobject]@{ Key = 'sst'; Name = 'SSTables'; Exe = $SstQueryExe; Dataset = $SstDatasetDir; Criterion = 'AC-M8 (SSTables)' }
    )
}

# --- Processes ---

function Format-ProgramArguments([string[]]$Arguments) { (@($Arguments) | ForEach-Object { '"' + $_ + '"' }) -join ' ' }

# Starts a LiveRun; $StdoutPath $null discards the standard output.
function Start-Live([string]$FilePath, [string[]]$Arguments, $StdoutPath) {
    [LiveRun]::Start($FilePath, (Format-ProgramArguments $Arguments), $RepoRoot, $StdoutPath)
}

# Runs a program with its standard output written to $StdoutPath (a listing of the full dataset
# does not fit in the script's memory) and, with -SampleMemory, its peak private committed bytes
# sampled every 10 ms and read again after the exit, as Wait-Program does.
function Invoke-ToFile([string]$FilePath, [string[]]$Arguments, [string]$StdoutPath, [switch]$SampleMemory) {
    $live = Start-Live $FilePath $Arguments $StdoutPath
    $peak = [long]0
    if ($SampleMemory) {
        while (-not $live.Process.HasExited) {
            $value = [QueryAcceptance]::PeakPrivateBytes($live.Process.Handle)
            if ($value -gt $peak) { $peak = $value }
            Start-Sleep -Milliseconds 10
        }
    }
    $exitCode = $live.Wait()
    if ($SampleMemory) {
        $value = [QueryAcceptance]::PeakPrivateBytes($live.Process.Handle)
        if ($value -gt $peak) { $peak = $value }
    }
    [pscustomobject]@{
        ExitCode    = $exitCode
        WallSeconds = $live.ExitedAt - $live.StartedAt
        PeakBytes   = $peak
        Stdout      = ''
        Stderr      = (@($live.Lines()) -join "`n")
        Arguments   = $Arguments
        StdoutPath  = $StdoutPath
    }
}

# --- AC-M8 and AC-S8: memory ---

# At the place of AC-C13 (a quiet machine, before the selects): F3 and F4 alone and the fixed set in
# a batch, with each query app on its dataset. Classified later, with the selects' counts.
function Invoke-FixedMemoryRuns {
    Initialize-FixedQueries
    Write-Step 'AC-M8: peak private memory of F3, F4 and the fixed query set in a batch, with each query app'
    foreach ($app in Get-QueryApps) {
        $runs = [ordered]@{}
        foreach ($id in @('F3', 'F4')) {
            $query = Get-FixedQuery $id
            $runs["$($app.Name): $id alone"] = Invoke-ToFile $app.Exe (@('--dataset', $app.Dataset) + $query.Arguments) (Join-Path $FixedOutDir "memory-$($app.Key)-$id.txt") -SampleMemory
        }
        $batch = Invoke-ToFile $app.Exe @('--dataset', $app.Dataset, '--batch', $FixedBatchFile) (Join-Path $FixedOutDir "batch-$($app.Key).txt") -SampleMemory
        $total = [long]0
        foreach ($section in (Read-BatchStderr $batch.Stderr).Sections) {
            $stats = Get-Stats ($section.Lines -join "`n")
            if ($stats) { $total += $stats.Count }
        }
        $batch | Add-Member -NotePropertyName ResultCount -NotePropertyValue $total
        $runs["$($app.Name): F1 to F11 in a batch"] = $batch
        $script:FixedMemoryRuns[$app.Key] = $runs
        $script:FixedBatchRuns[$app.Key] = $batch
        Write-Step ('{0}: peaks {1}' -f $app.Name, ((@($runs.Values) | ForEach-Object { Format-Mib $_.PeakBytes }) -join ', '))
    }
}

# AC-S8 of the CanopyDB query app: its two prefix counts as the selects, then the ratio (NOT MET).
function Add-PrefixMemoryRow {
    $group = Get-MemoryGroup $prefixMemoryRuns
    $hCount = Get-QueryCount $prefixMemoryRuns['count Case Number starts with H']
    $jkCount = Get-QueryCount $prefixMemoryRuns['count Case Number starts with JK438266']
    $countsOk = $hCount -ge 0 -and $hCount -eq (Get-SelectCount 'prefix-case-h') -and ($CaseHFigure -lt 0 -or $hCount -eq $CaseHFigure) `
        -and $jkCount -ge 0 -and $jkCount -eq (Get-SelectCount 'prefix-jk438266')
    $peaks = [double[]]@($prefixMemoryRuns.Values | ForEach-Object { $_.PeakBytes })
    $memory = Get-MemoryOutcome 'AC-S8' $peaks ($group.AllOk -and $countsOk) $MemoryRatioLimit $NotMetFindings
    Add-AssociableResult 'AC-S8 (Case Number prefixes)' 'AC-S8' $memory.Outcome ('{0}; largest/smallest peak = {1} (rule: at most {2}, same order whatever the number of results); each run exit 0 with the count of its select on the CSV ({3} and {4}): {5}' -f `
            ($group.Parts -join '; '), (Format-Invariant '{0:N2}' @($memory.Ratio)), $MemoryRatioLimit, (Format-Count (Get-SelectCount 'prefix-case-h')), (Format-Count (Get-SelectCount 'prefix-jk438266')), ($group.AllOk -and $countsOk))
}

# AC-M8 and AC-M8 (SSTables): each query app's three runs with the expected counts, then the ratio;
# only the CanopyDB row can be NOT MET (K8: the other is a failure of the project).
function Add-FixedMemoryRows {
    foreach ($app in Get-QueryApps) {
        $runs = $FixedMemoryRuns[$app.Key]
        $group = Get-MemoryGroup $runs
        $problems = New-Object System.Collections.Generic.List[string]
        foreach ($id in @('F3', 'F4')) {
            $query = Get-FixedQuery $id
            $count = Get-FixedCount $runs["$($app.Name): $id alone"] $query
            if ($count -ne (Get-FixedExpected $query)) { $problems.Add(('{0}: {1} results, expected {2}' -f $id, (Format-Count $count), (Format-Count (Get-FixedExpected $query)))) }
        }
        foreach ($problem in @(Get-FixedBatchProblems $FixedBatchRuns[$app.Key])) { $problems.Add("batch: $problem") }
        $runsOk = $group.AllOk -and $problems.Count -eq 0
        $peaks = [double[]]@($runs.Values | ForEach-Object { $_.PeakBytes })
        $memory = Get-MemoryOutcome 'AC-M8' $peaks $runsOk $MemoryRatioLimit $NotMetFindings -NotAssociable:($app.Key -ne 'canopy')
        $evidence = '{0}; largest/smallest peak = {1} (rule: at most {2}; the peaks of the two query apps are not compared); every run exit 0 with the counts of the selects on the CSV, the batch with its 11 headers and final line: {3}{4}' -f `
        ($group.Parts -join '; '), (Format-Invariant '{0:N2}' @($memory.Ratio)), $MemoryRatioLimit, ($problems.Count -eq 0), (Format-Problems $problems)
        if ($app.Key -eq 'canopy') { Add-AssociableResult $app.Criterion 'AC-M8' $memory.Outcome $evidence }
        else { Add-Result $app.Criterion ($memory.Outcome -eq 'PASS') $evidence }
    }
    Add-Section (@(
            '## AC-M8 and AC-S8: peak private memory'
            ''
            'Peak private committed bytes (PeakPagefileUsage, the counter behind PeakPagedMemorySize64), sampled every 10 ms and read again after the exit. The fixed set''s runs write their standard output to a file; the results of a batch are those of its 11 queries.'
            ''
            '| Query | Exit | Results | Peak private memory | Wall time |'
            '|---|---|---|---|---|'
            $memoryRows
        ) -join "`n")
}

# --- AC-M2 and AC-KC6: the fixed set with both query apps ---

function Read-JsonFile([string]$Path) {
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    Read-JsonDocument ([System.IO.File]::ReadAllText($Path, $Utf8))
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

function Get-FixedOutput([string]$Prefix, $Query) { Join-Path $FixedOutDir ('{0}-{1}.{2}' -f $Prefix, $Query.Id, $Query.Extension) }

# Each fixed query alone with one query app, its standard output in a file.
function Invoke-FixedAlone($App) {
    $runs = [ordered]@{}
    foreach ($query in $FixedQueries) {
        $runs[$query.Id] = Invoke-ToFile $App.Exe (@('--dataset', $App.Dataset) + $query.Arguments) (Get-FixedOutput "alone-$($App.Key)" $query)
    }
    $runs
}

function Test-FixedBothApps {
    Write-Step 'AC-M2: each query of the fixed set alone with both query apps'
    foreach ($app in Get-QueryApps) { $script:FixedAlone[$app.Key] = Invoke-FixedAlone $app }
    $tableRows = New-Object System.Collections.Generic.List[string]
    foreach ($query in $FixedQueries) {
        $canopy = $FixedAlone['canopy'][$query.Id]
        $sst = $FixedAlone['sst'][$query.Id]
        $canopyCount = Get-FixedCount $canopy $query
        $sstCount = Get-FixedCount $sst $query
        $problems = New-Object System.Collections.Generic.List[string]
        if ($canopyCount -lt 0) { $problems.Add("CanopyDB: exit $($canopy.ExitCode), no stats line or a count that disagrees with it") }
        if ($sstCount -lt 0) { $problems.Add("SSTables: exit $($sst.ExitCode), no stats line or a count that disagrees with it") }
        if ($canopyCount -ne $sstCount) { $problems.Add('the numbers of results differ') }
        $records = ''
        switch ($query.Id) {
            { $_ -eq 'F1' -or $_ -eq 'F2' } {
                $difference = Compare-JsonDocuments (Read-JsonFile $canopy.StdoutPath) (Read-JsonFile $sst.StdoutPath)
                if ($difference) { $problems.Add("records: $difference") }
                $records = '; the same interpreted query and records in any order: {0}' -f ($null -eq $difference)
            }
            'F10' {
                foreach ($key in @('canopy', 'sst')) {
                    $limited = [CanopyQueryAcceptance]::CheckLimited($FixedAlone[$key]['F10'].StdoutPath, $FixedAlone[$key]['F11'].StdoutPath, [int]$query.Limit)
                    if ($limited) { $problems.Add("${key}: $limited") }
                }
                $records = '; with each app {0} distinct records, all from its F11' -f (Format-Count $query.Limit)
            }
            'F11' {
                $difference = [CanopyQueryAcceptance]::CompareRowSetsHashed($canopy.StdoutPath, $sst.StdoutPath)
                if ($difference) { $problems.Add("records: $difference") }
                $records = '; the same CSV rows in any order (compared by 64-bit hashes): {0}' -f ($null -eq $difference)
            }
        }
        $ok = $problems.Count -eq 0 -and $canopyCount -ge 0
        Add-Result "AC-M2-$($query.Id)" $ok ('`{0}`: CanopyDB {1} results (exit {2}, {3}), SSTables {4} (exit {5}, {6}); select on the CSV {7}{8}{9}' -f `
                $query.Line, (Format-Count $canopyCount), $canopy.ExitCode, (Format-Seconds $canopy.WallSeconds), (Format-Count $sstCount), $sst.ExitCode, (Format-Seconds $sst.WallSeconds), `
            (Format-Count (Get-FixedExpected $query)), $records, (Format-Problems $problems))
        $tableRows.Add(('| {0} | `{1}` | {2} | {3} | {4} | {5} | {6} |' -f $query.Id, $query.Line.Replace('|', '\|'), (Format-Count $canopyCount), (Format-Seconds $canopy.WallSeconds), `
                (Format-Count $sstCount), (Format-Seconds $sst.WallSeconds), (Format-Count (Get-FixedExpected $query))))
    }
    Add-Section (@(
            '## AC-M2: the fixed query set with both query apps'
            ''
            ('`<número>` is {0}, the first case number the CSV repeats. Each query alone, its standard output in a file; results from the stats line.' -f $sharedCase)
            ''
            '| Id | Batch line | CanopyDB results | CanopyDB wall time | SSTables results | SSTables wall time | Select on the CSV |'
            '|---|---|---|---|---|---|---|'
            $tableRows
        ) -join "`n")
}

# AC-KC6: the JSON of the CanopyDB query app has the original's members and "canopydb-2".
function Test-JsonSchemaVersion {
    $problems = New-Object System.Collections.Generic.List[string]
    $shown = New-Object System.Collections.Generic.List[string]
    foreach ($id in @('F1', 'F2')) {
        $canopy = Read-JsonFile $FixedAlone['canopy'][$id].StdoutPath
        $sst = Read-JsonFile $FixedAlone['sst'][$id].StdoutPath
        if ($null -eq $canopy -or $null -eq $sst) { $problems.Add("${id}: a JSON document is missing or does not parse"); continue }
        $members = (@($canopy.PSObject.Properties | ForEach-Object { $_.Name }) -join ',')
        $sstMembers = (@($sst.PSObject.Properties | ForEach-Object { $_.Name }) -join ',')
        $datasetMembers = (@($canopy.dataset.PSObject.Properties | ForEach-Object { $_.Name }) -join ',')
        $sstDatasetMembers = (@($sst.dataset.PSObject.Properties | ForEach-Object { $_.Name }) -join ',')
        $schema = $canopy.dataset.schema_version
        if ($members -cne 'query,dataset,results' -or $members -cne $sstMembers) { $problems.Add("${id}: members $members, original $sstMembers") }
        if ($datasetMembers -cne $sstDatasetMembers) { $problems.Add("${id}: dataset members $datasetMembers, original $sstDatasetMembers") }
        if (-not ($schema -is [string]) -or $schema -cne 'canopydb-2') { $problems.Add(('{0}: schema_version {1}' -f $id, ($schema | ConvertTo-Json -Compress))) }
        if ($canopy.dataset.version -cne $current -or $canopy.dataset.generated_at -cne [string]$manifest.generated_at) { $problems.Add("${id}: dataset $($canopy.dataset | ConvertTo-Json -Compress)") }
        if ($sst.dataset.schema_version -is [string] -or $sst.dataset.schema_version -ne 2) { $problems.Add("${id}: the original query app's schema_version $($sst.dataset.schema_version | ConvertTo-Json -Compress)") }
        $shown.Add(('{0}: dataset {1}, original {2}' -f $id, ($canopy.dataset | ConvertTo-Json -Compress), ($sst.dataset | ConvertTo-Json -Compress)))
    }
    Add-Result 'AC-KC6' ($problems.Count -eq 0) ('the JSON outputs of F1 and F2 (AC-M2): members query, dataset and results as the original''s, dataset with version, generated_at and schema_version, version the CanopyDB dataset''s CURRENT {0}, generated_at its manifest''s, schema_version the text "canopydb-2" (the original''s the number 2); {1}{2}' -f `
            $current, ($shown -join '; '), (Format-Problems $problems))
}

# --- AC-M7 ---

# The batch of AC-M8 (the fixed set, on the full CanopyDB dataset) against each query alone.
function Test-FixedBatchEqualsAlone {
    $batch = $FixedBatchRuns['canopy']
    $alone = $FixedAlone['canopy']
    $problems = New-Object System.Collections.Generic.List[string]
    if ($batch.ExitCode -ne 0) { $problems.Add("exit $($batch.ExitCode)") }
    $parsed = Read-BatchStderr $batch.Stderr
    foreach ($problem in @(Test-BatchShape $parsed $FixedQueries.Count @($FixedQueries | ForEach-Object { $_.LineNumber }) -Completed)) { $problems.Add($problem) }
    for ($i = 0; $i -lt $FixedQueries.Count; $i++) {
        $section = if ($i -lt $parsed.Sections.Count) { $parsed.Sections[$i] } else { $null }
        foreach ($problem in @(Compare-BatchSection $section $alone[$FixedQueries[$i].Id])) { $problems.Add("$($FixedQueries[$i].Id): $problem") }
    }
    $targets = [string[]]@($FixedQueries | ForEach-Object { Get-FixedOutput 'batch-canopy' $_ })
    $split = [CanopyQueryAcceptance]::SplitBatchOutput($batch.StdoutPath, [string[]]@($FixedQueries | ForEach-Object { $_.Kind }), $targets, $Header)
    if ($split) { $problems.Add("stdout: $split") }
    else {
        for ($i = 0; $i -lt $FixedQueries.Count; $i++) {
            $query = $FixedQueries[$i]
            $own = switch ($query.Kind) {
                'json' {
                    $batchDocument = Read-JsonFile $targets[$i]
                    $aloneDocument = Read-JsonFile $alone[$query.Id].StdoutPath
                    $difference = Compare-JsonDocuments $batchDocument $aloneDocument
                    if (-not $difference -and ($batchDocument.dataset | ConvertTo-Json -Compress) -cne ($aloneDocument.dataset | ConvertTo-Json -Compress)) { $difference = 'the dataset members differ' }
                    $difference
                }
                'count' {
                    $batchText = [System.IO.File]::ReadAllText($targets[$i], $Utf8).Trim()
                    $aloneText = [System.IO.File]::ReadAllText($alone[$query.Id].StdoutPath, $Utf8).Trim()
                    if ($batchText -cne $aloneText) { 'count {0}, alone {1}' -f $batchText, $aloneText }
                }
                default {
                    if ($query.Limit -ge 0) { [CanopyQueryAcceptance]::CheckLimited($targets[$i], $alone['F11'].StdoutPath, [int]$query.Limit) }
                    else { [CanopyQueryAcceptance]::CompareRowSetsHashed($targets[$i], $alone[$query.Id].StdoutPath) }
                }
            }
            if ($own) { $problems.Add("$($query.Id): $own") }
        }
    }
    Add-Result 'AC-M7 (batch equals alone)' ($problems.Count -eq 0) ('`{0} --batch <work>\canopy-inputs\canopy-fixed-queries.txt` on the full dataset (the batch of AC-M8): exit {1}; {2} headers in order and the final line "{3}"; each query has the warnings and the stats line (count, indexes and filters) of the same query alone; on stdout F3 to F9 the same numbers, F1 and F2 the same JSON (query, dataset and records in any order), F11 the same CSV rows in any order and F10 {4} distinct records of F11 alone{5}' -f `
            $QueryName, $batch.ExitCode, $parsed.Sections.Count, $(if ($parsed.Final) { $parsed.Final.Value } else { '-' }), (Format-Count (Get-FixedQuery 'F10').Limit), (Format-Problems $problems))
    Remove-Outputs @($targets[-1])
}

# A copy of the swap dataset (slice 2) and a batch of the fixed set run twice, held inside its first
# F10 (its standard output is not read further) while the CanopyDB generator publishes slice 1: the
# second round's F1 and F2 run after the publication and must show the version the batch opened.
function Test-MidBatchPublication {
    Write-Step 'AC-M7: publishing a new version while a batch of the fixed set run twice is held inside its first F10'
    $null = Copy-Dataset $SwapDir $FixedSwapDir
    $old = Get-CurrentName $FixedSwapDir
    $total = 2 * $FixedQueries.Count
    $twice = @($FixedQueries) + @($FixedQueries)
    $file = New-BatchFile 'canopy-fixed-twice.txt' @($twice | ForEach-Object { $_.Line })
    $batch = Start-Program -FilePath $QueryExe -Arguments @('--dataset', $FixedSwapDir, '--batch', $file) -HoldStdout
    $start = Get-Date
    $reader = $batch.Process.StandardOutput
    $held = New-Object System.Text.StringBuilder
    $inF10 = $false
    while (-not $inF10) {
        $line = $reader.ReadLine()
        if ($null -eq $line) { break }
        [void]$held.Append($line).Append("`n")
        $inF10 = $line -ceq $Header
    }
    $heldAt = Get-Date
    $runningAtPublish = -not $batch.Process.HasExited
    $publish = Invoke-Generator -Source $Slice1 -Output $FixedSwapDir -Log (Join-Path $LogDir 'fixed-swap-new.log') -Overwrite
    $publishedAt = Get-Date
    $new = Get-CurrentName $FixedSwapDir
    $oldGone = -not (Test-Path -LiteralPath (Join-Path $FixedSwapDir $old))
    $runningAfterPublish = -not $batch.Process.HasExited
    $stdoutPath = Join-Path $FixedOutDir 'twice-stdout.txt'
    [CanopyQueryAcceptance]::DrainToFile($reader, $stdoutPath, $held.ToString())
    $done = Wait-Program $batch
    $end = Get-Date
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not $inF10) { $problems.Add('the batch never reached F10 on stdout') }
    $parsed = Read-BatchStderr $done.Stderr
    foreach ($problem in @(Test-BatchShape $parsed $total @(1..$total) -Completed)) { $problems.Add($problem) }
    $targets = [string[]]@(for ($i = 0; $i -lt $total; $i++) { Get-FixedOutput ('twice-{0}' -f ([math]::Floor($i / $FixedQueries.Count) + 1)) $twice[$i] })
    $split = [CanopyQueryAcceptance]::SplitBatchOutput($stdoutPath, [string[]]@($twice | ForEach-Object { $_.Kind }), $targets, $Header)
    $versions = New-Object System.Collections.Generic.List[string]
    $versionOk = $null -eq $split
    if ($split) { $problems.Add("stdout: $split") }
    else {
        foreach ($i in @(0, 1, $FixedQueries.Count, $FixedQueries.Count + 1)) {
            $document = Read-JsonFile $targets[$i]
            $version = if ($null -ne $document) { [string]$document.dataset.version } else { '(no JSON)' }
            $versionOk = $versionOk -and $version -ceq $old
            $versions.Add(('round {0} {1}: {2}' -f ([math]::Floor($i / $FixedQueries.Count) + 1), $twice[$i].Id, $version))
        }
    }
    # Round 2 gives what round 1 gave; alone on the new version, at least one query answers otherwise.
    $differs = $false
    $afterVersion = '-'
    for ($i = 0; $i -lt $FixedQueries.Count; $i++) {
        $query = $FixedQueries[$i]
        $first = if ($i -lt $parsed.Sections.Count) { Get-Stats ($parsed.Sections[$i].Lines -join "`n") } else { $null }
        $second = if ($i + $FixedQueries.Count -lt $parsed.Sections.Count) { Get-Stats ($parsed.Sections[$i + $FixedQueries.Count].Lines -join "`n") } else { $null }
        if (-not $first -or -not $second -or $first.Count -ne $second.Count) { $problems.Add(('{0}: round 1 "{1}", round 2 "{2}"' -f $query.Id, $(if ($first) { $first.Line } else { '-' }), $(if ($second) { $second.Line } else { '-' }))) }
        $after = Invoke-ToFile $QueryExe (@('--dataset', $FixedSwapDir) + $query.Arguments) (Get-FixedOutput 'after' $query)
        $afterStats = Get-Stats $after.Stderr
        if ($afterStats -and $second -and $afterStats.Count -ne $second.Count) { $differs = $true }
        if ($query.Id -eq 'F1') { $afterDocument = Read-JsonFile $after.StdoutPath; $afterVersion = if ($afterDocument) { [string]$afterDocument.dataset.version } else { '(no JSON)' } }
    }
    if (-not $differs) { $problems.Add('no query answers differently on the two versions, so the check cannot tell them apart') }
    if ($afterVersion -cne $new) { $problems.Add("F1 alone afterwards names version $afterVersion, not $new") }
    $switched = $publish.ExitCode -eq 0 -and $new -cne $old
    Add-Result 'AC-M7 (new version published mid-batch)' ($switched -and $runningAtPublish -and $runningAfterPublish -and $done.ExitCode -eq 0 -and $versionOk -and $problems.Count -eq 0) `
    ('`{0} --dataset canopy-fixed-swap --batch {1}` (the fixed set twice, {2} queries) on a copy of the swap dataset (version {3}, slice 2), held once its first F10 started on stdout; CanopyDB generator --overwrite with slice 1 exit {4}, CURRENT {3} -> {5}, old version folder gone: {6}; batch still running at the publication: {7}, and when the generator ended: {8}; then exit {9}, last line "{10}"; the dataset.version of each JSON output: {11} (expected {3}); round 2 gives the stats of round 1; alone afterwards F1 names {12} and some query answers otherwise: {13}{14}' -f `
            $QueryName, (Split-Path -Leaf $file), $total, $old, $publish.ExitCode, $new, $oldGone, $runningAtPublish, $runningAfterPublish, $done.ExitCode, `
        $(if ($parsed.Final) { $parsed.Final.Value } else { '-' }), ($versions -join '; '), $afterVersion, $differs, (Format-Problems $problems))
    Add-Section (@(
            '## AC-M7: timeline of the publication during a batch'
            ''
            '| Instant | Event |'
            '|---|---|'
            ('| {0:HH:mm:ss.fff} | batch of the fixed set twice started on {1} (CURRENT of the copy) |' -f $start, $old)
            ('| {0:HH:mm:ss.fff} | F1 to F9 of round 1 read and F10 started on stdout; stdout is not read further, so the batch cannot go on; still running: {1} |' -f $heldAt, $runningAtPublish)
            ('| {0:HH:mm:ss.fff} | CanopyDB generator --overwrite with slice 1 ended (exit {1}, {2}); CURRENT names {3} |' -f $publishedAt, $publish.ExitCode, (Format-Seconds $publish.WallSeconds), $new)
            ('| {0:HH:mm:ss.fff} | batch ended (exit {1}) after its output was read to the end |' -f $end, $done.ExitCode)
        ) -join "`n")
    Remove-Outputs @($stdoutPath, $targets[$FixedQueries.Count - 1], $targets[-1], (Get-FixedOutput 'after' (Get-FixedQuery 'F11')))
}

# --- AC-M5: a base held by another process ---

function Test-LockWait {
    Write-Step 'AC-M5: queries while a CanopyDB batch holds the base'
    $f3 = Get-FixedQuery 'F3'
    $f4 = Get-FixedQuery 'F4'
    $f11 = Get-FixedQuery 'F11'
    $warning = Get-LockWarning $DatasetDir
    $stillOpen = Get-StillOpenMessage $DatasetDir
    $waiterArguments = @('--dataset', $DatasetDir) + $f4.Arguments
    $expectedRare = Get-FixedExpected $f4

    # A batch of F11 repeated to last about $HolderTargetSeconds, its standard output discarded.
    $listingSeconds = [math]::Max(0.02, $FixedAlone['canopy']['F11'].WallSeconds)
    $repeat = [int][math]::Min(5000, [math]::Max(2, [math]::Ceiling($HolderTargetSeconds / $listingSeconds)))
    $holderFile = New-BatchFile 'canopy-lock-holder.txt' @(1..$repeat | ForEach-Object { $f11.Line })
    $holder = Start-Live $QueryExe @('--dataset', $DatasetDir, '--batch', $holderFile) $null
    $opened = $holder.WaitForLine('--- Consulta 1 de ', 300)
    $openedAt = [LiveRun]::Now

    # A second query: the warning, then the rejection 30 to 35 s after it.
    $rejectedStdout = Join-Path $FixedOutDir 'lock-rejected-stdout.txt'
    $waiter = Start-Live $QueryExe $waiterArguments $rejectedStdout
    $waiterExit = $waiter.Wait()
    $holderRunningAtRejection = $holder.ExitedAt -lt 0
    $waiterLines = @($waiter.Lines())
    $waiterTimes = @($waiter.Times())
    $warningAt = if ($waiterLines.Count -ge 1 -and $waiterLines[0] -ceq $warning) { $waiterTimes[0] } else { -1 }
    $delay = if ($warningAt -ge 0) { $waiter.ExitedAt - $warningAt } else { -1 }
    $rejectedEmpty = (Get-Item -LiteralPath $rejectedStdout).Length -eq 0
    $rejectedOk = $opened -and $waiterExit -eq 1 -and $waiterLines.Count -eq 2 -and $waiterLines[1] -ceq $stillOpen -and $warningAt -ge 0 `
        -and $delay -ge 30 -and $delay -le 35 -and $rejectedEmpty -and $holderRunningAtRejection
    Add-Result 'AC-M5 (still open after 30 s)' $rejectedOk ('while `{0} --batch` of {1} times F11 holds the full dataset (header 1 seen: {2}), `{3}`: exit {4}; stderr "{5}"; the rejection came {6} after the warning (rule: 30 to 35 s); stdout empty: {7}; the batch still running then: {8}' -f `
            $QueryName, $repeat, $opened, (Format-Command $QueryName $f4.Arguments), $waiterExit, (Format-Work ($waiterLines -join ' / ')), $(if ($delay -ge 0) { Format-Invariant '{0:N3} s' @($delay) } else { '(no warning)' }), $rejectedEmpty, $holderRunningAtRejection)

    # Ctrl+C during the wait, in a console of its own.
    $request = Join-Path $InputDir 'lock-ctrl-c-request.json'
    $resultFile = Join-Path $InputDir 'lock-ctrl-c-result.json'
    $stderrFile = Join-Path $InputDir 'lock-ctrl-c-stderr.txt'
    $stdoutFile = Join-Path $InputDir 'lock-ctrl-c-stdout.txt'
    [pscustomobject]@{
        Exe          = $QueryExe
        Arguments    = $waiterArguments
        DelaySeconds = $LockCtrlCDelaySeconds
        StdoutFile   = $stdoutFile
        StderrFile   = $stderrFile
        ResultFile   = $resultFile
    } | ConvertTo-Json | Set-Content -LiteralPath $request -Encoding UTF8
    $ctrlCStartedAt = [LiveRun]::Now
    Start-Process -FilePath 'powershell.exe' -ArgumentList ('-NoProfile -ExecutionPolicy Bypass -File "{0}" -RequestFile "{1}"' -f $CtrlCHelper, $request) -WindowStyle Hidden -Wait
    $holderRunningAfterCtrlC = $holder.ExitedAt -lt 0
    $ctrlC = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
    $ctrlCLines = @(Get-Lines ([System.IO.File]::ReadAllText($stderrFile, $Utf8)))
    $ctrlCStdout = [System.IO.File]::ReadAllText($stdoutFile, $Utf8)
    $ctrlCOk = $ctrlC.RunningWhenSent -and $ctrlC.Sent -and $ctrlC.ExitCode -eq 130 -and [double]$ctrlC.SecondsToExit -lt 1.0 `
        -and $ctrlCLines.Count -eq 2 -and $ctrlCLines[0] -ceq $warning -and $ctrlCLines[1] -ceq $Messages.Cancelled -and $ctrlCStdout -eq '' -and $holderRunningAfterCtrlC
    Add-Result 'AC-M5 (Ctrl+C during the wait)' $ctrlCOk ('`{0}` in a console of its own while the same batch holds the base; Ctrl+C sent at {1} s, while running: {2}; exit {3} after {4} s (rule: less than 1 s); stderr "{5}"; stdout empty: {6}; the batch still running afterwards: {7}' -f `
            (Format-Command $QueryName $f4.Arguments), $ctrlC.SentAtSeconds, $ctrlC.RunningWhenSent, $ctrlC.ExitCode, $ctrlC.SecondsToExit, (Format-Work ($ctrlCLines -join ' / ')), ($ctrlCStdout -eq ''), $holderRunningAfterCtrlC)

    # The batch ends normally, after more than 45 s.
    $holderExit = $holder.Wait()
    $holderParsed = Read-BatchStderr (@($holder.Lines()) -join "`n")
    $holderProblems = @(Test-BatchShape $holderParsed $repeat @(1..$repeat) -Completed)
    $holderSeconds = $holder.ExitedAt - $holder.StartedAt
    Add-Result 'AC-M5 (the batch ends normally)' ($holderExit -eq 0 -and $holderProblems.Count -eq 0 -and $holderSeconds -gt 45) ('the batch of {0} times F11 (F11 alone took {1}): exit {2} after {3} (rule: more than 45 s); its headers and final line "{4}"{5}' -f `
            $repeat, (Format-Seconds $listingSeconds), $holderExit, (Format-Seconds $holderSeconds), $(if ($holderParsed.Final) { $holderParsed.Final.Value } else { '-' }), (Format-Problems $holderProblems))

    # A batch shorter than 30 s: the second query warns, waits and answers.
    $countSeconds = [math]::Max(0.05, $FixedAlone['canopy']['F3'].WallSeconds)
    $shortRepeat = [int][math]::Min(400, [math]::Max(1, [math]::Floor($ShortHolderSeconds / $countSeconds)))
    $shortFile = New-BatchFile 'canopy-lock-short.txt' @(1..$shortRepeat | ForEach-Object { $f3.Line })
    $short = Start-Live $QueryExe @('--dataset', $DatasetDir, '--batch', $shortFile) $null
    $shortOpened = $short.WaitForLine('--- Consulta 1 de ', 120)
    $lateStdout = Join-Path $FixedOutDir 'lock-released-stdout.txt'
    $late = Start-Live $QueryExe $waiterArguments $lateStdout
    $lateExit = $late.Wait()
    $shortExit = $short.Wait()
    $lateLines = @($late.Lines())
    $lateTimes = @($late.Times())
    $lateText = [System.IO.File]::ReadAllText($lateStdout, $Utf8).Trim()
    $shortSeconds = $short.ExitedAt - $short.StartedAt
    $waited = $lateTimes.Count -ge 1 -and $lateTimes[0] -lt $short.ExitedAt -and $late.ExitedAt -gt $short.ExitedAt
    $releasedOk = $shortOpened -and $lateExit -eq 0 -and $lateLines.Count -eq 2 -and $lateLines[0] -ceq $warning -and $lateLines[1] -match $StatsPattern `
        -and $lateText -match '^[0-9]+$' -and [long]$lateText -eq $expectedRare -and $shortExit -eq 0 -and $shortSeconds -lt 30 -and $waited
    Add-Result 'AC-M5 (released before 30 s)' $releasedOk ('while `{0} --batch` of {1} times F3 holds the full dataset (exit {2} after {3}, rule: less than 30 s), `{4}`: exit {5}; stdout "{6}" (select on the CSV {7}); stderr "{8}"; it warned while the batch ran and answered after it ended: {9}' -f `
            $QueryName, $shortRepeat, $shortExit, (Format-Seconds $shortSeconds), (Format-Command $QueryName $f4.Arguments), $lateExit, $lateText, (Format-Count $expectedRare), (Format-Work ($lateLines -join ' / ')), $waited)

    $moments = [ordered]@{
        "batch of $repeat times F11 started"                                                      = $holder.StartedAt
        'its first header seen (the base is open)'                                               = $openedAt
        'second query started'                                                                   = $waiter.StartedAt
        'its warning'                                                                            = $warningAt
        "second query ended (exit $waiterExit)"                                                  = $waiter.ExitedAt
        "Ctrl+C query started in a console of its own; Ctrl+C $($ctrlC.SentAtSeconds) s later" = $ctrlCStartedAt
        "batch ended (exit $holderExit)"                                                         = $holder.ExitedAt
        "batch of $shortRepeat times F3 started"                                                 = $short.StartedAt
        'late query started'                                                                     = $late.StartedAt
        'its first stderr line'                                                                  = $(if ($lateTimes.Count -ge 1) { $lateTimes[0] } else { -1 })
        "short batch ended (exit $shortExit)"                                                    = $short.ExitedAt
        "late query ended (exit $lateExit)"                                                      = $late.ExitedAt
    }
    $timeline = foreach ($moment in $moments.Keys) { if ($moments[$moment] -ge 0) { Format-Invariant '| {0:N3} | {1} |' @($moments[$moment], $moment) } }
    Add-Section (@(
            '## AC-M5: timeline (seconds on the clock of the script''s runs)'
            ''
            '| Second | Event |'
            '|---|---|'
            $timeline
        ) -join "`n")
}

# --- AC-M6: reading changes nothing ---

# The relative paths under $Dir, with -NamesOnly; otherwise with each file's size and SHA-256.
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

# The PASS/FAIL/SKIP rows of a verifier report, by criterion.
function Get-Verdicts([string]$Stdout) {
    $verdicts = [ordered]@{}
    foreach ($line in Get-Lines $Stdout) {
        $match = [regex]::Match($line, '^(PASS|FAIL|SKIP)  (\S+)  (.*)$')
        if ($match.Success) { $verdicts[$match.Groups[2].Value] = [pscustomobject]@{ Outcome = $match.Groups[1].Value; Summary = $match.Groups[3].Value } }
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
        if ($verdicts.Contains($id)) { $passed = $passed -and $verdicts[$id].Outcome -eq 'PASS'; '{0} {1}' -f $table, $verdicts[$id].Summary }
        else { $passed = $false; "$table missing" }
    }
    if ($verdicts.Contains('COMPARE')) { $parts = @($parts) + ('COMPARE {0}: {1}' -f $verdicts['COMPARE'].Outcome, $verdicts['COMPARE'].Summary) }
    [pscustomobject]@{
        Passed   = $passed
        Evidence = ('--compare {0} {1}: exit {2}; {3}' -f (Split-Path $Left -Leaf), (Split-Path $Right -Leaf), $run.ExitCode, (@($parts) -join ', '))
        Stdout   = $run.Stdout
    }
}

function Test-ReadsChangeNothing {
    Write-Step 'AC-M6: the CanopyDB dataset after every query against its copy'
    $copied = Test-Path -LiteralPath (Join-Path $CanopyCopyDir 'CURRENT') -PathType Leaf
    $versionName = Get-CurrentName $DatasetDir
    $sameCurrent = $copied -and $versionName -ceq (Get-CurrentName $CanopyCopyDir)
    $namesAfter = Get-Snapshot (Join-Path $DatasetDir $versionName) -NamesOnly
    $sameNames = $sameCurrent -and $namesAfter -ceq (Get-Snapshot (Join-Path $CanopyCopyDir $versionName) -NamesOnly)
    $sameManifest = $sameCurrent -and (Get-FileSha (Join-Path (Join-Path $DatasetDir $versionName) 'manifest.json')) -eq (Get-FileSha (Join-Path (Join-Path $CanopyCopyDir $versionName) 'manifest.json'))
    $compare = if ($copied) { Invoke-Compare $DatasetDir $CanopyCopyDir } else { $null }
    Add-Result 'AC-M6' ($sameNames -and $sameManifest -and $null -ne $compare -and $compare.Passed) ('copy of the output folder made right after the generation, before anything opened its base: {0}; after every query of this run (the fixed set alone and in batches included), the version folder holds the same names as the copy ({1}): {2}; manifest.json byte-identical: {3}; {4}' -f `
            $copied, (($namesAfter -split "`n") -join ', '), $sameNames, $sameManifest, $(if ($compare) { $compare.Evidence } else { 'no comparison' }))
    if ($compare) { Add-Section (@('## AC-M6: `crime-canopy-acceptance --compare` of the dataset and its copy', '', $compare.Evidence, '', (Format-Code $compare.Stdout)) -join "`n") }
}

# --- AC-KC2 to AC-KC5: datasets that must be rejected ---

# One query (`--where ID=<first CSV ID> --count`) on a dataset: refused with exactly $Message and 1,
# nothing on stdout and the version folder byte-identical; or with -Normal, the answer 1 and the
# version folder with the same names and manifest (the engine may change its own files, K6).
function Test-DatasetCase {
    param([string]$Criterion, [string]$Row, [string]$Dataset, [string]$Message, [switch]$Normal, [string]$Note = '')
    $versionDir = Get-CurrentDir $Dataset
    $before = Get-Snapshot $versionDir
    $namesBefore = Get-Snapshot $versionDir -NamesOnly
    $manifestBefore = Get-FileSha (Join-Path $versionDir 'manifest.json')
    $run = Invoke-Program -FilePath $QueryExe -Arguments @('--dataset', $Dataset, '--where', "ID=$firstId", '--count')
    $lines = @(Get-Lines $run.Stderr)
    if ($Normal) {
        $count = Get-QueryCount $run
        $sameNames = (Get-Snapshot $versionDir -NamesOnly) -ceq $namesBefore
        $sameManifest = (Get-FileSha (Join-Path $versionDir 'manifest.json')) -eq $manifestBefore
        $ok = $run.ExitCode -eq 0 -and $count -eq 1 -and $lines.Count -eq 1 -and $lines[0] -match $StatsPattern -and $sameNames -and $sameManifest
        $evidence = '`{0}` on {1}{2}: exit {3}, count {4} (expected 1: ID {5} is the first CSV row); stderr "{6}"; afterwards the same names: {7}, the same manifest: {8}' -f `
        (Format-Query $run), (Split-Path -Leaf $Dataset), $Note, $run.ExitCode, $count, $firstId, ($lines -join ' / '), $sameNames, $sameManifest
    }
    else {
        $unchanged = (Get-Snapshot $versionDir) -ceq $before
        $ok = $run.ExitCode -eq 1 -and $lines.Count -eq 1 -and $lines[0] -ceq $Message -and $run.Stdout -eq '' -and $unchanged
        $evidence = '`{0}` on {1}{2}: exit {3}; stderr "{4}" (one line, expected "{5}"); stdout empty: {6}; the version folder byte-identical afterwards: {7}' -f `
        (Format-Query $run), (Split-Path -Leaf $Dataset), $Note, $run.ExitCode, (Format-Work ($lines -join ' / ')), (Format-Work $Message), ($run.Stdout -eq ''), $unchanged
    }
    Add-Result "$Criterion $Row" $ok $evidence
}

# Runs each case on the manifest of $Dataset: writes the edited manifest, runs Test-DatasetCase and
# puts the original bytes back. Each case: Row, Edit (a script block from the text to the edited
# text), Message ($null: answers normally).
function Test-ManifestCases([string]$Criterion, [string]$Dataset, $Cases) {
    $path = Join-Path (Get-CurrentDir $Dataset) 'manifest.json'
    $original = [System.IO.File]::ReadAllBytes($path)
    $text = [System.IO.File]::ReadAllText($path, $Utf8)
    foreach ($case in $Cases) {
        $edited = & $case.Edit $text
        if ($edited -ceq $text) { Add-Result "$Criterion $($case.Row)" $false 'the manifest edit did not apply'; continue }
        [System.IO.File]::WriteAllText($path, $edited, $Utf8)
        try {
            if ($null -eq $case.Message) { Test-DatasetCase $Criterion $case.Row $Dataset $null -Normal }
            else { Test-DatasetCase $Criterion $case.Row $Dataset $case.Message }
        }
        finally { [System.IO.File]::WriteAllBytes($path, $original) }
    }
}

function New-ManifestCase([string]$Row, [scriptblock]$Edit, $Message) { [pscustomobject]@{ Row = $Row; Edit = $Edit; Message = $Message } }

# AC-KC2: SSTable datasets, other schema versions and manifests without the CanopyDB shape.
function Test-SchemaRejections {
    Write-Step 'AC-KC2: SSTable datasets, other schema versions and other manifest shapes'
    Test-DatasetCase 'AC-KC2' '(SSTable dataset of schema 2)' $SmallSstDir (Get-NotFoundMessage $SmallSstDir)
    Test-DatasetCase 'AC-KC2' '(SSTable dataset of schema 1)' $SmallSstSchema1Dir (Get-NotFoundMessage $SmallSstSchema1Dir)
    $null = Copy-Dataset $SmallDir $KcManifestDir
    $notFound = Get-NotFoundMessage $KcManifestDir
    $schema = '"schema_version":\s*"canopydb-2"'
    $cases = @(
        New-ManifestCase '(schema_version canopydb-3)' { param($t) [regex]::Replace($t, $schema, '"schema_version": "canopydb-3"') } (Get-SchemaMessage '3')
        New-ManifestCase '(schema_version canopydb-02)' { param($t) [regex]::Replace($t, $schema, '"schema_version": "canopydb-02"') } (Get-SchemaMessage '02')
        New-ManifestCase '(schema_version canopydb-x)' { param($t) [regex]::Replace($t, $schema, '"schema_version": "canopydb-x"') } $notFound
        New-ManifestCase '(schema_version canopydb-)' { param($t) [regex]::Replace($t, $schema, '"schema_version": "canopydb-"') } $notFound
        New-ManifestCase '(schema_version CANOPYDB-2)' { param($t) [regex]::Replace($t, $schema, '"schema_version": "CANOPYDB-2"') } $notFound
        New-ManifestCase '(schema_version the number 2)' { param($t) [regex]::Replace($t, $schema, '"schema_version": 2') } $notFound
        New-ManifestCase '(schema_version the text "2")' { param($t) [regex]::Replace($t, $schema, '"schema_version": "2"') } $notFound
        New-ManifestCase '(without schema_version)' { param($t) [regex]::Replace($t, '\s*"schema_version":\s*"canopydb-2",', '') } $notFound
        New-ManifestCase '(without tables)' { param($t) [regex]::Replace($t, ',\s*"tables":\s*\[[^\]]*\]', '') } $notFound
        New-ManifestCase '(five tables)' { param($t) [regex]::Replace($t, ',\s*\{\s*"name":\s*"case_number"[^}]*\}', '') } $notFound
        New-ManifestCase '(the tables in another order)' { param($t)
            $swapped = [regex]::Replace($t, '"name":\s*"primary"', '"name": "<first>"')
            $swapped = [regex]::Replace($swapped, '"name":\s*"case_number"', '"name": "primary"')
            $swapped.Replace('"name": "<first>"', '"name": "case_number"') } $notFound
        New-ManifestCase '(another engine.name)' { param($t) [regex]::Replace($t, '"name":\s*"canopydb"', '"name": "sqlite"') } $notFound
        New-ManifestCase '(without engine.fork_commit)' { param($t) [regex]::Replace($t, ',\s*"fork_commit":\s*(null|"[^"]*")', '') } $notFound
        New-ManifestCase '(an unknown field added)' { param($t) [regex]::Replace($t, '^\{', "{`n  `"added_by_the_acceptance`": `"an unknown field`",") } $null
    )
    Test-ManifestCases 'AC-KC2' $KcManifestDir $cases
}

# AC-KC3: a dataset written with another CanopyDB version or fork.
function Test-OtherEngineVersion {
    Write-Step 'AC-KC3: another CanopyDB version'
    $engine = (Get-Content -LiteralPath (Join-Path (Get-CurrentDir $KcManifestDir) 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json).engine
    $fork = '0123456789abcdef0123456789abcdef01234567'
    $own = if ($null -eq $engine.fork_commit) { [string]$engine.version } else { '{0} (fork {1})' -f $engine.version, $engine.fork_commit }
    $olderText = if ($null -eq $engine.fork_commit) { '0.2.4' } else { '0.2.4 (fork {0})' -f $engine.fork_commit }
    $cases = @(
        New-ManifestCase '(engine.version 0.2.4)' { param($t) [regex]::Replace($t, '"version":\s*"[^"]*"', '"version": "0.2.4"') } (Get-EngineVersionMessage $olderText $own)
        New-ManifestCase '(a fork_commit of 40 characters)' { param($t) [regex]::Replace($t, '"fork_commit":\s*(null|"[^"]*")', ('"fork_commit": "{0}"' -f $fork)) } (Get-EngineVersionMessage ('{0} (fork {1})' -f $engine.version, $fork) $own)
    )
    Test-ManifestCases 'AC-KC3' $KcManifestDir $cases
}

# AC-KC4: a missing base, a missing file, a listed path that is a folder, a file not listed.
function Test-MissingBase {
    Write-Step 'AC-KC4: a missing base, a missing file, a folder in place of a file, a file not listed'
    $listed = @((Get-Content -LiteralPath (Join-Path (Get-CurrentDir $SmallDir) 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json).files | ForEach-Object { [string]$_.path })
    $victim = $listed[-1]
    $version = Copy-Dataset $SmallDir $KcNoBaseDir
    Remove-Item -LiteralPath (Join-Path $version 'canopydb') -Recurse -Force
    Test-DatasetCase 'AC-KC4' '(without canopydb/)' $KcNoBaseDir (Get-NotFoundMessage $KcNoBaseDir)
    $version = Copy-Dataset $SmallDir $KcMissingFileDir
    Remove-Item -LiteralPath (Join-Path $version $victim.Replace('/', '\')) -Force
    Test-DatasetCase 'AC-KC4' '(a file of files missing)' $KcMissingFileDir (Get-NotFoundMessage $KcMissingFileDir) -Note " without $victim"
    $version = Copy-Dataset $SmallDir $KcFolderEntryDir
    $path = Join-Path $version $victim.Replace('/', '\')
    Remove-Item -LiteralPath $path -Force
    New-Item -ItemType Directory -Path $path | Out-Null
    Test-DatasetCase 'AC-KC4' '(a path of files that is a folder)' $KcFolderEntryDir (Get-NotFoundMessage $KcFolderEntryDir) -Note " with a folder at $victim"
    $version = Copy-Dataset $SmallDir $KcExtraFileDir
    Write-Utf8 (Join-Path $version 'canopydb\not-listed.txt') "a file that files does not list`n"
    Test-DatasetCase 'AC-KC4' '(a file in canopydb/ that files does not list)' $KcExtraFileDir $null -Normal -Note ' with canopydb/not-listed.txt'
    Add-Section (@(
            '## AC-KC4: a base without one of the six tables'
            ''
            'This script cannot make that dataset: the CanopyDB generator always writes the six tables and no other program writes a CanopyDB base. The tests of crime-canopy-query cover that case.'
        ) -join "`n")
}

# AC-KC5: engine errors from outside: read-only files of canopydb/ (Windows) and a damaged base, each
# alone and in a batch.
function Test-EngineErrors {
    Write-Step 'AC-KC5: read-only files and a damaged base'
    $version = Copy-Dataset $SmallDir $KcReadOnlyDir
    $files = @(Get-ChildItem -LiteralPath (Join-Path $version 'canopydb') -Recurse -File)
    foreach ($file in $files) { $file.IsReadOnly = $true }
    $before = Get-Snapshot $version
    $message = Get-UnreadableMessage $KcReadOnlyDir
    $batchFile = New-BatchFile 'canopy-kc5-read-only.txt' @((Format-BatchLine @('--where', "ID=$firstId", '--count')))
    $single = Invoke-Program -FilePath $QueryExe -Arguments @('--dataset', $KcReadOnlyDir, '--where', "ID=$firstId", '--count')
    $batch = Invoke-Program -FilePath $QueryExe -Arguments @('--dataset', $KcReadOnlyDir, '--batch', $batchFile)
    $unchanged = (Get-Snapshot $version) -ceq $before
    $stillReadOnly = @(Get-ChildItem -LiteralPath (Join-Path $version 'canopydb') -Recurse -File | Where-Object { -not $_.IsReadOnly }).Count -eq 0
    foreach ($file in @(Get-ChildItem -LiteralPath (Join-Path $version 'canopydb') -Recurse -File)) { $file.IsReadOnly = $false }
    $singleLines = @(Get-Lines $single.Stderr)
    $batchLines = @(Get-Lines $batch.Stderr)
    $singleOk = $single.ExitCode -eq 1 -and (Test-EngineErrorLines $singleLines $message) -and $single.Stdout -eq ''
    $batchOk = $batch.ExitCode -eq 1 -and (Test-EngineErrorLines $batchLines $message) -and $batch.Stdout -eq ''
    Add-Result 'AC-KC5 (read-only files)' ($singleOk -and $batchOk -and $unchanged -and $stillReadOnly) ('a copy of the small dataset with its {0} files of canopydb/ read-only: `{1}` exit {2}, stderr "{3}"; the same query as a batch exit {4}, stderr "{5}" (before any header); stdout empty: {6}; the files byte-identical afterwards: {7}, still read-only: {8}' -f `
            $files.Count, (Format-Query $single), $single.ExitCode, (Format-Work ($singleLines -join ' / ')), $batch.ExitCode, (Format-Work ($batchLines -join ' / ')), ($single.Stdout -eq '' -and $batch.Stdout -eq ''), $unchanged, $stillReadOnly)

    $outDir = Join-Path $OutDir 'kc5-damaged'
    New-Item -ItemType Directory -Path $outDir | Out-Null
    $message = Get-UnreadableMessage $DamagedDir
    $single = Invoke-Program -FilePath $QueryExe -Arguments @('--dataset', $DamagedDir, '--range', $Everything, '--format', 'csv', '--output', (Join-Path $outDir 'single.csv'))
    $batchLines = @((Format-BatchLine @('--range', $Everything, '--format', 'csv', '--output', (Join-Path $outDir 'batch.csv'))))
    $batch = Invoke-Program -FilePath $QueryExe -Arguments @('--dataset', $DamagedDir, '--batch', (New-BatchFile 'canopy-kc5-damaged.txt' $batchLines))
    $left = @(Get-EntryNames $outDir)
    $singleLines = @(Get-Lines $single.Stderr)
    $batchErrLines = @(Get-Lines $batch.Stderr)
    $singleOk = $single.ExitCode -eq 1 -and (Test-EngineErrorLines $singleLines $message) -and $single.Stdout -eq ''
    $batchOk = $batch.ExitCode -eq 1 -and (Test-EngineErrorLines $batchErrLines $message -Batch) -and $batch.Stdout -eq ''
    Add-Result 'AC-KC5 (damaged base)' ($singleOk -and $batchOk -and $left.Count -eq 0) ('the damaged copy of AC-C10 ({0} overwritten): `{1}` exit {2}, stderr "{3}"; the same query as a batch exit {4}, stderr "{5}"; stdout empty: {6}; the output folder holds afterwards: {7}' -f `
            $damagedName, (Format-Query $single), $single.ExitCode, (Format-Work ($singleLines -join ' / ')), $batch.ExitCode, (Format-Work ($batchErrLines -join ' / ')), ($single.Stdout -eq '' -and $batch.Stdout -eq ''), $(if ($left.Count -eq 0) { 'nothing' } else { $left -join ', ' }))
}

# --- AC-KC7: the NOT MET rule ---

function New-FabricatedRow([string]$Criterion, [string]$Outcome, [string]$Finding) { [pscustomobject]@{ Criterion = $Criterion; Result = $Outcome; Finding = $Finding } }

function Test-NotMetRule {
    Write-Step 'AC-KC7: the NOT MET rule with made-up figures, and invalid associations'
    $registered = @(Get-RegisteredFindings)
    $problems = New-Object System.Collections.Generic.List[string]
    $cases = New-Object System.Collections.Generic.List[string]
    if ($registered.Count -eq 0) { $problems.Add('the findings register has no finding') }
    else {
        $first = $registered[0]
        $second = if ($registered.Count -ge 2) { $registered[1] } else { $first }
        $over = [double[]]@(20MB, 25MB, 70MB)
        $within = [double[]]@(20MB, 25MB, 50MB)
        $passed = @(1..9 | ForEach-Object { New-FabricatedRow "AC-X$_" 'PASS' $null })
        $expect = {
            param([string]$Name, [string]$Actual, [string]$Expected)
            $cases.Add(('{0}: "{1}"' -f $Name, $Actual))
            if ($Actual -cne $Expected) { $problems.Add(('{0}: "{1}", expected "{2}"' -f $Name, $Actual, $Expected)) }
        }
        $findings = @{ 'AC-M8' = $first }
        $notMet = Get-MemoryOutcome 'AC-M8' $over $true $MemoryRatioLimit $findings
        & $expect 'AC-M8=<finding>, peaks 20, 25 and 70 MiB' $notMet.Outcome 'NOT MET'
        $overall = Get-OverallResult ($passed + @((New-FabricatedRow 'AC-M8' $notMet.Outcome $first), (New-FabricatedRow 'AC-M8 (SSTables)' 'PASS' $null)))
        & $expect '  result' ('{0} (exit {1})' -f $overall.Line, $overall.ExitCode) ('PASS: 10 of 11 checks passed, 1 not met by CanopyDB ({0}) (exit 0)' -f $first)
        & $expect '  evidence' (Format-NotMetEvidence 'peaks' 'NOT MET' $first) ('peaks (not met by CanopyDB, {0})' -f $first)
        $two = @{ 'AC-C7' = $first; 'AC-M8' = $second }
        $time = Get-AssociatedOutcome 'AC-C7' $true $false $two
        $memory = Get-MemoryOutcome 'AC-M8' $over $true $MemoryRatioLimit $two
        & $expect 'AC-C7=<finding> with only the time failing' $time 'NOT MET'
        $overall = Get-OverallResult (@(New-FabricatedRow 'AC-C7' $time $first) + $passed + @(New-FabricatedRow 'AC-M8' $memory.Outcome $second))
        $listed = if ($second -ceq $first) { $first } else { '{0}, {1}' -f $first, $second }
        & $expect '  with AC-M8=<another finding>' ('{0} (exit {1})' -f $overall.Line, $overall.ExitCode) ('PASS: 9 of 11 checks passed, 2 not met by CanopyDB ({0}) (exit 0)' -f $listed)
        $overall = Get-OverallResult (@(New-FabricatedRow 'AC-C7' 'NOT MET' $first) + $passed + @(New-FabricatedRow 'AC-M8' 'NOT MET' $first))
        & $expect '  with the same finding twice' ('{0} (exit {1})' -f $overall.Line, $overall.ExitCode) ('PASS: 9 of 11 checks passed, 2 not met by CanopyDB ({0}) (exit 0)' -f $first)
        $sst = Get-MemoryOutcome 'AC-M8' $over $true $MemoryRatioLimit $findings -NotAssociable
        & $expect 'AC-M8 (SSTables) over the ratio, AC-M8=<finding>' $sst.Outcome 'FAIL'
        $overall = Get-OverallResult ($passed + @((New-FabricatedRow 'AC-M8' 'PASS' $null), (New-FabricatedRow 'AC-M8 (SSTables)' $sst.Outcome $null)))
        & $expect '  result' ('{0} (exit {1})' -f $overall.Line, $overall.ExitCode) 'FAIL: 1 of 11 checks failed (exit 1)'
        & $expect 'AC-M8=<finding> with a run that failed' (Get-MemoryOutcome 'AC-M8' $over $false $MemoryRatioLimit $findings).Outcome 'FAIL'
        & $expect 'AC-M8 over the ratio without an association' (Get-MemoryOutcome 'AC-M8' $over $true $MemoryRatioLimit @{}).Outcome 'FAIL'
        & $expect 'AC-M8=<finding> within the ratio' (Get-MemoryOutcome 'AC-M8' $within $true $MemoryRatioLimit $findings).Outcome 'PASS'
        & $expect 'AC-C7=<finding> with a run that failed' (Get-AssociatedOutcome 'AC-C7' $false $false $two) 'FAIL'
        & $expect 'AC-S8=<finding> with only the ratio failing' (Get-MemoryOutcome 'AC-S8' $over $true $MemoryRatioLimit @{ 'AC-S8' = $first }).Outcome 'NOT MET'
    }
    Add-Result 'AC-KC7 (NOT MET rule)' ($problems.Count -eq 0) ('the classification and result functions of this script with made-up figures (<finding> = {0}): {1}{2}' -f `
            $(if ($registered.Count) { $registered[0] } else { '-' }), ($cases -join '; '), (Format-Problems $problems))

    # Invalid associations: each start ends with 1 and the error before anything runs.
    $entry = Join-Path $PSScriptRoot 'run-canopy-query-acceptance.ps1'
    $powershell = Join-Path $PSHOME 'powershell.exe'
    $first = if ($registered.Count) { $registered[0] } else { 'H1' }
    $numbers = @($registered | ForEach-Object { if ($_ -match '^H([0-9]+)$') { [int]$Matches[1] } })
    $absent = 'H{0}' -f ($(if ($numbers.Count) { ($numbers | Measure-Object -Maximum).Maximum } else { 0 }) + 1)
    # Each case: what is passed, the association the error names, the reason.
    $invalid = @(
        , @("AC-C13=$first", "AC-C13=$first", 'only AC-C7, AC-S8 and AC-M8 can be associated')
        , @("AC-M8=$absent", "AC-M8=$absent", "$absent is not in the findings register")
        , @("AC-M8=$first,AC-M8=$first", "AC-M8=$first", 'AC-M8 is already associated')
        , @('AC-M8', 'AC-M8', 'expected <criterion>=<finding>')
    )
    $parts = New-Object System.Collections.Generic.List[string]
    $invalidOk = $true
    foreach ($case in $invalid) {
        $run = Invoke-Program -FilePath $powershell -Arguments @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $entry, '-WorkDir', $KcNeverDir, '-NotMetByCanopyDb', $case[0])
        $expected = "Invalid -NotMetByCanopyDb association '$($case[1])': $($case[2])."
        $lines = @(Get-Lines $run.Stderr)
        $created = Test-Path -LiteralPath $KcNeverDir
        $ok = $run.ExitCode -eq 1 -and $lines.Count -eq 1 -and $lines[0] -ceq $expected -and $run.Stdout.Trim() -eq '' -and -not $created
        $invalidOk = $invalidOk -and $ok
        $parts.Add(('-NotMetByCanopyDb {0}: exit {1} in {2}, stderr "{3}", -WorkDir created: {4}' -f $case[0], $run.ExitCode, (Format-Seconds $run.WallSeconds), ($lines -join ' / '), $created))
    }
    Add-Result 'AC-KC7 (invalid associations)' $invalidOk ('`run-canopy-query-acceptance.ps1 -WorkDir <work>\canopy-kc7-never` with an invalid association, each ending with 1, exactly its error line on stderr, nothing on stdout and no -WorkDir: {0}' -f ($parts -join '; '))
}

# --- The CanopyDB rows, after every row of the original script ---

function Invoke-CanopyChecks {
    Add-FixedMemoryRows
    Test-FixedBothApps
    Test-JsonSchemaVersion
    Test-FixedBatchEqualsAlone
    Test-MidBatchPublication
    Test-LockWait
    Test-ReadsChangeNothing
    Test-SchemaRejections
    Test-OtherEngineVersion
    Test-MissingBase
    Test-EngineErrors
    Test-NotMetRule
    # The listings of F11 of the full dataset are no longer needed (about 0.6 GB each).
    $f11 = Get-FixedQuery 'F11'
    Remove-Outputs @((Get-FixedOutput 'alone-canopy' $f11), (Get-FixedOutput 'alone-sst' $f11), $FixedBatchRuns['canopy'].StdoutPath, $FixedBatchRuns['sst'].StdoutPath)
}

# The report header lines of the CanopyDB run.
function Get-CanopyReportHeader {
    ('- **SSTable dataset** (crime-generator, for AC-M2, AC-M8 (SSTables) and AC-KC6): `{0}`, version {1}, generated in {2}' -f $SstDatasetDir, $SstCurrent, (Format-Seconds $SstGeneration.WallSeconds))
    ('- **Fixed query set**: `scripts/acceptance/canopy-fixed-queries.txt`, with `<número>` = {0}' -f $sharedCase)
    ('- **Not met by CanopyDB** (`-NotMetByCanopyDb`): {0}' -f (Format-Associations $NotMetFindings))
}
