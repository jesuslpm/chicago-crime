<#
.SYNOPSIS
    Acceptance of the crime query app on any compatible CSV: the query app PRD (AC-C1 to AC-C18)
    and the startswith PRD of its prefix conditions (AC-S1 to AC-S12).

.DESCRIPTION
    Both PRDs share one run: the AC-C rows are the consultor criteria repeated on the build that
    holds the prefix conditions, and give AC-S12 ("los resultados no cambian").

    On the reference extract, recognized by the source SHA-256 in the dataset's manifest, it also
    requires the exact figures of the PRDs; on any other CSV every count is only compared with the
    independent filtering of that CSV.

    Builds the generator, the query app and the independent verifier in release mode and, inside
    -WorkDir:

      dataset    generates the full dataset (schema 2) into a fresh folder, plus three small
                 datasets made from slices of the CSV: "small" (its first rows), whose copies are
                 edited to schema 1 (AC-C18) or damaged (AC-C10, AC-C15), and "swap", which holds
                 version 1 of AC-C12;
      selects    every count the query app gives is compared with `crime-acceptance select` on the
                 CSV, the independent filtering that uses no query app code; on the reference
                 extract the fixed PRD figures (1.838.502 THEFT, 613.720 without community area)
                 are asserted too;
      AC-C1..18  one PASS/FAIL row per criterion, several for some of them: AC-C10 has one row per
                 validation of the parent and child query tables, each provoked through the binary
                 with its exact message and the exit code of decision C6;
      AC-S1..12  the same for the prefix conditions (`--startswith`): every count is compared with
                 `crime-acceptance select --startswith` on the CSV, and on the reference extract the
                 PRD figures (1.256.646 CRIM, 269.655 JA, 3.095.148 THEFT or CRIM, 5.762.199 H) are
                 asserted too; AC-S11 has one row per row of the PRD's validation table; AC-S12
                 passes when every AC-C row passed;
      verifier   crime-acceptance on the full dataset, with --reference on the reference extract
                 (AC-C18).

    Two checks follow the state of a run, not the clock: AC-C10's Ctrl+C is pressed (a real
    console Ctrl+C, see send-ctrl-c.ps1) once the query's temporary output file holds 1 MiB, and
    AC-C12 publishes version 2 while the query on version 1 is held in flight: its standard output
    is read only up to its first rows, so it cannot finish, and is read to the end once CURRENT has
    switched. AC-C12 passes only if the publication completed during the query: CURRENT names
    version 2 and the version 1 folder is gone while the query still runs.

    AC-C1 reads "el primero, el último" of the parent AC3 as the first and the last CSV rows; the
    generator verifier's AC-G3-SAMPLE (crime-acceptance, run by run-acceptance.ps1 and by the
    verifier step here) reads them as the smallest and the largest IDs. Both readings are checked
    across the two scripts.

    Memory (AC-C13, AC-S8) is the peak private committed bytes (PeakPagefileUsage, the counter behind
    PeakPagedMemorySize64), sampled every 10 ms and read once more after the process ends.

    It writes query-acceptance-report.md to -WorkDir and exits with 1 when any check fails. The
    CSV is only read. Only the entries this script creates in -WorkDir are deleted when
    it starts.

.PARAMETER WorkDir
    Folder for the datasets, the inputs, the outputs and the report. It needs about 5 GB free.

.PARAMETER Csv
    The CSV: the reference extract or any later download. By default, chicago-crime-data.csv at
    the repository root.

.PARAMETER SliceRows
    Rows of each of the two CSV slices behind the versions of AC-C12.

.PARAMETER CtrlCTimeoutSeconds
    How long AC-C10's Ctrl+C waits for the query's output file to reach 1 MiB. If it does not by
    then, Ctrl+C is pressed anyway and the check fails.

.PARAMETER SkipBuild
    Use the release binaries already built.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/acceptance/run-query-acceptance.ps1 -WorkDir C:\query-acceptance
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [string]$Csv,
    [int]$SliceRows = 1000000,
    [int]$CtrlCTimeoutSeconds = 300,
    [switch]$SkipBuild
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
if (-not $Csv) { $Csv = Join-Path $RepoRoot 'chicago-crime-data.csv' }
$Csv = (Resolve-Path -LiteralPath $Csv).Path
$WorkDir = [System.IO.Path]::GetFullPath($WorkDir).TrimEnd('\')
$GeneratorExe = Join-Path $RepoRoot 'target\release\crime-generator.exe'
$QueryExe = Join-Path $RepoRoot 'target\release\crime-query.exe'
$AcceptanceExe = Join-Path $RepoRoot 'target\release\crime-acceptance.exe'
$CtrlCHelper = Join-Path $PSScriptRoot 'send-ctrl-c.ps1'
. (Join-Path $PSScriptRoot 'reference-extract.ps1')
$Invariant = [System.Globalization.CultureInfo]::InvariantCulture
$Utf8 = New-Object System.Text.UTF8Encoding $false

# Fixed figures of the reference extract (parent PRD, AC4 and AC5); -1 on another CSV (no figure).
$TheftFigure = 1838502
$NoCommunityAreaFigure = 613720
# Fixed figures of the reference extract (startswith PRD); -1 on another CSV.
$CrimFigure = 1256646        # Primary Type starts with CRIM
$JaFigure = 269655           # Case Number starts with JA
$TheftCrimFigure = 3095148   # Primary Type THEFT or starting with CRIM
$CaseHFigure = 5762199       # Case Number starts with H
$DistrictEightFigure = 0     # District starts with 8: codes are never padded
# Queries shared by several criteria: a month whose first and last minutes both hold records on
# the reference CSV, a day inside it, the spring-forward hour of 2019 and a rare primary type.
$Month = 'Date=2019-08-01..2019-08-31'
$FirstMinute = 'Date=2019-08-01 00:00..2019-08-01 00:00'
$LastMinute = 'Date=2019-08-31 23:59..2019-08-31 23:59'
$Day = 'Date=2019-08-15..2019-08-15'
$SpringForward = 'Date=2019-03-10 02:00..2019-03-10 02:59'
$Everything = 'Date=2000-01-01..'
$RareType = 'RITUALISM'
$MissingId = 123          # below the smallest ID of the CSV (634)
$MissingCase = 'JA000000'
# The end of the name of the query app's temporary output file, `.<target name>.XXXXXX.crime-query.tmp`.
$TemporarySuffix = '.crime-query.tmp'
# Peaks of the same order (AC-C13): the largest is at most this many times the smallest.
$MemoryRatioLimit = 3

$Header = '"ID","Case Number","Date","Block","IUCR","Primary Type","Description","Location Description","Arrest","Domestic","Beat","District","Ward","Community Area","FBI Code","X Coordinate","Y Coordinate","Year","Updated On","Latitude","Longitude","Location"'
$Fields = @($Header.Split(',') | ForEach-Object { $_.Trim('"') })
$Conditionable = @($Fields | Where-Object { @('Block', 'Description', 'Location') -notcontains $_ })

# Exact user-facing texts of the query app (parent and child PRDs, feature decisions).
$Messages = @{
    NoIndexed       = 'Indica al menos una condición sobre un campo indexado: ID, número de caso, fecha, tipo principal, distrito o área comunitaria.'
    MissingId       = "No existe ningún delito con ID $MissingId."
    MissingId2      = "No existe ningún delito con ID $($MissingId + 1)."
    MissingCase     = "No existe ningún delito con número de caso $MissingCase."
    Mandatory       = 'El campo "Primary Type" es obligatorio; no admite "sin valor".'
    NotInteger      = '"abc" no es válido para "Community Area": se esperaba un número entero.'
    RangeNotDate    = 'Solo se admiten rangos sobre "Date".'
    Limit           = 'El límite debe ser un número entero mayor que 0.'
    Cancelled       = 'Consulta cancelada.'
    CountFormat     = '"Solo conteo" no se combina con un formato de salida.'
    OutputFormat    = 'El archivo de salida solo admite los formatos CSV o JSON.'
    UnknownField    = 'El campo "Gravedad" no existe. Campos disponibles: ' + ($Conditionable -join ', ') + '.'
    InvalidDate     = '"2019-13-01" no es una fecha válida. Formato esperado: AAAA-MM-DD [hh:mm].'
    InvertedRange   = 'La fecha inicial es posterior a la final.'
    Schema          = 'El conjunto usa el esquema v1 y este consultor admite v2. Regenera el conjunto.'
    EmptyPrefix     = 'El prefijo de "Primary Type" no puede estar vacío.'
    NotText         = '"Empieza por" solo se admite en campos de texto: Case Number, Primary Type, District, IUCR, Location Description, Beat y FBI Code.'
    BlockCondition  = 'El campo "Block" no admite condiciones. Campos disponibles: ' + ($Conditionable -join ', ') + '.'
    DescCondition   = 'El campo "Description" no admite condiciones. Campos disponibles: ' + ($Conditionable -join ', ') + '.'
    PrefixForm      = 'La condición "Primary Type" no tiene la forma CAMPO=PREFIJO.'
}
function Get-NotFoundMessage([string]$Folder) { "No se encontró un conjunto de datos válido en $Folder." }
function Get-UnreadableMessage([string]$Folder) { "No se pudo leer el conjunto de datos en $Folder." }
function Get-UnwritableMessage([string]$Path) { "No se puede escribir el archivo de salida $Path." }
$StatsPattern = '^(?<count>[0-9.]+) resultados? en [0-9.]+,[0-9]{3} s\. Índices usados: (?<indexes>.*)\. Filtros sin índice: (?<filters>.*)\.$'

# --- CSV helpers (compiled: they read millions of lines) ---

Add-Type -AssemblyName Microsoft.VisualBasic
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class QueryAcceptance {
    static readonly Encoding Utf8 = new UTF8Encoding(false);

    // The first data row and the last data row of a CSV, as text.
    public static string[] FirstAndLastRows(string path) {
        string first, last = null, line;
        using (var reader = new StreamReader(path, Utf8, false, 1 << 20)) {
            reader.ReadLine();
            first = reader.ReadLine();
        }
        using (var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read)) {
            long start = Math.Max(0, stream.Length - 4096);
            stream.Seek(start, SeekOrigin.Begin);
            using (var reader = new StreamReader(stream, Utf8)) {
                while ((line = reader.ReadLine()) != null) { if (line.Length > 0) last = line; }
            }
        }
        return new string[] { first, last };
    }

    // The first case number (second column, compared exactly) that a later row repeats, or null.
    public static string FirstSharedCaseNumber(string path) {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        string line;
        using (var reader = new StreamReader(path, Utf8, false, 1 << 20)) {
            reader.ReadLine();
            while ((line = reader.ReadLine()) != null) {
                int first = line.IndexOf(',');
                int second = first < 0 ? -1 : line.IndexOf(',', first + 1);
                if (second < 0) continue;
                string caseNumber = line.Substring(first + 1, second - first - 1).Trim('"');
                if (!seen.Add(caseNumber)) return caseNumber;
            }
        }
        return null;
    }

    // Writes the header and the data rows skip+1 .. skip+rows of a CSV to target, ending every
    // line with a line feed like the reference CSV. Returns the rows written.
    public static long WriteSlice(string source, string target, long skip, long rows) {
        long written = 0;
        string line;
        using (var reader = new StreamReader(source, Utf8, false, 1 << 20))
        using (var writer = new StreamWriter(target, false, Utf8, 1 << 20)) {
            writer.NewLine = "\n";
            writer.WriteLine(reader.ReadLine());
            for (long i = 0; i < skip && reader.ReadLine() != null; i++) { }
            while (written < rows && (line = reader.ReadLine()) != null) {
                writer.WriteLine(line);
                written++;
            }
        }
        return written;
    }

    // The values of one CSV line, unquoted, joined by the unit separator: two lines that differ only
    // in their quoting give the same text.
    public static string CanonicalRow(string line) {
        var fields = new List<string>();
        var field = new StringBuilder();
        bool quoted = false;
        for (int i = 0; i < line.Length; i++) {
            char c = line[i];
            if (quoted) {
                if (c != '"') field.Append(c);
                else if (i + 1 < line.Length && line[i + 1] == '"') { field.Append('"'); i++; }
                else quoted = false;
            }
            else if (c == '"') quoted = true;
            else if (c == ',') { fields.Add(field.ToString()); field.Clear(); }
            else field.Append(c);
        }
        fields.Add(field.ToString());
        return String.Join("\u001F", fields);
    }

    // Compares two CSV files as sets of rows: the same first line and the same other lines, in
    // any order, byte for byte or, with byValue, field value by field value. Returns null when
    // equal, otherwise the first difference in words.
    public static string CompareRowSets(string left, string right, bool byValue) {
        string[] a = File.ReadAllLines(left, Utf8);
        string[] b = File.ReadAllLines(right, Utf8);
        if (byValue) {
            a = Array.ConvertAll(a, CanonicalRow);
            b = Array.ConvertAll(b, CanonicalRow);
        }
        if (a.Length == 0 || b.Length == 0) return "a file is empty";
        if (a[0] != b[0]) return "the header lines differ";
        if (a.Length != b.Length) return String.Format("{0} rows against {1}", a.Length - 1, b.Length - 1);
        string[] x = new string[a.Length - 1], y = new string[b.Length - 1];
        Array.Copy(a, 1, x, 0, x.Length);
        Array.Copy(b, 1, y, 0, y.Length);
        Array.Sort(x, StringComparer.Ordinal);
        Array.Sort(y, StringComparer.Ordinal);
        for (int i = 0; i < x.Length; i++) {
            if (x[i] != y[i]) return ("first different row: " + x[i] + " / " + y[i]).Replace('\u001F', ',');
        }
        return null;
    }

    // The integers of a file with one per line.
    public static HashSet<long> ReadIds(string path) {
        var ids = new HashSet<long>();
        foreach (string line in File.ReadLines(path, Utf8)) {
            if (line.Length > 0) ids.Add(Int64.Parse(line));
        }
        return ids;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct Counters {
        public uint cb; public uint PageFaultCount;
        public UIntPtr PeakWorkingSetSize; public UIntPtr WorkingSetSize;
        public UIntPtr QuotaPeakPagedPoolUsage; public UIntPtr QuotaPagedPoolUsage;
        public UIntPtr QuotaPeakNonPagedPoolUsage; public UIntPtr QuotaNonPagedPoolUsage;
        public UIntPtr PagefileUsage; public UIntPtr PeakPagefileUsage;
    }

    [DllImport("psapi.dll", SetLastError = true)]
    static extern bool GetProcessMemoryInfo(IntPtr process, out Counters counters, uint size);

    // The peak private committed bytes of a process, also once it has ended (while its handle
    // is open); -1 when Windows does not tell.
    public static long PeakPrivateBytes(IntPtr process) {
        Counters counters;
        if (!GetProcessMemoryInfo(process, out counters, (uint)Marshal.SizeOf(typeof(Counters)))) return -1;
        return (long)counters.PeakPagefileUsage.ToUInt64();
    }
}

// Samples without pause, on a thread of its own, the size of every file of a folder whose name
// matches a pattern, until Stop: it sees what a short run writes to a temporary file that the
// run deletes before it ends. Each size is read through a handle, which is current while the
// writer still holds the file open.
public sealed class TemporaryOutputWatch {
    readonly string dir, pattern;
    readonly Thread thread;
    readonly Dictionary<string, long> largest = new Dictionary<string, long>(StringComparer.Ordinal);
    volatile bool stopping;
    long samples;

    public TemporaryOutputWatch(string dir, string pattern) {
        this.dir = dir;
        this.pattern = pattern;
        thread = new Thread(Run);
        thread.IsBackground = true;
        thread.Start();
    }

    public long Samples { get { return Interlocked.Read(ref samples); } }

    void Run() {
        while (!stopping) { Sample(); Thread.Yield(); }
        Sample();
    }

    void Sample() {
        string[] paths;
        try { paths = Directory.GetFiles(dir, pattern); }
        catch (IOException) { return; }
        foreach (string path in paths) {
            long length;
            try {
                using (var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete)) {
                    length = stream.Length;
                }
            }
            catch (IOException) { continue; }
            catch (UnauthorizedAccessException) { continue; }
            string name = Path.GetFileName(path);
            lock (largest) {
                long known;
                if (!largest.TryGetValue(name, out known) || length > known) largest[name] = length;
            }
        }
        Interlocked.Increment(ref samples);
    }

    // Stops sampling and returns each file seen with the largest size it was seen holding.
    public Dictionary<string, long> Stop() {
        stopping = true;
        thread.Join();
        lock (largest) { return new Dictionary<string, long>(largest, StringComparer.Ordinal); }
    }
}
"@

# --- Results and report ---

$Results = New-Object System.Collections.Generic.List[object]
$Sections = New-Object System.Collections.Generic.List[string]

function Add-Result([string]$Criterion, [bool]$Passed, [string]$Evidence) {
    $outcome = if ($Passed) { 'PASS' } else { 'FAIL' }
    $Results.Add([pscustomobject]@{ Order = $Results.Count; Criterion = $Criterion; Result = $outcome; Evidence = $Evidence })
    Write-Host ('{0}  {1}  {2}' -f $outcome, $Criterion, $Evidence)
}

function Add-Section([string]$Markdown) { $Sections.Add($Markdown) }

function Write-Step([string]$Text) { Write-Host ('[{0:HH:mm:ss}] {1}' -f (Get-Date), $Text) }

function Format-Invariant([string]$Format, [object[]]$Values) { [string]::Format($Invariant, $Format, $Values) }

function Format-Mib([double]$Bytes) { Format-Invariant '{0:N1} MiB' @($Bytes / 1MB) }

function Format-Seconds([double]$Seconds) { Format-Invariant '{0:N1} s' @($Seconds) }

function Format-Millis([double]$Seconds) { Format-Invariant '{0:N0} ms' @($Seconds * 1000) }

function Format-Count([long]$Value) { Format-Invariant '{0:N0}' @($Value) }

function Get-Lines([string]$Text) { @($Text -split "`r?`n" | Where-Object { $_ -ne '' }) }

function Format-Code([string]$Text) { "``````text`n$($Text.TrimEnd())`n``````" }

function Format-Command([string]$Program, [string[]]$Arguments) {
    $quoted = foreach ($argument in $Arguments) { if ($argument -match '[\s"]' -or $argument -eq '') { '"' + $argument + '"' } else { $argument } }
    ('{0} {1}' -f $Program, ($quoted -join ' ')).Replace($WorkDir, '<work>').Replace($Csv, '<csv>')
}

function Write-Utf8([string]$Path, [string]$Text) { [System.IO.File]::WriteAllText($Path, $Text, $Utf8) }

# --- Files ---

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

function Get-CurrentName([string]$Output) {
    (Read-SharedText (Join-Path $Output 'CURRENT')).TrimEnd("`r", "`n")
}

function Get-FileSha([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Copy-Dataset([string]$From, [string]$To) {
    New-Item -ItemType Directory -Path $To | Out-Null
    $current = Get-CurrentName $From
    Copy-Item -LiteralPath (Join-Path $From 'CURRENT') -Destination $To
    Copy-Item -LiteralPath (Join-Path $From $current) -Destination $To -Recurse
    Join-Path $To $current
}

# --- Processes ---

# Starts a program with its output redirected and its standard input closed, so that it can
# never wait for an answer. Without -HoldStdout its standard output is read in the background;
# with it, the caller reads Process.StandardOutput itself.
function Start-Program([string]$FilePath, [string[]]$Arguments, [switch]$HoldStdout) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ($Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $psi.WorkingDirectory = $RepoRoot
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = [System.Diagnostics.Process]::Start($psi)
    $null = $process.Handle  # keeps the exit code and the memory counters readable after the exit
    $process.StandardInput.Close()
    $stdout = if ($HoldStdout) { $null } else { $process.StandardOutput.ReadToEndAsync() }
    [pscustomobject]@{
        Process   = $process
        Stdout    = $stdout
        Stderr    = $process.StandardError.ReadToEndAsync()
        Watch     = $watch
        Arguments = $Arguments
        Started   = Get-Date
    }
}

# Waits for a started program. With -SampleMemory it samples the peak private committed bytes
# every 10 ms and reads them once more after the exit.
function Wait-Program($Started, [switch]$SampleMemory) {
    $process = $Started.Process
    $peak = [long]0
    if ($SampleMemory) {
        while (-not $process.HasExited) {
            $value = [QueryAcceptance]::PeakPrivateBytes($process.Handle)
            if ($value -gt $peak) { $peak = $value }
            Start-Sleep -Milliseconds 10
        }
    }
    $process.WaitForExit()
    $Started.Watch.Stop()
    if ($SampleMemory) {
        $value = [QueryAcceptance]::PeakPrivateBytes($process.Handle)
        if ($value -gt $peak) { $peak = $value }
    }
    [pscustomobject]@{
        ExitCode    = $process.ExitCode
        WallSeconds = $Started.Watch.Elapsed.TotalSeconds
        PeakBytes   = $peak
        Stdout      = if ($null -ne $Started.Stdout) { $Started.Stdout.Result } else { '' }
        Stderr      = $Started.Stderr.Result
        Arguments   = $Started.Arguments
    }
}

function Invoke-Program([string]$FilePath, [string[]]$Arguments, [switch]$SampleMemory) {
    Wait-Program (Start-Program -FilePath $FilePath -Arguments $Arguments) -SampleMemory:$SampleMemory
}

function Invoke-Generator([string]$Source, [string]$Output, [string]$Log, [switch]$Overwrite) {
    $arguments = @('--input', $Source, '--output', $Output, '--temp-dir', $TempDir, '--log', $Log)
    if ($Overwrite) { $arguments += '--overwrite' }
    Invoke-Program -FilePath $GeneratorExe -Arguments $arguments
}

function Invoke-Query([string[]]$Arguments, [string]$Dataset = $DatasetDir, [switch]$SampleMemory) {
    $all = @('--dataset', $Dataset)
    if ($Arguments) { $all += $Arguments }
    Invoke-Program -FilePath $QueryExe -Arguments $all -SampleMemory:$SampleMemory
}

function Format-Query($Run) { Format-Command 'crime-query' ($Run.Arguments | Select-Object -Skip 2) }

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

# The bare number a --count query prints, or -1.
function Get-QueryCount($Run) {
    $text = $Run.Stdout.Trim()
    if ($Run.ExitCode -ne 0 -or $text -notmatch '^[0-9]+$') { return -1 }
    [long]$text
}

# --- The independent side: crime-acceptance select ---

$Selects = [ordered]@{}

function Start-Select([string]$Name, [string[]]$Arguments, [string]$Source = $Csv) {
    $Selects[$Name] = [pscustomobject]@{
        Name    = $Name
        Command = Format-Command 'crime-acceptance select --csv' (@($Source) + $Arguments)
        Started = Start-Program -FilePath $AcceptanceExe -Arguments (@('select', '--csv', $Source) + $Arguments)
        Run     = $null
        Count   = [long]-1
    }
}

function Complete-Selects {
    foreach ($select in $Selects.Values) {
        if ($null -ne $select.Run) { continue }
        $select.Run = Wait-Program $select.Started
        $text = $select.Run.Stdout.Trim()
        if ($select.Run.ExitCode -eq 0 -and $text -match '^[0-9]+$') { $select.Count = [long]$text }
        Write-Step ('select {0}: exit {1}, {2} rows, {3}' -f $select.Name, $select.Run.ExitCode, $select.Count, (Format-Seconds $select.Run.WallSeconds))
    }
}

function Get-SelectCount([string]$Name) { $Selects[$Name].Count }

# --- Output parsing ---

# The records of a CSV text, parsed strictly (every row 22 fields), or $null when it does not parse.
function Read-CsvRecords([string]$Text) {
    $reader = New-Object System.IO.StringReader($Text)
    $parser = New-Object Microsoft.VisualBasic.FileIO.TextFieldParser($reader)
    try {
        $parser.TextFieldType = 'Delimited'
        $parser.SetDelimiters(',')
        $parser.HasFieldsEnclosedInQuotes = $true
        $parser.TrimWhiteSpace = $false
        $names = $parser.ReadFields()
        if ($null -eq $names -or ($names -join '|') -ne ($Fields -join '|')) { return $null }
        $records = New-Object System.Collections.Generic.List[object]
        while (-not $parser.EndOfData) {
            $row = $parser.ReadFields()
            if ($row.Count -ne $Fields.Count) { return $null }
            $records.Add($row)
        }
        , $records.ToArray()
    }
    catch { $null }
    finally { $parser.Close() }
}

# Whether two CSV texts hold the same lines: byte for byte, or field by field with -ByValue (another
# CSV than the reference extract, which may quote its fields differently from the query app). Only
# the empty string after a final line feed is left out; any other empty line still counts.
function Test-SameCsvText([string]$Left, [string]$Right, [bool]$ByValue) {
    if (-not $ByValue) { return $Left -ceq $Right }
    $canonical = foreach ($text in @($Left, $Right)) {
        $lines = @($text -split "`n")
        if ($lines.Count -gt 0 -and $lines[-1] -eq '') { $lines = @($lines | Select-Object -First ($lines.Count - 1)) }
        (@($lines | ForEach-Object { [QueryAcceptance]::CanonicalRow($_) }) -join "`n")
    }
    $canonical[0] -ceq $canonical[1]
}

# The data rows of a table output: the header line, a separator line, then one line per record.
function Read-TableRows([string]$Text) {
    $lines = @(Get-Lines $Text)
    if ($lines.Count -lt 2 -or -not $lines[0].StartsWith('ID ') -or $lines[1] -notmatch '^[- ]+$') { return $null }
    , @($lines | Select-Object -Skip 2)
}

function Test-SameValue([string]$CsvValue, $JsonValue) {
    if ($null -eq $JsonValue) { return $CsvValue -eq '' }
    if ($JsonValue -is [bool]) { return $CsvValue -eq $JsonValue.ToString().ToLowerInvariant() }
    if ($JsonValue -is [string]) { return $CsvValue -ceq $JsonValue }
    $number = 0.0
    if (-not [double]::TryParse($CsvValue, [System.Globalization.NumberStyles]::Float, $Invariant, [ref]$number)) { return $false }
    [math]::Abs($number - [double]$JsonValue) -le 1e-9
}

# Table, CSV and JSON of the same query (AC-C8, AC-S9): valid, the same records, nothing else on stdout.
function Test-Formats([string]$Label, [string[]]$Arguments, [string[]]$ExpectedWarnings, [string]$Criterion = 'AC-C8') {
    $tableRun = Invoke-Query -Arguments $Arguments
    $csvRun = Invoke-Query -Arguments ($Arguments + @('--format', 'csv'))
    $jsonRun = Invoke-Query -Arguments ($Arguments + @('--format', 'json'))
    $problems = New-Object System.Collections.Generic.List[string]
    foreach ($run in @($tableRun, $csvRun, $jsonRun)) {
        if ($run.ExitCode -ne 0) { $problems.Add("exit $($run.ExitCode) for $(Format-Query $run)") }
        $stats = Get-Stats $run.Stderr
        $warnings = @(Get-Lines $run.Stderr | Where-Object { $_ -ne $(if ($stats) { $stats.Line } else { '' }) })
        if (-not $stats) { $problems.Add("no stats line on stderr for $(Format-Query $run)") }
        if (($warnings -join '|') -ne (@($ExpectedWarnings) -join '|')) { $problems.Add("stderr warnings [$($warnings -join '; ')]") }
        foreach ($line in Get-Lines $run.Stdout) {
            if ($line -match $StatsPattern -or $line.StartsWith('No existe') -or $line.StartsWith('Aviso')) { $problems.Add("stdout holds ""$line""") }
        }
    }
    $records = Read-CsvRecords $csvRun.Stdout
    $csvRows = @(Get-Lines $csvRun.Stdout).Count - 1
    $byPowerShell = @(Get-Lines $csvRun.Stdout | ConvertFrom-Csv)
    $document = try { $jsonRun.Stdout | ConvertFrom-Json } catch { $null }
    $tableRows = Read-TableRows $tableRun.Stdout
    if ($null -eq $records) { $problems.Add('the CSV does not parse strictly (22 quoted fields per row, the source header)') }
    if ($null -eq $document) { $problems.Add('the JSON does not parse') }
    if ($null -eq $tableRows) { $problems.Add('the table has no header and separator') }
    $count = -1
    if ($null -ne $records -and $null -ne $document -and $null -ne $tableRows) {
        $count = $records.Count
        $jsonResults = @($document.results)
        if ($csvRows -ne $count -or $byPowerShell.Count -ne $count) { $problems.Add("CSV lines $csvRows, ConvertFrom-Csv $($byPowerShell.Count), strict parser $count") }
        if ($jsonResults.Count -ne $count -or $tableRows.Count -ne $count) { $problems.Add("records: CSV $count, JSON $($jsonResults.Count), table $($tableRows.Count)") }
        $csvIds = @($records | ForEach-Object { $_[0] } | Sort-Object)
        $jsonIds = @($jsonResults | ForEach-Object { [string]$_.ID } | Sort-Object)
        $tableIds = @($tableRows | ForEach-Object { ($_ -split '\s+')[0] } | Sort-Object)
        if (($csvIds -join ',') -ne ($jsonIds -join ',') -or ($csvIds -join ',') -ne ($tableIds -join ',')) { $problems.Add('the ID sets differ') }
        $jsonById = @{}
        foreach ($result in $jsonResults) { $jsonById[[string]$result.ID] = $result }
        $tableById = @{}
        foreach ($row in $tableRows) { $tableById[($row -split '\s+')[0]] = $row }
        foreach ($record in $records) {
            $result = $jsonById[$record[0]]
            $row = $tableById[$record[0]]
            if ($null -eq $result -or $null -eq $row) { continue }
            $position = 0
            for ($i = 0; $i -lt $Fields.Count; $i++) {
                if (-not (Test-SameValue $record[$i] $result.($Fields[$i]))) { $problems.Add("ID $($record[0]) field $($Fields[$i]): CSV ""$($record[$i])"", JSON ""$($result.($Fields[$i]))"""); break }
                if ($record[$i] -eq '') { continue }
                $found = $row.IndexOf($record[$i], $position, [System.StringComparison]::Ordinal)
                if ($found -lt 0) { $problems.Add("ID $($record[0]) field $($Fields[$i]): ""$($record[$i])"" not in its table row"); break }
                $position = $found + $record[$i].Length
            }
        }
    }
    $evidence = '`{0}` as table, CSV and JSON: exit {1}/{2}/{3}; {4} records in each, the same IDs and field values (CSV parsed strictly and by ConvertFrom-Csv, JSON by ConvertFrom-Json, every CSV value found in order in its table row); stdout holds only results, stderr the stats line{5}' -f `
        (Format-Query $tableRun), $tableRun.ExitCode, $csvRun.ExitCode, $jsonRun.ExitCode, $count, $(if ($ExpectedWarnings) { ' and "' + ($ExpectedWarnings -join '", "') + '"' } else { '' })
    if ($problems.Count -gt 0) { $evidence += '; PROBLEMS: ' + (($problems | Select-Object -First 5) -join '; ') }
    Add-Result "$Criterion ($Label)" ($problems.Count -eq 0 -and $count -gt 0) $evidence
}

# A query that must be refused (AC-C10, AC-S11): exactly one stderr line with the message, nothing
# on stdout, and the exit code of decision C6.
#
# With -AfterWriting, the query's output file is -AfterWriting and the query must fail after it
# started writing records: while it runs, its temporary file `.<target name>.XXXXXX.crime-query.tmp`
# must be seen holding more bytes than the header line. A run that fails when it opens the
# dataset never creates that file, and one that fails before its first record writes the header
# line at most. The run gets the properties TemporaryName, TemporaryBytes and WroteRecords.
function Test-Rejection([string]$Row, [string[]]$Arguments, [string]$Message, [int]$ExitCode, [string]$Dataset = $DatasetDir, [string]$AfterWriting, [string]$Criterion = 'AC-C10') {
    $watch = $null
    if ($AfterWriting) {
        $watch = New-Object TemporaryOutputWatch -ArgumentList (Split-Path -Parent $AfterWriting), ('*' + $TemporarySuffix)
    }
    $run = Invoke-Query -Arguments $Arguments -Dataset $Dataset
    $lines = @(Get-Lines $run.Stderr)
    $ok = $run.ExitCode -eq $ExitCode -and $lines.Count -eq 1 -and $lines[0] -ceq $Message -and $run.Stdout -eq ''
    $evidence = '`{0}`: exit {1} (C6: {2}); stderr "{3}"; stdout empty: {4}' -f `
        (Format-Query $run), $run.ExitCode, $ExitCode, ($lines -join ' / '), ($run.Stdout -eq '')
    if ($watch) {
        $seen = $watch.Stop()
        $pattern = '^' + [regex]::Escape('.' + (Split-Path -Leaf $AfterWriting) + '.') + '[A-Za-z0-9]{6}' + [regex]::Escape($TemporarySuffix) + '$'
        $temporary = @($seen.GetEnumerator() | Where-Object { $_.Key -cmatch $pattern } | Sort-Object Value -Descending | Select-Object -First 1)
        $headerBytes = $Utf8.GetByteCount($Header) + 1
        $wrote = $temporary.Count -eq 1 -and $temporary[0].Value -gt $headerBytes
        $ok = $ok -and $wrote
        $evidence += '; while it ran ({0} samples of the target folder), {1}' -f (Format-Count $watch.Samples), $(
            if ($temporary.Count -eq 1) {
                'its temporary file {0} held up to {1} bytes (the header line has {2}): records were being written before the abort: {3}' -f `
                    $temporary[0].Key, (Format-Count $temporary[0].Value), $headerBytes, $wrote
            }
            else { 'no temporary file was seen (FAIL: no proof that writing had started)' })
        $run | Add-Member -NotePropertyName TemporaryName -NotePropertyValue $(if ($temporary.Count -eq 1) { $temporary[0].Key } else { $null })
        $run | Add-Member -NotePropertyName TemporaryBytes -NotePropertyValue $(if ($temporary.Count -eq 1) { $temporary[0].Value } else { 0 })
        $run | Add-Member -NotePropertyName WroteRecords -NotePropertyValue $wrote
    }
    Add-Result "$Criterion $Row" $ok $evidence
    $run
}

# --- Preparation ---

$started = Get-Date
foreach ($path in @($Csv, $CtrlCHelper)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing file: $path" }
}
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
$DatasetDir = Join-Path $WorkDir 'full'
# The folder earlier versions of this script gave the full dataset; it is only cleaned.
$LegacyDatasetDir = Join-Path $WorkDir 'reference'
$SmallDir = Join-Path $WorkDir 'small'
$Schema1Dir = Join-Path $WorkDir 'schema1'
$DamagedDir = Join-Path $WorkDir 'damaged'
$SwapDir = Join-Path $WorkDir 'swap'
$EmptyDir = Join-Path $WorkDir 'no-dataset'
$IncompleteDir = Join-Path $WorkDir 'incomplete'
$TempDir = Join-Path $WorkDir 'temp'
$LogDir = Join-Path $WorkDir 'logs'
$InputDir = Join-Path $WorkDir 'inputs'
$OutDir = Join-Path $WorkDir 'outputs'
$ReportPath = Join-Path $WorkDir 'query-acceptance-report.md'
Write-Step "Cleaning the entries of a previous query acceptance in $WorkDir"
foreach ($path in @($DatasetDir, $LegacyDatasetDir, $SmallDir, $Schema1Dir, $DamagedDir, $SwapDir, $EmptyDir, $IncompleteDir, $TempDir, $LogDir, $InputDir, $OutDir, $ReportPath)) {
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Recurse -Force }
}
foreach ($dir in @($TempDir, $LogDir, $InputDir, $OutDir, $EmptyDir)) { New-Item -ItemType Directory -Path $dir | Out-Null }

$freeBytes = (Get-Item -LiteralPath $WorkDir).PSDrive.Free
if ($freeBytes -lt 5GB) { throw ('Only {0} free in {1}; about 5 GB are needed.' -f (Format-Mib $freeBytes), $WorkDir) }

if (-not $SkipBuild) {
    Write-Step 'Building crime-generator, crime-query and crime-acceptance in release mode'
    $cargo = (Get-Command cargo).Source
    $build = Invoke-Program -FilePath $cargo -Arguments @('build', '--release', '-p', 'crime-generator', '-p', 'crime-query', '-p', 'crime-acceptance')
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

# The full dataset, schema 2.
Write-Step "Generating the full dataset into $DatasetDir"
$generation = Invoke-Generator -Source $Csv -Output $DatasetDir -Log (Join-Path $LogDir 'full.log')
if ($generation.ExitCode -ne 0) { throw "The generator failed (exit $($generation.ExitCode)):`n$($generation.Stderr)" }
$current = Get-CurrentName $DatasetDir
$manifest = Get-Content -LiteralPath (Join-Path (Join-Path $DatasetDir $current) 'manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
Write-Step ('Full dataset {0}: schema {1}, {2} records, {3}' -f $current, $manifest.schema_version, (Format-Count $manifest.records.valid), (Format-Seconds $generation.WallSeconds))
$SourceSha256 = [string]$manifest.source.sha256
$IsReference = Test-ReferenceExtract $SourceSha256
Write-Step (Format-CsvMode $SourceSha256)
if (-not (Test-Sha256 $SourceSha256)) {
    Add-Result 'Full dataset: source SHA-256' $false ('the manifest records no valid source SHA-256: "{0}"' -f $SourceSha256)
}
if (-not $IsReference) {
    $TheftFigure = -1
    $NoCommunityAreaFigure = -1
    $CrimFigure = -1
    $JaFigure = -1
    $TheftCrimFigure = -1
    $CaseHFigure = -1
    $DistrictEightFigure = -1
}
# The query app's CSV rows against the source rows: byte for byte on the reference extract, which
# quotes every field like the query app does; field by field on another CSV, which may quote less
# (the portal's downloads quote only the fields that need it).
$ByValue = -not $IsReference
$RowMatchText = if ($ByValue) { 'field by field' } else { 'byte for byte' }
# Every CSV must give 0 rejected records (PRD aceptacion-cualquier-csv, flow step 4); on another CSV
# the verifier runs without --reference, which is what required it there.
$rejected = [long]$manifest.records.rejected
Add-Result 'Full dataset: 0 rejected' ($rejected -eq 0) ('manifest of {0}: {1} valid, {2} rejected' -f `
        $current, (Format-Count $manifest.records.valid), (Format-Count $rejected))

# The slices of the CSV behind the small datasets.
Write-Step "Writing the CSV slices ($SliceRows rows each) and the small CSV"
$Slice1 = Join-Path $InputDir 'slice-1.csv'
$Slice2 = Join-Path $InputDir 'slice-2.csv'
$SmallCsv = Join-Path $InputDir 'small.csv'
$slice1Count = [QueryAcceptance]::WriteSlice($Csv, $Slice1, 0, $SliceRows)
$slice2Count = [QueryAcceptance]::WriteSlice($Csv, $Slice2, $SliceRows, $SliceRows)
$smallCount = [QueryAcceptance]::WriteSlice($Csv, $SmallCsv, 0, 20000)
foreach ($pair in @(@($SmallCsv, $SmallDir, 'small'), @($Slice1, $SwapDir, 'swap-v1'))) {
    $run = Invoke-Generator -Source $pair[0] -Output $pair[1] -Log (Join-Path $LogDir "$($pair[2]).log")
    if ($run.ExitCode -ne 0) { throw "The generator failed on $($pair[0]):`n$($run.Stderr)" }
}

# A schema-1 copy and a damaged copy of the small dataset, and an incomplete folder whose CURRENT
# names a version folder that is not there.
$schema1Version = Copy-Dataset $SmallDir $Schema1Dir
$schema1Manifest = Join-Path $schema1Version 'manifest.json'
$manifestText = [System.IO.File]::ReadAllText($schema1Manifest, $Utf8)
$editedManifest = [regex]::Replace($manifestText, '"schema_version":\s*2', '"schema_version": 1')
if ($editedManifest -eq $manifestText) { throw 'The manifest has no "schema_version": 2 to edit.' }
Write-Utf8 $schema1Manifest $editedManifest
$damagedVersion = Copy-Dataset $SmallDir $DamagedDir
$damagedPrimary = Join-Path $damagedVersion 'primary.sst'
$primaryLength = (Get-Item -LiteralPath $damagedPrimary).Length
$damageOffset = [long][math]::Floor($primaryLength / 2)
$stream = [System.IO.File]::Open($damagedPrimary, 'Open', 'ReadWrite')
try {
    $stream.Seek($damageOffset, 'Begin') | Out-Null
    $junk = New-Object byte[] 4096
    for ($i = 0; $i -lt $junk.Length; $i++) { $junk[$i] = 0xA5 }
    $stream.Write($junk, 0, $junk.Length)
}
finally { $stream.Dispose() }
New-Item -ItemType Directory -Path $IncompleteDir | Out-Null
Write-Utf8 (Join-Path $IncompleteDir 'CURRENT') "dataset-20260101-000000-1`n"

# Rows the criteria need, found on the CSV without the query app.
$edgeRows = [QueryAcceptance]::FirstAndLastRows($Csv)
$firstId = [long]($edgeRows[0].Split(',')[0].Trim('"'))
$lastId = [long]($edgeRows[1].Split(',')[0].Trim('"'))
Write-Step 'Looking for a shared case number on the CSV'
$sharedCase = [QueryAcceptance]::FirstSharedCaseNumber($Csv)
if (-not $sharedCase) { throw 'The CSV has no shared case number.' }
Write-Step "First case number repeated in the CSV: $sharedCase"

# --- AC-C7: an existing ID in less than 1 second (quiet machine: before the selects start) ---

Write-Step 'AC-C7: an existing ID, five runs'
$idRuns = @(1..5 | ForEach-Object { Invoke-Query -Arguments @('--where', "ID=$lastId") })
$idOk = @($idRuns | Where-Object { $_.ExitCode -ne 0 -or $_.WallSeconds -ge 1.0 -or @(Get-Lines $_.Stdout).Count -ne 3 }).Count -eq 0
Add-Result 'AC-C7' $idOk ('`crime-query --where ID={0}` (table), whole process wall time per run, the first right after the generation: {1}; each run exit 0 with one record; limit 1 s' -f `
        $lastId, (($idRuns | ForEach-Object { Format-Millis $_.WallSeconds }) -join ', '))

# --- AC-C13: memory does not grow with the result ---

Write-Step 'AC-C13: peak private memory of three counts and of two record listings'
$memoryRuns = [ordered]@{
    'count THEFT'                         = Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--count') -SampleMemory
    "count $RareType"                     = Invoke-Query -Arguments @('--where', "Primary Type=$RareType", '--count') -SampleMemory
    'count THEFT, 2005-2015'              = Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--range', 'Date=2005-01-01..2015-12-31', '--count') -SampleMemory
}
$listingRuns = [ordered]@{
    'CSV file of THEFT'                   = Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--format', 'csv', '--output', (Join-Path $OutDir 'memory-theft.csv')) -SampleMemory
    "CSV file of $RareType"               = Invoke-Query -Arguments @('--where', "Primary Type=$RareType", '--format', 'csv', '--output', (Join-Path $OutDir 'memory-rare.csv')) -SampleMemory
}
$memoryRows = New-Object System.Collections.Generic.List[string]
function Test-MemoryGroup([string]$Label, $Runs, [string]$Criterion = 'AC-C13') {
    $peaks = @($Runs.Values | ForEach-Object { $_.PeakBytes })
    $min = ($peaks | Measure-Object -Minimum).Minimum
    $max = ($peaks | Measure-Object -Maximum).Maximum
    $allOk = @($Runs.Values | Where-Object { $_.ExitCode -ne 0 -or $_.PeakBytes -le 0 }).Count -eq 0
    $parts = foreach ($name in $Runs.Keys) {
        $run = $Runs[$name]
        $stats = Get-Stats $run.Stderr
        $resultCount = if ($stats) { $stats.Count } else { -1 }
        $memoryRows.Add(('| {0} | {1} | {2} | {3} ({4} bytes) | {5} |' -f $name, $run.ExitCode, (Format-Count $resultCount), (Format-Mib $run.PeakBytes), (Format-Count $run.PeakBytes), (Format-Seconds $run.WallSeconds)))
        '{0}: {1} results, {2}' -f $name, (Format-Count $resultCount), (Format-Mib $run.PeakBytes)
    }
    $ratio = if ($min -gt 0) { $max / $min } else { [double]::PositiveInfinity }
    Add-Result "$Criterion ($Label)" ($allOk -and $ratio -le $MemoryRatioLimit) ('{0}; largest/smallest peak = {1} (rule: at most {2}, same order whatever the number of results; the query app shares a block cache bounded to 2,048 blocks, about 8 MiB)' -f `
            ($parts -join '; '), (Format-Invariant '{0:N2}' @($ratio)), $MemoryRatioLimit)
}
Test-MemoryGroup 'counts' $memoryRuns
Test-MemoryGroup 'records written' $listingRuns
# AC-S8: a prefix that spans 5.761.892 case numbers against one that spans a single one.
$prefixMemoryRuns = [ordered]@{
    'count Case Number starts with H'        = Invoke-Query -Arguments @('--startswith', 'Case Number=H', '--count') -SampleMemory
    'count Case Number starts with JK438266' = Invoke-Query -Arguments @('--startswith', 'Case Number=JK438266', '--count') -SampleMemory
}
Test-MemoryGroup 'Case Number prefixes' $prefixMemoryRuns -Criterion 'AC-S8'
Add-Section (@(
        '## AC-C13 and AC-S8: peak private memory'
        ''
        'Peak private committed bytes (PeakPagefileUsage, the counter behind PeakPagedMemorySize64), sampled every 10 ms and read again after the exit.'
        ''
        '| Query | Exit | Results | Peak private memory | Wall time |'
        '|---|---|---|---|---|'
        $memoryRows
    ) -join "`n")

# --- The independent counts (in the background while the query-only checks run) ---

Write-Step 'Starting the independent selects on the CSV and the verifier'
$NoCoordIds = Join-Path $OutDir 'select-no-coordinates.txt'
$NoWardIds = Join-Path $OutDir 'select-no-ward.txt'
$DstIds = Join-Path $OutDir 'select-spring-forward.txt'
$TheftIds = Join-Path $OutDir 'select-theft.txt'
$CaseRows = Join-Path $OutDir 'select-case.csv'
$DayRows = Join-Path $OutDir 'select-day.csv'
$Slice1Rows = Join-Path $OutDir 'select-slice-1.csv'
$Slice2Rows = Join-Path $OutDir 'select-slice-2.csv'
Start-Select 'no-coordinates' @('--where', 'X Coordinate=', '--ids-out', $NoCoordIds)
Start-Select 'no-ward' @('--where', 'Ward=', '--ids-out', $NoWardIds)
Start-Select 'spring-forward' @('--range', $SpringForward, '--ids-out', $DstIds)
Start-Select 'theft' @('--where', 'Primary Type=THEFT', '--ids-out', $TheftIds)
Start-Select 'no-community-area' @('--where', 'Community Area=')
Start-Select 'month' @('--range', $Month)
Start-Select 'first-minute' @('--range', $FirstMinute)
Start-Select 'last-minute' @('--range', $LastMinute)
Start-Select 'theft-008' @('--where', 'Primary Type=THEFT', '--where', 'District=008')
Start-Select 'case' @('--where', "Case Number=$sharedCase", '--rows-out', $CaseRows)
Start-Select 'theft-month' @('--where', 'Primary Type=THEFT', '--range', $Month)
Start-Select 'theft-month-arrest' @('--where', 'Primary Type=THEFT', '--range', $Month, '--where', 'Arrest=true')
Start-Select 'theft-month-street' @('--where', 'Primary Type=THEFT', '--range', $Month, '--where', 'Location Description=STREET')
Start-Select 'theft-day' @('--where', 'Primary Type=THEFT', '--range', $Day, '--rows-out', $DayRows)
Start-Select 'slice-1' @('--range', $Everything, '--rows-out', $Slice1Rows) $Slice1
Start-Select 'slice-2' @('--range', $Everything, '--rows-out', $Slice2Rows) $Slice2
# The prefix conditions (startswith PRD).
$CrimIds = Join-Path $OutDir 'select-prefix-crim.txt'
$CrimDayRows = Join-Path $OutDir 'select-prefix-crim-day.csv'
Start-Select 'prefix-crim' @('--startswith', 'Primary Type=CRIM', '--ids-out', $CrimIds)
Start-Select 'prefix-ja' @('--startswith', 'Case Number=JA')
Start-Select 'theft-prefix-01' @('--where', 'Primary Type=THEFT', '--startswith', 'District=01')
Start-Select 'prefix-resid-month' @('--startswith', 'Location Description=resid', '--range', $Month)
Start-Select 'theft-or-prefix-crim' @('--where', 'Primary Type=THEFT', '--startswith', 'Primary Type=CRIM')
Start-Select 'prefix-district-8' @('--startswith', 'District=8')
Start-Select 'prefix-district-00' @('--startswith', 'District=00')
Start-Select 'prefix-case-h' @('--startswith', 'Case Number=H')
Start-Select 'prefix-crim-day' @('--startswith', 'Primary Type=CRIM', '--range', $Day, '--rows-out', $CrimDayRows)
$verifierArguments = @('--csv', $Csv, '--dataset', $DatasetDir)
if ($IsReference) { $verifierArguments += '--reference' }
$verifierStarted = Start-Program -FilePath $AcceptanceExe -Arguments $verifierArguments

# --- AC-C10: every validation of the parent and child query tables ---

Write-Step 'AC-C10: validations'
# Parent table.
Test-Rejection 'parent: folder without a dataset' @('--where', 'ID=634') (Get-NotFoundMessage $EmptyDir) 1 $EmptyDir | Out-Null
Test-Rejection 'parent: incomplete folder (CURRENT names a missing version)' @('--where', 'ID=634') (Get-NotFoundMessage $IncompleteDir) 1 $IncompleteDir | Out-Null
$schemaRun = Test-Rejection 'parent: incompatible schema version' @('--where', 'ID=634') $Messages.Schema 1 $Schema1Dir
$missing = Invoke-Query -Arguments @('--where', "ID=$MissingId", '--format', 'csv')
$missingLines = @(Get-Lines $missing.Stderr)
$missingStats = Get-Stats $missing.Stderr
Add-Result 'AC-C10 parent: nonexistent ID (empty result, not an error)' ($missing.ExitCode -eq 0 -and $missingLines[0] -ceq $Messages.MissingId -and $missingStats -and $missingStats.Count -eq 0 -and $missing.Stdout.TrimEnd() -ceq $Header) `
('`{0}`: exit {1}; stderr "{2}"; stdout holds only the CSV header: {3}' -f (Format-Query $missing), $missing.ExitCode, ($missingLines -join ' / '), ($missing.Stdout.TrimEnd() -ceq $Header))
Test-Rejection 'parent: unknown field' @('--where', 'Gravedad=alta') $Messages.UnknownField 1 | Out-Null
Test-Rejection 'parent: invalid date' @('--range', 'Date=2019-13-01..') $Messages.InvalidDate 1 | Out-Null
Test-Rejection 'parent: inverted date range' @('--range', 'Date=2019-08-31..2019-08-01') $Messages.InvertedRange 1 | Out-Null
# Child table (the parent's "campo sin índice" row is replaced by C10's first row).
Test-Rejection 'child: no condition' @() $Messages.NoIndexed 1 | Out-Null
$arrestAlone = Test-Rejection 'child: only a condition without an index (Arrest)' @('--where', 'Arrest=true', '--count') $Messages.NoIndexed 1
$severalIds = Invoke-Query -Arguments @('--where', "ID=$firstId", '--where', "ID=$MissingId", '--where', "ID=$($MissingId + 1)", '--format', 'csv')
$severalLines = @(Get-Lines $severalIds.Stderr)
$severalRecords = Read-CsvRecords $severalIds.Stdout
$severalOk = $severalIds.ExitCode -eq 0 -and $severalLines.Count -eq 3 -and $severalLines[0] -ceq $Messages.MissingId -and $severalLines[1] -ceq $Messages.MissingId2 `
    -and $null -ne $severalRecords -and $severalRecords.Count -eq 1 -and $severalRecords[0][0] -eq [string]$firstId
Add-Result 'AC-C10 child: several IDs, some missing' $severalOk ('`{0}`: exit {1}; stderr "{2}"; the CSV holds ID {3} only: {4}' -f `
        (Format-Query $severalIds), $severalIds.ExitCode, ($severalLines -join ' / '), $firstId, ($null -ne $severalRecords -and $severalRecords.Count -eq 1))
$missingCaseRun = Invoke-Query -Arguments @('--where', "Case Number=$MissingCase", '--count')
$missingCaseLines = @(Get-Lines $missingCaseRun.Stderr)
Add-Result 'AC-C10 child: nonexistent case number (empty result, not an error)' ($missingCaseRun.ExitCode -eq 0 -and $missingCaseLines[0] -ceq $Messages.MissingCase -and (Get-QueryCount $missingCaseRun) -eq 0) `
('`{0}`: exit {1}; stdout "{2}"; stderr "{3}"' -f (Format-Query $missingCaseRun), $missingCaseRun.ExitCode, $missingCaseRun.Stdout.Trim(), ($missingCaseLines -join ' / '))
Test-Rejection 'child: "sin valor" on a mandatory field' @('--where', 'Primary Type=') $Messages.Mandatory 1 | Out-Null
Test-Rejection 'child: value of the wrong type' @('--where', 'Community Area=abc') $Messages.NotInteger 1 | Out-Null
Test-Rejection 'child: range on a field that is not a date' @('--range', 'ID=1..5') $Messages.RangeNotDate 1 | Out-Null
Test-Rejection 'child: limit 0' @('--where', 'ID=634', '--limit', '0') $Messages.Limit 2 | Out-Null
Test-Rejection 'child: negative limit' @('--where', 'ID=634', '--limit', '-5') $Messages.Limit 2 | Out-Null
Test-Rejection 'child: count with an output format' @('--where', 'ID=634', '--count', '--format', 'csv') $Messages.CountFormat 2 | Out-Null
Test-Rejection 'child: output file with the table format' @('--where', 'ID=634', '--output', (Join-Path $OutDir 'never-table.txt')) $Messages.OutputFormat 2 | Out-Null
Test-Rejection 'child: output file with count only' @('--where', 'ID=634', '--count', '--output', (Join-Path $OutDir 'never-count.txt')) $Messages.OutputFormat 2 | Out-Null
$unwritable = Join-Path $OutDir 'no-such-folder\result.csv'
Test-Rejection 'child: output file that cannot be created' @('--where', 'ID=634', '--format', 'csv', '--output', $unwritable) (Get-UnwritableMessage $unwritable) 1 | Out-Null
$folderTarget = Join-Path $OutDir 'a-folder'
New-Item -ItemType Directory -Path $folderTarget | Out-Null
Test-Rejection 'child: output file that is a folder' @('--where', 'ID=634', '--format', 'json', '--output', $folderTarget) (Get-UnwritableMessage $folderTarget) 1 | Out-Null

# The active version unreadable in the middle of the query, with an output file (AC-C10 and
# AC-C15): the query starts writing, reaches the damaged block and aborts. Both runs must show
# that writing had started: the output file run through its temporary file (see Test-Rejection),
# the stdout run through the header and at least one record on stdout before the abort.
$damagedOut = Join-Path $OutDir 'damaged'
New-Item -ItemType Directory -Path $damagedOut | Out-Null
$damagedTarget = Join-Path $damagedOut 'result.csv'
$damaged = Test-Rejection 'child: the version becomes unreadable mid-query (output file)' @('--range', $Everything, '--format', 'csv', '--output', $damagedTarget) (Get-UnreadableMessage $DamagedDir) 1 $DamagedDir -AfterWriting $damagedTarget
$damagedEntries = @(Get-EntryNames $damagedOut)
$damagedStdout = Invoke-Query -Arguments @('--range', $Everything, '--format', 'csv') -Dataset $DamagedDir
$damagedStdoutLines = @(Get-Lines $damagedStdout.Stderr)
$damagedStdoutRows = @(Get-Lines $damagedStdout.Stdout)
$damagedStdoutWrote = $damagedStdoutRows.Count -ge 2 -and $damagedStdoutRows[0] -ceq $Header
Add-Result 'AC-C10 child: the version becomes unreadable mid-query (stdout)' ($damagedStdout.ExitCode -eq 1 -and $damagedStdoutLines.Count -eq 1 -and $damagedStdoutLines[0] -ceq (Get-UnreadableMessage $DamagedDir) -and -not (Get-Stats $damagedStdout.Stderr) -and $damagedStdoutWrote) `
('`{0}` on a copy of the small dataset with 4 KiB overwritten at byte {1} of primary.sst ({2} bytes): exit {3}; stderr "{4}"; {5} CSV lines reached stdout before the abort, the header first and then records: {6}; no stats line' -f `
        (Format-Query $damagedStdout), (Format-Count $damageOffset), (Format-Count $primaryLength), $damagedStdout.ExitCode, ($damagedStdoutLines -join ' / '), $damagedStdoutRows.Count, $damagedStdoutWrote)

# --- AC-C18 (second half): a schema-1 dataset is rejected ---

Add-Result 'AC-C18 (schema 1)' ($schemaRun.ExitCode -eq 1 -and @(Get-Lines $schemaRun.Stderr)[0] -ceq $Messages.Schema) `
('a copy of a generated dataset with "schema_version": 1 in its manifest: exit {0}; stderr "{1}"' -f $schemaRun.ExitCode, $schemaRun.Stderr.Trim())

# --- AC-C15: a rejected or failing query leaves no file with the requested name ---

Write-Step 'AC-C15: rejected and failing queries with an output file'
$rejectedOut = Join-Path $OutDir 'rejected'
New-Item -ItemType Directory -Path $rejectedOut | Out-Null
$rejectedTarget = Join-Path $rejectedOut 'result.csv'
$rejectedRun = Invoke-Query -Arguments @('--where', 'Gravedad=alta', '--format', 'csv', '--output', $rejectedTarget)
$rejectedUsage = Invoke-Query -Arguments @('--where', 'ID=634', '--format', 'json', '--limit', '0', '--output', $rejectedTarget)
$rejectedNone = @(Get-EntryNames $rejectedOut).Count -eq 0
$previousTarget = Join-Path $rejectedOut 'previous.csv'
Write-Utf8 $previousTarget "previous content`n"
$previousHash = Get-FileSha $previousTarget
$rejectedOver = Invoke-Query -Arguments @('--range', 'Date=2019-08-31..2019-08-01', '--format', 'csv', '--output', $previousTarget)
$failedOver = Invoke-Query -Dataset $DamagedDir -Arguments @('--range', $Everything, '--format', 'csv', '--output', $previousTarget)
$previousKept = (Get-FileSha $previousTarget) -eq $previousHash -and @(Get-EntryNames $rejectedOut).Count -eq 1
Add-Result 'AC-C15 (rejected)' ($rejectedRun.ExitCode -eq 1 -and $rejectedUsage.ExitCode -eq 2 -and $rejectedNone -and $rejectedOver.ExitCode -eq 1 -and $previousKept) `
('unknown field with --output: exit {0}; limit 0 with --output: exit {1}; no file in the target folder afterwards: {2}. Over an existing file: an inverted range exits {3} and the damaged dataset exits {4}; the file keeps its SHA-256 and the folder holds only it: {5}' -f `
        $rejectedRun.ExitCode, $rejectedUsage.ExitCode, $rejectedNone, $rejectedOver.ExitCode, $failedOver.ExitCode, $previousKept)
Add-Result 'AC-C15 (failing)' ($damaged.ExitCode -eq 1 -and $damaged.WroteRecords -and $damagedEntries.Count -eq 0 -and $failedOver.ExitCode -eq 1) `
('the query over the damaged dataset failed after it started writing: exit {0}, stderr "{1}"; while it ran, its temporary file {2} held up to {3} bytes (more than the header line: {4}); the target folder holds afterwards: {5}' -f `
        $damaged.ExitCode, $damaged.Stderr.Trim(), $(if ($damaged.TemporaryName) { $damaged.TemporaryName } else { '(none seen)' }), (Format-Count $damaged.TemporaryBytes), $damaged.WroteRecords, `
        $(if ($damagedEntries.Count -eq 0) { 'nothing (no file with the requested name, no temporary file)' } else { $damagedEntries -join ', ' }))

# --- AC-C10: a real Ctrl+C on a long query with an output file ---

Write-Step 'AC-C10: Ctrl+C once the output file holds 1 MiB'
$ctrlCOut = Join-Path $OutDir 'ctrl-c'
New-Item -ItemType Directory -Path $ctrlCOut | Out-Null
$ctrlCTarget = Join-Path $ctrlCOut 'cancelled.csv'
$requestFile = Join-Path $InputDir 'ctrl-c-request.json'
$resultFile = Join-Path $InputDir 'ctrl-c-result.json'
$ctrlCStderr = Join-Path $InputDir 'ctrl-c-stderr.txt'
$ctrlCStdout = Join-Path $InputDir 'ctrl-c-stdout.txt'
$ctrlCArguments = @('--dataset', $DatasetDir, '--range', $Everything, '--format', 'csv', '--output', $ctrlCTarget)
[pscustomobject]@{
    Exe             = $QueryExe
    Arguments       = $ctrlCArguments
    TriggerDir      = $ctrlCOut
    TriggerMinBytes = 1MB
    TimeoutSeconds  = $CtrlCTimeoutSeconds
    StdoutFile      = $ctrlCStdout
    StderrFile      = $ctrlCStderr
    ResultFile      = $resultFile
} | ConvertTo-Json | Set-Content -LiteralPath $requestFile -Encoding UTF8
# A console of its own, so the Ctrl+C reaches the query app and nothing else.
$helperArguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}" -RequestFile "{1}"' -f $CtrlCHelper, $requestFile
Start-Process -FilePath 'powershell.exe' -ArgumentList $helperArguments -WindowStyle Hidden -Wait
$ctrlC = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
$ctrlCErrText = [System.IO.File]::ReadAllText($ctrlCStderr, $Utf8)
$ctrlCErrLines = @(Get-Lines $ctrlCErrText)
$ctrlCEntries = @(Get-EntryNames $ctrlCOut)
$ctrlCOk = $ctrlC.RunningWhenSent -and $ctrlC.Sent -and $null -ne $ctrlC.Observed -and $ctrlC.ExitCode -eq 130 `
    -and $ctrlCErrLines.Count -eq 1 -and $ctrlCErrLines[0] -ceq $Messages.Cancelled -and $ctrlCEntries.Count -eq 0 `
    -and [System.IO.File]::ReadAllText($ctrlCStdout, $Utf8) -eq ''
Add-Result 'AC-C10 child: Ctrl+C (output file)' $ctrlCOk ('`{0}` in a console of its own; Ctrl+C sent at {1} s once {2}, while running: {3}; exit {4} after {5} s; stderr "{6}"; the target folder holds afterwards: {7}' -f `
        (Format-Command 'crime-query' ($ctrlCArguments | Select-Object -Skip 2)), $ctrlC.SentAtSeconds, $(if ($ctrlC.Observed) { $ctrlC.Observed } else { 'the timeout passed (FAIL)' }), `
        $ctrlC.RunningWhenSent, $ctrlC.ExitCode, $ctrlC.SecondsToExit, ($ctrlCErrLines -join ' / '), $(if ($ctrlCEntries.Count -eq 0) { 'nothing' } else { $ctrlCEntries -join ', ' }))

# --- The selects are needed from here on ---

Write-Step 'Waiting for the selects'
Complete-Selects
$sampleIds = @(
    [pscustomobject]@{ Label = 'first CSV row'; Id = $firstId }
    [pscustomobject]@{ Label = 'last CSV row'; Id = $lastId }
    [pscustomobject]@{ Label = 'without coordinates'; Id = [long](@(Get-Content -LiteralPath $NoCoordIds -TotalCount 1)[0]) }
    [pscustomobject]@{ Label = 'without Ward'; Id = [long](@(Get-Content -LiteralPath $NoWardIds -TotalCount 1)[0]) }
    [pscustomobject]@{ Label = 'spring-forward hour'; Id = [long](@(Get-Content -LiteralPath $DstIds -TotalCount 1)[0]) }
)
$SampleRows = Join-Path $OutDir 'select-sample.csv'
Start-Select 'sample' (@($sampleIds | ForEach-Object { '--where'; "ID=$($_.Id)" }) + @('--rows-out', $SampleRows))

# --- AC-C2, AC-C3: counts ---

function Test-Count([string]$Criterion, [string[]]$Arguments, [string]$Select, [long]$Fixed = -1, [string]$Extra = '') {
    $run = Invoke-Query -Arguments ($Arguments + @('--count'))
    $count = Get-QueryCount $run
    $expected = Get-SelectCount $Select
    $ok = $count -ge 0 -and $count -eq $expected -and ($Fixed -lt 0 -or $count -eq $Fixed)
    $fixedText = if ($Fixed -ge 0) { '; PRD figure {0}' -f (Format-Count $Fixed) } else { '' }
    $stats = Get-Stats $run.Stderr
    Add-Result $Criterion $ok ('`{0}`: {1}; select on the CSV: {2}{3}; stats "{4}"{5}' -f `
            (Format-Query $run), (Format-Count $count), (Format-Count $expected), $fixedText, $(if ($stats) { $stats.Line } else { 'missing' }), $Extra)
    [pscustomobject]@{ Run = $run; Count = $count; Stats = $stats; Ok = $ok }
}

Write-Step 'Counts'
Test-Count 'AC-C2' @('--where', 'Primary Type=THEFT') 'theft' $TheftFigure | Out-Null
Test-Count 'AC-C3' @('--where', 'Community Area=') 'no-community-area' $NoCommunityAreaFigure | Out-Null

# --- AC-C4: a month, with its first and last instants ---

Test-Count 'AC-C4 (month)' @('--range', $Month) 'month' | Out-Null
$first = Test-Count 'AC-C4 (first minute)' @('--range', $FirstMinute) 'first-minute' -Extra '; the first minute of the month holds records'
$last = Test-Count 'AC-C4 (last minute)' @('--range', $LastMinute) 'last-minute' -Extra '; the last minute of the month (up to 23:59:59) holds records'
if ($first.Count -le 0 -or $last.Count -le 0) { Add-Result 'AC-C4 (edges hold records)' $false 'the first or the last minute of the month holds no record, so the edges are not proven' }

# --- AC-C5: two indexed conditions ---

$c5 = Test-Count 'AC-C5' @('--where', 'Primary Type=THEFT', '--where', 'District=008') 'theft-008'
Add-Result 'AC-C5 (indexes reported)' ($null -ne $c5.Stats -and $c5.Stats.Indexes -eq 'Primary Type, District') ('stats line reports the indexes used: "{0}"' -f $(if ($c5.Stats) { $c5.Stats.Indexes } else { 'missing' }))

# --- AC-C6: a shared case number returns every record ---

$caseRun = Invoke-Query -Arguments @('--where', "Case Number=$sharedCase", '--format', 'csv')
$caseFile = Join-Path $OutDir 'query-case.csv'
Write-Utf8 $caseFile $caseRun.Stdout
$caseDiff = if ($caseRun.ExitCode -eq 0) { [QueryAcceptance]::CompareRowSets($caseFile, $CaseRows, $ByValue) } else { 'the query failed' }
$caseCount = @(Get-Lines $caseRun.Stdout).Count - 1
Add-Result 'AC-C6' ($null -eq $caseDiff -and $caseCount -ge 2 -and (Get-SelectCount 'case') -eq $caseCount) `
('case number {0}, the first one the CSV repeats (found by a scan of the CSV''s second column): `{1}` returns {2} records; select returns {3}; the same rows {5}, in any order: {4}' -f `
        $sharedCase, (Format-Query $caseRun), $caseCount, (Get-SelectCount 'case'), $(if ($null -eq $caseDiff) { 'yes' } else { $caseDiff }), $RowMatchText)

# --- AC-C9: a filter without an index on top of two indexed conditions ---

$c9 = Test-Count 'AC-C9' @('--where', 'Arrest=true', '--where', 'Primary Type=THEFT', '--range', $Month) 'theft-month-arrest'
$c9Si = Test-Count 'AC-C9 (Arrest=sí)' @('--where', 'Arrest=sí', '--where', 'Primary Type=THEFT', '--range', $Month) 'theft-month-arrest'
Add-Result 'AC-C9 (indexes and filter reported)' ($null -ne $c9.Stats -and $c9.Stats.Indexes -eq 'Date, Primary Type' -and $c9.Stats.Filters -eq 'Arrest') `
('stats line: indexes "{0}", filters without an index "{1}"' -f $(if ($c9.Stats) { $c9.Stats.Indexes } else { '-' }), $(if ($c9.Stats) { $c9.Stats.Filters } else { '-' }))
Add-Result 'AC-C9 (filter alone rejected)' ($arrestAlone.ExitCode -eq 1 -and $arrestAlone.Stderr.Trim() -ceq $Messages.NoIndexed) `
('`{0}`: exit {1}; stderr "{2}"' -f (Format-Query $arrestAlone), $arrestAlone.ExitCode, $arrestAlone.Stderr.Trim())

# --- AC-C11: a limit returns exactly N records of the result ---

$theftSet = [QueryAcceptance]::ReadIds($TheftIds)
$limitRuns = @(1..2 | ForEach-Object { Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--limit', '100', '--format', 'csv') })
$limitParts = @()
$limitOk = $theftSet.Count -eq (Get-SelectCount 'theft') -and ($TheftFigure -lt 0 -or $theftSet.Count -eq $TheftFigure)
$limitSets = @()
foreach ($run in $limitRuns) {
    $records = Read-CsvRecords $run.Stdout
    $count = if ($null -ne $records) { $records.Count } else { -1 }
    $allTheft = $null -ne $records -and @($records | Where-Object { $_[5] -cne 'THEFT' -or -not $theftSet.Contains([long]$_[0]) }).Count -eq 0
    $limitOk = $limitOk -and $run.ExitCode -eq 0 -and $count -eq 100 -and $allTheft
    $limitSets += , @($records | ForEach-Object { $_[0] } | Sort-Object)
    $limitParts += 'exit {0}, {1} records, all THEFT and in the select result: {2}' -f $run.ExitCode, $count, $allTheft
}
$sameHundred = ($limitSets[0] -join ',') -eq ($limitSets[1] -join ',')
Add-Result 'AC-C11' $limitOk ('`{0}` twice: {1}; select lists {2} THEFT IDs; both runs returned the same 100: {3} (allowed either way, C11)' -f `
        (Format-Query $limitRuns[0]), ($limitParts -join ' / '), (Format-Count $theftSet.Count), $sameHundred)

# --- AC-C16: the interpreted query in the JSON ---

$jsonRun = Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--range', $Month, '--format', 'json')
$document = try { $jsonRun.Stdout | ConvertFrom-Json } catch { $null }
$c16Ok = $false
$c16Text = 'the JSON does not parse'
if ($null -ne $document) {
    $conditions = @($document.query.conditions)
    $typeCondition = @($conditions | Where-Object { $_.field -eq 'Primary Type' })
    $dateCondition = @($conditions | Where-Object { $_.field -eq 'Date' })
    $interval = @(if ($dateCondition.Count -eq 1) { $dateCondition[0].intervals })
    $c16Ok = $jsonRun.ExitCode -eq 0 -and $typeCondition.Count -eq 1 -and (@($typeCondition[0].values) -join ',') -eq 'theft' `
        -and $interval.Count -eq 1 -and $interval[0].from -eq '2019-08-01T00:00:00' -and $interval[0].to -eq '2019-08-31T23:59:59' `
        -and $document.dataset.version -eq $current -and $document.dataset.schema_version -eq 2 -and @($document.results).Count -eq (Get-SelectCount 'theft-month')
    $c16Text = 'query {0}; dataset {1}; {2} results (select: {3}); CURRENT names {4}' -f `
    ($document.query | ConvertTo-Json -Depth 6 -Compress), ($document.dataset | ConvertTo-Json -Compress), @($document.results).Count, (Get-SelectCount 'theft-month'), $current
}
Add-Result 'AC-C16' $c16Ok ('`{0}`: exit {1}; {2}' -f (Format-Query $jsonRun), $jsonRun.ExitCode, $c16Text)

# --- AC-C17: case-insensitive text ---

$spellings = foreach ($spelling in @('theft', 'Theft', 'THEFT')) {
    $run = Invoke-Query -Arguments @('--where', "Primary Type=$spelling", '--count')
    [pscustomobject]@{ Spelling = $spelling; Count = Get-QueryCount $run }
}
$spellingsOk = @($spellings | Where-Object { ($TheftFigure -ge 0 -and $_.Count -ne $TheftFigure) -or $_.Count -ne (Get-SelectCount 'theft') }).Count -eq 0
$theftFigureText = if ($TheftFigure -ge 0) { Format-Count $TheftFigure } else { 'none (not the reference extract)' }
Add-Result 'AC-C17 (primary type spellings)' $spellingsOk ('{0}; PRD figure {1}, select {2}' -f `
    (($spellings | ForEach-Object { '{0} -> {1}' -f $_.Spelling, (Format-Count $_.Count) }) -join ', '), $theftFigureText, (Format-Count (Get-SelectCount 'theft')))
$listing = Invoke-Query -Arguments @('--where', 'Primary Type=theft', '--limit', '20', '--format', 'csv')
$listingRecords = Read-CsvRecords $listing.Stdout
$listingOk = $listing.ExitCode -eq 0 -and $null -ne $listingRecords -and $listingRecords.Count -eq 20 -and @($listingRecords | Where-Object { $_[5] -cne 'THEFT' }).Count -eq 0
Add-Result 'AC-C17 (listed as in the CSV)' $listingOk ('`{0}`: {1} records, Primary Type of each: {2}' -f `
    (Format-Query $listing), $(if ($listingRecords) { $listingRecords.Count } else { 0 }), $(if ($listingRecords) { (@($listingRecords | ForEach-Object { $_[5] }) | Sort-Object -Unique) -join ', ' } else { '-' }))
$lower = Test-Count 'AC-C17 (Location Description, lowercase)' @('--where', 'Primary Type=THEFT', '--range', $Month, '--where', 'Location Description=street') 'theft-month-street'
$upper = Test-Count 'AC-C17 (Location Description, uppercase)' @('--where', 'Primary Type=THEFT', '--range', $Month, '--where', 'Location Description=STREET') 'theft-month-street'
Add-Result 'AC-C17 (filter reported)' ($null -ne $lower.Stats -and $lower.Stats.Filters -eq 'Location Description' -and $lower.Count -eq $upper.Count) `
('both spellings count {0} and {1}; filter without an index reported: "{2}"' -f (Format-Count $lower.Count), (Format-Count $upper.Count), $(if ($lower.Stats) { $lower.Stats.Filters } else { '-' }))

# --- AC-C14: an output file in CSV and JSON ---

Write-Step 'AC-C14: output files'
$fileOut = Join-Path $OutDir 'file'
New-Item -ItemType Directory -Path $fileOut | Out-Null
$c14Parts = @()
$c14Ok = $true
foreach ($format in @('csv', 'json')) {
    $target = Join-Path $fileOut "result.$format"
    $caseArgs = @('--where', "Case Number=$sharedCase", '--format', $format)
    $toFile = Invoke-Query -Arguments ($caseArgs + @('--output', $target))
    $toStdout = Invoke-Query -Arguments $caseArgs
    $tableRun = Invoke-Query -Arguments @('--where', "Case Number=$sharedCase")
    $fileText = if (Test-Path -LiteralPath $target) { [System.IO.File]::ReadAllText($target, $Utf8) } else { $null }
    $fileIds = @(if ($format -eq 'csv') { @((Read-CsvRecords $fileText) | ForEach-Object { $_[0] } | Sort-Object) } else { @(($fileText | ConvertFrom-Json).results | ForEach-Object { [string]$_.ID } | Sort-Object) })
    $tableIds = @((Read-TableRows $tableRun.Stdout) | ForEach-Object { ($_ -split '\s+')[0] } | Sort-Object)
    $sameAsStdout = $null -ne $fileText -and $fileText -ceq $toStdout.Stdout
    $sameAsTable = $fileIds.Count -gt 0 -and ($fileIds -join ',') -eq ($tableIds -join ',')
    $stdoutEmpty = $toFile.Stdout -eq ''
    $statsOnStderr = $null -ne (Get-Stats $toFile.Stderr)
    # Again, with another query: the file is overwritten without a question (stdin is closed).
    $otherArgs = @('--where', "ID=$firstId", '--where', "ID=$lastId", '--format', $format)
    $again = Invoke-Query -Arguments ($otherArgs + @('--output', $target))
    $againStdout = Invoke-Query -Arguments $otherArgs
    $overwritten = $again.ExitCode -eq 0 -and [System.IO.File]::ReadAllText($target, $Utf8) -ceq $againStdout.Stdout -and $again.Stdout -eq ''
    $onlyFile = @(Get-EntryNames $fileOut | Where-Object { $_ -notlike 'result.*' }).Count -eq 0
    $ok = $toFile.ExitCode -eq 0 -and $sameAsStdout -and $sameAsTable -and $stdoutEmpty -and $statsOnStderr -and $overwritten -and $onlyFile
    $c14Ok = $c14Ok -and $ok
    $c14Parts += '{0}: exit {1}; the file equals the stdout output of the same query: {2}; its IDs equal the table''s ({3}): {4}; stdout empty: {5}; stats on stderr: {6}; a second query over the file exits {7} and replaces it with its own output: {8}; no temporary file left: {9}' -f `
        $format.ToUpperInvariant(), $toFile.ExitCode, $sameAsStdout, ($tableIds -join ', '), $sameAsTable, $stdoutEmpty, $statsOnStderr, $again.ExitCode, $overwritten, $onlyFile
}
Add-Result 'AC-C14' $c14Ok ('case number {0}, then IDs {1} and {2}. {3}' -f $sharedCase, $firstId, $lastId, ($c14Parts -join '. '))

# --- AC-C8: table, CSV and JSON hold the same records ---

Write-Step 'AC-C8: formats'
Test-Formats 'sample IDs and a missing ID' (@($sampleIds | ForEach-Object { '--where'; "ID=$($_.Id)" }) + @('--where', "ID=$MissingId")) @($Messages.MissingId)
Test-Formats 'shared case number' @('--where', "Case Number=$sharedCase") @()
Test-Formats 'THEFT on one day' @('--where', 'Primary Type=THEFT', '--range', $Day) @()
$dayRun = Invoke-Query -Arguments @('--where', 'Primary Type=THEFT', '--range', $Day, '--format', 'csv')
$dayFile = Join-Path $OutDir 'query-day.csv'
Write-Utf8 $dayFile $dayRun.Stdout
$dayDiff = [QueryAcceptance]::CompareRowSets($dayFile, $DayRows, $ByValue)
Add-Result 'AC-C8 (THEFT on one day equals the CSV rows)' ($dayRun.ExitCode -eq 0 -and $null -eq $dayDiff) `
('`{0}`: {1} rows; select: {2}; the same rows {4}, in any order: {3}' -f (Format-Query $dayRun), (@(Get-Lines $dayRun.Stdout).Count - 1), (Get-SelectCount 'theft-day'), $(if ($null -eq $dayDiff) { 'yes' } else { $dayDiff }), $RowMatchText)

# --- AC-C12: a new version is published while a query runs ---

Write-Step 'AC-C12: publishing version 2 while a query on version 1 is in flight'
$v1Name = Get-CurrentName $SwapDir
$swapQuery = Start-Program -FilePath $QueryExe -Arguments @('--dataset', $SwapDir, '--range', $Everything, '--format', 'csv') -HoldStdout
$queryStart = Get-Date
$swapReader = $swapQuery.Process.StandardOutput
$held = New-Object System.Text.StringBuilder
for ($i = 0; $i -lt 1001; $i++) {
    $line = $swapReader.ReadLine()
    if ($null -eq $line) { break }
    [void]$held.Append($line).Append("`n")
}
$firstRowsAt = Get-Date
$runningAtPublish = -not $swapQuery.Process.HasExited
$publish = Invoke-Generator -Source $Slice2 -Output $SwapDir -Log (Join-Path $LogDir 'swap-v2.log') -Overwrite
$publishedAt = Get-Date
$v2Name = Get-CurrentName $SwapDir
$v1Gone = -not (Test-Path -LiteralPath (Join-Path $SwapDir $v1Name))
$runningAfterPublish = -not $swapQuery.Process.HasExited
[void]$held.Append($swapReader.ReadToEnd())
$swapDone = Wait-Program $swapQuery
$queryEnd = Get-Date
$duringFile = Join-Path $OutDir 'query-swap-during.csv'
Write-Utf8 $duringFile $held.ToString()
$duringDiff = if ($swapDone.ExitCode -eq 0) { [QueryAcceptance]::CompareRowSets($duringFile, $Slice1Rows, $ByValue) } else { 'the query failed' }
$duringStats = Get-Stats $swapDone.Stderr
$afterFile = Join-Path $OutDir 'query-swap-after.csv'
$afterRun = Invoke-Query -Dataset $SwapDir -Arguments @('--range', $Everything, '--format', 'csv', '--output', $afterFile)
$afterDiff = if ($afterRun.ExitCode -eq 0) { [QueryAcceptance]::CompareRowSets($afterFile, $Slice2Rows, $ByValue) } else { 'the query failed' }
$switched = $publish.ExitCode -eq 0 -and $v2Name -ne $v1Name
Add-Result 'AC-C12 (query in flight)' ($switched -and $v1Gone -and $runningAtPublish -and $runningAfterPublish -and $swapDone.ExitCode -eq 0 -and $null -eq $duringDiff) `
('`crime-query --dataset swap --range "{0}" --format csv` on version 1 ({1}, {2} rows of slice 1) held after its first 1,000 rows; generator --overwrite with slice 2 exit {3}, CURRENT {1} -> {4}, old version folder gone: {5}; query still running when the generator ended: {6}; then exit {7}, {8} rows, equal to select on slice 1 ({9}) {11} in any order: {10}' -f `
        $Everything, $v1Name, (Format-Count $slice1Count), $publish.ExitCode, $v2Name, $v1Gone, $runningAfterPublish, $swapDone.ExitCode, $(if ($duringStats) { Format-Count $duringStats.Count } else { '-' }), `
    (Format-Count (Get-SelectCount 'slice-1')), $(if ($null -eq $duringDiff) { 'yes' } else { $duringDiff }), $RowMatchText)
Add-Result 'AC-C12 (next query)' ($switched -and $afterRun.ExitCode -eq 0 -and $null -eq $afterDiff) `
('a new query afterwards: exit {0}, equal to select on slice 2 ({1} rows) {3} in any order: {2}' -f `
        $afterRun.ExitCode, (Format-Count (Get-SelectCount 'slice-2')), $(if ($null -eq $afterDiff) { 'yes' } else { $afterDiff }), $RowMatchText)
Add-Section (@(
        '## AC-C12: timeline'
        ''
        '| Instant | Event |'
        '|---|---|'
        ('| {0:HH:mm:ss.fff} | query started on {1} (CURRENT) |' -f $queryStart, $v1Name)
        ('| {0:HH:mm:ss.fff} | its first 1,000 rows read; its standard output is not read further, so it cannot finish; still running: {1} |' -f $firstRowsAt, $runningAtPublish)
        ('| {0:HH:mm:ss.fff} | generator --overwrite with slice 2 ended (exit {1}, {2}); CURRENT names {3}; {4} exists: {5} |' -f $publishedAt, $publish.ExitCode, (Format-Seconds $publish.WallSeconds), $v2Name, $v1Name, (-not $v1Gone))
        ('| {0:HH:mm:ss.fff} | query ended (exit {1}) after its output was read to the end; still running before that: {2}; stats "{3}" |' -f $queryEnd, $swapDone.ExitCode, $runningAfterPublish, $(if ($duringStats) { $duringStats.Line } else { '-' }))
    ) -join "`n")

# --- AC-C1: the sample by ID ---

Write-Step 'AC-C1: the sample by ID'
Complete-Selects
$sampleText = [System.IO.File]::ReadAllText($SampleRows, $Utf8)
$sampleLines = @($sampleText -split "`n" | Where-Object { $_ -ne '' })
$c1Rows = New-Object System.Collections.Generic.List[string]
$c1Ok = $sampleLines.Count -eq 6
foreach ($sample in $sampleIds) {
    $run = Invoke-Query -Arguments @('--where', "ID=$($sample.Id)", '--format', 'csv')
    # select copies the source line byte for byte, its ID quoted or not; the first and last rows are
    # also read from the CSV directly.
    $quotedPrefix = '"{0}",' -f $sample.Id
    $barePrefix = '{0},' -f $sample.Id
    $sourceLine = @($sampleLines | Where-Object { $_.StartsWith($quotedPrefix) -or $_.StartsWith($barePrefix) }) | Select-Object -First 1
    if ($sample.Id -eq $firstId -and $sourceLine -cne $edgeRows[0]) { $sourceLine = $null }
    if ($sample.Id -eq $lastId -and $sourceLine -cne $edgeRows[1]) { $sourceLine = $null }
    $expected = if ($sourceLine -and (Test-SameCsvText $sampleLines[0] $Header $ByValue)) { $sampleLines[0] + "`n" + $sourceLine + "`n" } else { $null }
    $identical = $run.ExitCode -eq 0 -and $null -ne $expected -and (Test-SameCsvText $run.Stdout $expected $ByValue)
    # Only the reference extract quotes every non-empty field, so only there is a bare empty field notable.
    $bare = if (-not $ByValue -and $sourceLine -and $sourceLine.Contains(',,')) { ' (with bare empty fields)' } else { '' }
    $queried = Read-CsvRecords $run.Stdout
    $source = if ($expected) { Read-CsvRecords $expected } else { $null }
    $dateText = if ($queried -and $queried.Count -eq 1) { $queried[0][2] } else { '-' }
    $dateExact = $null -ne $source -and $dateText -ceq $source[0][2]
    $spring = $sample.Label -ne 'spring-forward hour' -or $dateText -like '03/10/2019 02:*'
    $c1Ok = $c1Ok -and $identical -and $dateExact -and $spring
    $sameText = if ($ByValue) { 'equal field by field to' } else { 'byte-identical to' }
    $c1Rows.Add(('{0} ID {1}: header and row {6} the source line{2}: {3}, Date "{4}" as published: {5}' -f $sample.Label, $sample.Id, $bare, $identical, $dateText, $dateExact, $sameText))
}
Add-Result 'AC-C1' $c1Ok (($c1Rows -join '; ') + ' (each queried alone with `--format csv`; the source lines copied by `crime-acceptance select --where ID=... --rows-out`, the first and last also read from the CSV directly)')

# --- AC-S1 to AC-S11: prefix conditions (startswith PRD) ---

Write-Step 'AC-S: prefix conditions'
$crimSet = [QueryAcceptance]::ReadIds($CrimIds)

# AC-S1: the three spellings give the same count; a listing shows the type as in the CSV.
$crimSpellings = @(foreach ($spelling in @('crim', 'Crim', 'CRIM')) {
        Test-Count "AC-S1 (Primary Type starts with $spelling)" @('--startswith', "Primary Type=$spelling") 'prefix-crim' $CrimFigure
    })
$crimListing = Invoke-Query -Arguments @('--startswith', 'Primary Type=crim', '--limit', '20', '--format', 'csv')
$crimRecords = Read-CsvRecords $crimListing.Stdout
$crimTypes = if ($null -ne $crimRecords) { (@($crimRecords | ForEach-Object { $_[5] }) | Sort-Object -Unique) -join ', ' } else { '-' }
$crimListed = $crimListing.ExitCode -eq 0 -and $null -ne $crimRecords -and $crimRecords.Count -eq 20 `
    -and @($crimRecords | Where-Object { -not $_[5].StartsWith('CRIM', [System.StringComparison]::Ordinal) -or -not $crimSet.Contains([long]$_[0]) }).Count -eq 0
Add-Result 'AC-S1 (listed as in the CSV)' $crimListed ('`{0}`: {1} records, all in the select result, Primary Type of each (uppercase as in the CSV): {2}' -f `
        (Format-Query $crimListing), $(if ($null -ne $crimRecords) { $crimRecords.Count } else { 0 }), $crimTypes)

# AC-S2: a prefix on the case number is an indexed condition on its own.
$s2 = Test-Count 'AC-S2' @('--startswith', 'Case Number=JA') 'prefix-ja' $JaFigure -Extra '; no other condition'
Add-Result 'AC-S2 (index reported)' ($null -ne $s2.Stats -and $s2.Stats.Indexes -eq 'Case Number') ('stats line reports the indexes used: "{0}"' -f $(if ($s2.Stats) { $s2.Stats.Indexes } else { 'missing' }))

# AC-S3: an equality and a prefix on two indexed fields.
$s3 = Test-Count 'AC-S3' @('--where', 'Primary Type=THEFT', '--startswith', 'District=01') 'theft-prefix-01'
Add-Result 'AC-S3 (indexes reported)' ($null -ne $s3.Stats -and $s3.Stats.Indexes -eq 'Primary Type, District') ('stats line reports the indexes used: "{0}"' -f $(if ($s3.Stats) { $s3.Stats.Indexes } else { 'missing' }))

# AC-S4: a prefix on a field without an index is a filter; alone it is refused (C10).
$s4 = Test-Count 'AC-S4' @('--startswith', 'Location Description=resid', '--range', $Month) 'prefix-resid-month'
Add-Result 'AC-S4 (filter reported)' ($null -ne $s4.Stats -and $s4.Stats.Indexes -eq 'Date' -and $s4.Stats.Filters -eq 'Location Description') `
('stats line: indexes "{0}", filters without an index "{1}"' -f $(if ($s4.Stats) { $s4.Stats.Indexes } else { '-' }), $(if ($s4.Stats) { $s4.Stats.Filters } else { '-' }))
$residAlone = Test-Rejection 'only prefixes on fields without an index' @('--startswith', 'Location Description=resid', '--count') $Messages.NoIndexed 1 -Criterion 'AC-S11'
Add-Result 'AC-S4 (prefix alone rejected)' ($residAlone.ExitCode -eq 1 -and $residAlone.Stderr.Trim() -ceq $Messages.NoIndexed) `
('`{0}`: exit {1}; stderr "{2}"' -f (Format-Query $residAlone), $residAlone.ExitCode, $residAlone.Stderr.Trim())

# AC-S5: overlapping alternative prefixes count each record once.
$crimAlone = $crimSpellings[2]
$s5 = Test-Count 'AC-S5' @('--startswith', 'Primary Type=CRIM', '--startswith', 'Primary Type=CRIMINAL D') 'prefix-crim' $CrimFigure -Extra ('; the query with CRIM alone: {0}' -f (Format-Count $crimAlone.Count))
Add-Result 'AC-S5 (no record counted twice)' ($s5.Count -ge 0 -and $s5.Count -eq $crimAlone.Count) ('CRIM or CRIMINAL D: {0}; CRIM alone: {1}' -f (Format-Count $s5.Count), (Format-Count $crimAlone.Count))

# AC-S6 (P1): an equality and a prefix on the same field are alternatives.
Test-Count 'AC-S6' @('--where', 'Primary Type=THEFT', '--startswith', 'Primary Type=CRIM') 'theft-or-prefix-crim' $TheftCrimFigure | Out-Null

# AC-S7: codes are never padded; no match is an empty result without a warning (P4).
$s7 = Test-Count 'AC-S7 (District starts with 8)' @('--startswith', 'District=8') 'prefix-district-8' $DistrictEightFigure
$s7Warnings = @(Get-Lines $s7.Run.Stderr | Where-Object { $_ -ne $(if ($s7.Stats) { $s7.Stats.Line } else { '' }) })
Add-Result 'AC-S7 (no error, no warning)' ($s7.Run.ExitCode -eq 0 -and $null -ne $s7.Stats -and $s7Warnings.Count -eq 0 -and ($DistrictEightFigure -lt 0 -or $s7.Count -eq 0)) `
('`{0}`: exit {1}, count {2}; stderr besides the stats line: {3}' -f (Format-Query $s7.Run), $s7.Run.ExitCode, (Format-Count $s7.Count), $(if ($s7Warnings.Count -eq 0) { 'nothing' } else { $s7Warnings -join ' / ' }))
Test-Count 'AC-S7 (District starts with 00)' @('--startswith', 'District=00') 'prefix-district-00' | Out-Null

# AC-S8: the count behind the memory group (measured with AC-C13, on a quiet machine).
$hRun = $prefixMemoryRuns['count Case Number starts with H']
$hCount = Get-QueryCount $hRun
$hFixedText = if ($CaseHFigure -ge 0) { '; PRD figure {0}' -f (Format-Count $CaseHFigure) } else { '' }
Add-Result 'AC-S8 (count of H)' ($hCount -ge 0 -and $hCount -eq (Get-SelectCount 'prefix-case-h') -and ($CaseHFigure -lt 0 -or $hCount -eq $CaseHFigure)) `
('`{0}`: {1}; select on the CSV: {2}{3}' -f (Format-Query $hRun), (Format-Count $hCount), (Format-Count (Get-SelectCount 'prefix-case-h')), $hFixedText)

# AC-S9: table, CSV and JSON hold the same records; the JSON holds the prefix in lowercase.
$crimDayArgs = @('--startswith', 'Primary Type=CRIM', '--range', $Day)
Test-Formats 'Primary Type starts with CRIM on one day' $crimDayArgs @() -Criterion 'AC-S9'
$crimDayRun = Invoke-Query -Arguments ($crimDayArgs + @('--format', 'csv'))
$crimDayFile = Join-Path $OutDir 'query-prefix-crim-day.csv'
Write-Utf8 $crimDayFile $crimDayRun.Stdout
$crimDayDiff = if ($crimDayRun.ExitCode -eq 0) { [QueryAcceptance]::CompareRowSets($crimDayFile, $CrimDayRows, $ByValue) } else { 'the query failed' }
Add-Result 'AC-S9 (equals the CSV rows)' ($null -eq $crimDayDiff) ('`{0}`: {1} rows; select: {2}; the same rows {4}, in any order: {3}' -f `
        (Format-Query $crimDayRun), (@(Get-Lines $crimDayRun.Stdout).Count - 1), (Get-SelectCount 'prefix-crim-day'), $(if ($null -eq $crimDayDiff) { 'yes' } else { $crimDayDiff }), $RowMatchText)
$s9Run = Invoke-Query -Arguments ($crimDayArgs + @('--format', 'json'))
$s9Document = try { $s9Run.Stdout | ConvertFrom-Json } catch { $null }
$s9Prefixes = '-'
$s9Ok = $false
$s9Query = 'the JSON does not parse'
if ($null -ne $s9Document) {
    $s9Condition = @(@($s9Document.query.conditions) | Where-Object { $_.field -eq 'Primary Type' })
    if ($s9Condition.Count -eq 1 -and $null -ne $s9Condition[0].PSObject.Properties['startswith']) { $s9Prefixes = @($s9Condition[0].startswith) -join ',' }
    $s9Ok = $s9Run.ExitCode -eq 0 -and $s9Condition.Count -eq 1 -and $s9Prefixes -ceq 'crim'
    $s9Query = $s9Document.query | ConvertTo-Json -Depth 6 -Compress
}
Add-Result 'AC-S9 (condition in the JSON)' $s9Ok ('`{0}`: exit {1}; query {2}; the Primary Type condition''s startswith holds [{3}] (expected the lowercase prefix crim)' -f `
        (Format-Query $s9Run), $s9Run.ExitCode, $s9Query, $s9Prefixes)

# AC-S10 (C11): a limit returns exactly N records of the unlimited result.
$s10Run = Invoke-Query -Arguments @('--startswith', 'Primary Type=CRIM', '--limit', '100', '--format', 'csv')
$s10Records = Read-CsvRecords $s10Run.Stdout
$s10Count = if ($null -ne $s10Records) { $s10Records.Count } else { -1 }
$s10InResult = $null -ne $s10Records -and @($s10Records | Where-Object { -not $crimSet.Contains([long]$_[0]) }).Count -eq 0
$s10Distinct = $null -ne $s10Records -and @($s10Records | ForEach-Object { $_[0] } | Sort-Object -Unique).Count -eq $s10Count
$s10Ok = $s10Run.ExitCode -eq 0 -and $crimSet.Count -gt 100 -and $crimSet.Count -eq (Get-SelectCount 'prefix-crim') -and $s10Count -eq 100 -and $s10InResult -and $s10Distinct
Add-Result 'AC-S10' $s10Ok ('`{0}`: exit {1}, {2} records, distinct: {3}, all in the {4} IDs select lists for CRIM: {5}' -f `
        (Format-Query $s10Run), $s10Run.ExitCode, $s10Count, $s10Distinct, (Format-Count $crimSet.Count), $s10InResult)

# AC-S11: every row of the PRD's validation table (the C10 row is checked with AC-S4 above).
Test-Rejection 'empty prefix' @('--startswith', 'Primary Type=') $Messages.EmptyPrefix 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'prefix on a field that is not text (Ward)' @('--startswith', 'Ward=1') $Messages.NotText 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'prefix on a field that is not text (Date)' @('--startswith', 'Date=2019') $Messages.NotText 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'prefix on Block' @('--startswith', 'Block=001') $Messages.BlockCondition 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'prefix on Description' @('--startswith', 'Description=SIMPLE') $Messages.DescCondition 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'prefix on a nonexistent field' @('--startswith', 'Gravedad=alta') $Messages.UnknownField 1 -Criterion 'AC-S11' | Out-Null
Test-Rejection 'condition without =' @('--startswith', 'Primary Type') $Messages.PrefixForm 2 -Criterion 'AC-S11' | Out-Null

# --- AC-C18: the independent verifier ---

Write-Step 'AC-C18: waiting for the independent verifier'
$verifier = Wait-Program $verifierStarted
$verdicts = @{}
foreach ($line in Get-Lines $verifier.Stdout) {
    $match = [regex]::Match($line, '^(PASS|FAIL|SKIP)  (\S+)  (.*)$')
    if ($match.Success) { $verdicts[$match.Groups[2].Value] = [pscustomobject]@{ Outcome = $match.Groups[1].Value; Summary = $match.Groups[3].Value } }
}
$notPassed = @($verdicts.Keys | Where-Object { $verdicts[$_].Outcome -ne 'PASS' })
$named = foreach ($criterion in @('AC-C18-LOWERCASE', 'AC-G3', 'AC-G3-SAMPLE', 'MANIFEST-SCHEMA')) {
    if ($verdicts.ContainsKey($criterion)) { '{0} {1}: {2}' -f $criterion, $verdicts[$criterion].Outcome, $verdicts[$criterion].Summary } else { "$criterion missing" }
}
$verifierOk = $verifier.ExitCode -eq 0 -and $verdicts.Count -gt 0 -and $notPassed.Count -eq 0 -and $verdicts.ContainsKey('AC-C18-LOWERCASE') -and $verdicts.ContainsKey('AC-G3')
$verifierFlag = if ($IsReference) { ' --reference' } else { '' }
Add-Result 'AC-C18 (verifier)' $verifierOk ('`crime-acceptance --csv <csv> --dataset full{0}`: exit {1} in {2}, {3} checks, not PASS: {4}; {5}' -f `
        $verifierFlag, $verifier.ExitCode, (Format-Seconds $verifier.WallSeconds), $verdicts.Count, $(if ($notPassed.Count -eq 0) { 'none' } else { $notPassed -join ', ' }), ($named -join ' / '))
Add-Section (@(
        '## Independent verifier on the full dataset'
        ''
        ('`crime-acceptance --csv <CSV> --dataset full{0}`: exit {1}.' -f $verifierFlag, $verifier.ExitCode)
        ''
        (Format-Code $verifier.Stdout)
    ) -join "`n")

# --- AC-S12: the consultor criteria give the same results ---

$consultorRows = @($Results | Where-Object { $_.Criterion -match '^AC-C[0-9]+' })
$consultorFailed = @($consultorRows | Where-Object { $_.Result -ne 'PASS' })
$consultorCriteria = @($consultorRows | ForEach-Object { [regex]::Match($_.Criterion, '^AC-C[0-9]+').Value } | Sort-Object -Unique)
Add-Result 'AC-S12' ($consultorRows.Count -gt 0 -and $consultorFailed.Count -eq 0) ('the consultor criteria repeated on this build: {0} AC-C rows of {1} criteria, not PASS: {2}' -f `
        $consultorRows.Count, $consultorCriteria.Count, $(if ($consultorFailed.Count -eq 0) { 'none' } else { (@($consultorFailed | ForEach-Object { $_.Criterion }) -join ', ') }))

# --- Report ---

$selectRows = foreach ($select in $Selects.Values) {
    '| {0} | `{1}` | {2} | {3} | {4} |' -f $select.Name, $select.Command.Replace('|', '\|'), $select.Run.ExitCode, (Format-Count $select.Count), (Format-Seconds $select.Run.WallSeconds)
}
Add-Section (@(
        '## Independent counts (crime-acceptance select)'
        ''
        '| Name | Command | Exit | Matching rows | Wall time |'
        '|---|---|---|---|---|'
        $selectRows
    ) -join "`n")

# AC-C rows by criterion, then the rows of no criterion, then AC-S rows by criterion.
$ordered = @($Results | Sort-Object @{ Expression = {
            $m = [regex]::Match($_.Criterion, '^AC-([CS])([0-9]+)')
            if (-not $m.Success) { 99 } elseif ($m.Groups[1].Value -eq 'C') { [int]$m.Groups[2].Value } else { 1000 + [int]$m.Groups[2].Value }
        }
    }, Order)
$failed = @($Results | Where-Object { $_.Result -ne 'PASS' })
$resultRows = foreach ($result in $ordered) {
    '| {0} | {1} | {2} |' -f $result.Criterion, $result.Result, $result.Evidence.Replace('|', '\|')
}
$criteriaSeen = @($Results | ForEach-Object { [regex]::Match($_.Criterion, '^AC-[CS][0-9]+').Value } | Where-Object { $_ } | Sort-Object -Unique)
$overall = if ($failed.Count -eq 0) { "PASS: all $($Results.Count) checks passed ($($criteriaSeen.Count) criteria)" } else { "FAIL: $($failed.Count) of $($Results.Count) checks failed" }
$duration = ((Get-Date) - $started).TotalSeconds
$report = @(
    '# Acceptance of the crime query app'
    ''
    ('- **Result**: {0}' -f $overall)
    ('- **Date**: {0:yyyy-MM-dd HH:mm} (duration {1})' -f $started, (Format-Seconds $duration))
    ('- **Commit**: {0}' -f $commit)
    ('- **Machine**: {0}, {1} logical processors, {2} GiB RAM, {3}' -f $processor, [Environment]::ProcessorCount, $memoryGiB, $os)
    ('- **CSV**: `{0}`, {1} bytes' -f $Csv, (Format-Count $csvSize))
    ('- {0}' -f (Format-CsvMode $SourceSha256))
    ('- **Dataset**: `{0}`, version {1}, schema {2}, {3} records, generated in {4}' -f $DatasetDir, $current, $manifest.schema_version, (Format-Count $manifest.records.valid), (Format-Seconds $generation.WallSeconds))
    ('- **Small datasets**: small ({0} rows), swap v1 / v2 ({1} / {2} rows)' -f (Format-Count $smallCount), (Format-Count $slice1Count), (Format-Count $slice2Count))
    ''
    '## Results'
    ''
    '| Criterion | Result | Evidence |'
    '|---|---|---|'
    $resultRows
    ''
    ($Sections -join "`n`n")
) -join "`n"
Write-Utf8 $ReportPath ($report + "`n")
Write-Step "Report written to $ReportPath"
Write-Host $overall
if ($failed.Count -gt 0) { exit 1 }
exit 0
