// Compiled helpers of the CanopyDB query app acceptance and the comparison bench, loaded with
// Add-Type -Path.
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
