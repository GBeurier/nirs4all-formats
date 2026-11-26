using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Nodes;

namespace Nirs4all.Formats;

/// <summary>
/// Read spectroscopy files and export records through the nirs4all-formats native library.
/// </summary>
/// <remarks>
/// The native library <c>nirs4all_formats_capi</c> is loaded from the file named by the
/// <c>NIRS4ALL_FORMATS_LIBRARY</c> environment variable when it is set, otherwise by .NET's
/// default probing: the package's <c>runtimes/&lt;rid&gt;/native/</c> folder, the
/// application directory, then the system library path. Every method throws
/// <see cref="FormatsException"/> when the native call fails.
/// </remarks>
public static class Nirs4allFormats
{
    /// <summary>Version of the native nirs4all-formats library.</summary>
    public static string Version => Native.Take(Native.n4fmt_library_version()) ?? "";

    /// <summary>Version of the C ABI.</summary>
    public static string AbiVersion => Native.Take(Native.n4fmt_abi_version()) ?? "";

    /// <summary>Readers compiled into the native library.</summary>
    public static IReadOnlyList<string> Readers() =>
        Parse(Native.Call(Native.n4fmt_reader_catalog)).AsArray()
            .Select(r => JsonHelpers.Text(r!, "reader") ?? "")
            .ToList();

    /// <summary>Readers that recognize the file, most confident first.</summary>
    public static IReadOnlyList<FormatProbe> Probe(string path) =>
        Probes(Native.Call((out IntPtr json, out IntPtr error) => Native.n4fmt_probe_path(path, out json, out error)));

    /// <summary>Readers that recognize in-memory content named <paramref name="name"/>, most confident first.</summary>
    public static IReadOnlyList<FormatProbe> Probe(string name, byte[] bytes) =>
        Probes(Native.Call((out IntPtr json, out IntPtr error) =>
            Native.n4fmt_probe_bytes(name, bytes, (nuint)bytes.Length, out json, out error)));

    /// <summary>Read a file. Companion files of sidecar formats are found next to it.</summary>
    public static IReadOnlyList<SpectralRecord> Open(string path, ReadOptions? options = null)
    {
        string optionsJson = (options ?? new ReadOptions()).ToJson();
        return Records(Native.Call((out IntPtr json, out IntPtr error) =>
            Native.n4fmt_open_path(path, optionsJson, out json, out error)));
    }

    /// <summary>
    /// Read in-memory content. <paramref name="name"/> is the file name, used for sniffing and
    /// provenance. Formats that need a companion file fail with <see cref="FormatsStatus.UnsupportedSidecar"/>.
    /// </summary>
    public static IReadOnlyList<SpectralRecord> Open(string name, byte[] bytes, ReadOptions? options = null)
    {
        string optionsJson = (options ?? new ReadOptions()).ToJson();
        return Records(Native.Call((out IntPtr json, out IntPtr error) =>
            Native.n4fmt_open_bytes(name, bytes, (nuint)bytes.Length, optionsJson, out json, out error)));
    }

    /// <summary>Export formats and software presets.</summary>
    public static ExportTargets ExportTargets() =>
        global::Nirs4all.Formats.ExportTargets.FromJson(Parse(Native.Call(Native.n4fmt_export_targets)));

    /// <summary>Write records to an export format or software preset, in memory.</summary>
    public static WriteResult Write(IEnumerable<SpectralRecord> records, string target, WriteOptions? options = null)
    {
        string recordsJson = "[" + string.Join(",", records.Select(r => r.ToJsonString())) + "]";
        string optionsJson = (options ?? new WriteOptions()).ToJson(withOverwrite: false);
        return WriteResult.FromJson(Parse(Native.Call((out IntPtr json, out IntPtr error) =>
            Native.n4fmt_write_records(recordsJson, target, optionsJson, out json, out error))));
    }

    /// <summary>
    /// Read <paramref name="inputPath"/> and save its export into the existing directory
    /// <paramref name="outputDirectory"/>. Existing files are kept, and nothing is written,
    /// unless <see cref="WriteOptions.Overwrite"/> is set.
    /// </summary>
    public static ConvertResult Convert(string inputPath, string target, string outputDirectory, WriteOptions? options = null)
    {
        string optionsJson = (options ?? new WriteOptions()).ToJson(withOverwrite: true);
        return ConvertResult.FromJson(Parse(Native.Call((out IntPtr json, out IntPtr error) =>
            Native.n4fmt_convert_path(inputPath, target, outputDirectory, optionsJson, out json, out error))));
    }

    private static JsonNode Parse(string json) =>
        JsonNode.Parse(json) ?? throw new InvalidOperationException("the native library returned null");

    private static IReadOnlyList<SpectralRecord> Records(string json)
    {
        var array = Parse(json).AsArray();
        var records = new List<SpectralRecord>(array.Count);
        // Detach each record from the array so it can be serialized on its own.
        for (int i = array.Count - 1; i >= 0; i--)
        {
            var record = array[i]!.AsObject();
            array.RemoveAt(i);
            records.Add(new SpectralRecord(record));
        }
        records.Reverse();
        return records;
    }

    private static IReadOnlyList<FormatProbe> Probes(string json) =>
        Parse(json).AsArray()
            .Select(p => new FormatProbe(
                JsonHelpers.Text(p!, "format") ?? "",
                JsonHelpers.Text(p!, "reader") ?? "",
                JsonHelpers.Text(p!, "confidence") ?? "",
                JsonHelpers.Text(p!, "reason") ?? ""))
            .ToList();
}
