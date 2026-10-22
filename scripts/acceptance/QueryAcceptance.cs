// Compiled helpers of the query app acceptance checks and the comparison bench, loaded with
// Add-Type -Path: they read files of millions of lines and the memory counters of a process.
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
