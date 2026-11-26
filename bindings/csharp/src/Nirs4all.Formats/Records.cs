using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Nodes;

namespace Nirs4all.Formats;

/// <summary>Coordinate of one signal dimension.</summary>
/// <param name="Values">Native axis values, in the stored order.</param>
/// <param name="Unit">Unit such as <c>nm</c>, <c>cm-1</c> or <c>index</c>.</param>
/// <param name="Kind"><c>wavelength</c>, <c>wavenumber</c>, <c>frequency</c>, <c>energy</c>, <c>time</c> or <c>index</c>.</param>
/// <param name="Order"><c>ascending</c>, <c>descending</c> or <c>non_monotonic</c>.</param>
public sealed record SpectralAxis(double[] Values, string Unit, string Kind, string Order)
{
    /// <summary>Number of axis points.</summary>
    public int Count => Values.Length;

    /// <summary>Whether both axes have the same values, unit and kind.</summary>
    public bool SameAs(SpectralAxis other) =>
        Unit == other.Unit && Kind == other.Kind && Values.AsSpan().SequenceEqual(other.Values);

    internal static SpectralAxis FromJson(JsonNode node) => new(
        JsonHelpers.Doubles(node["values"]),
        JsonHelpers.Text(node, "unit") ?? "",
        JsonHelpers.Text(node, "kind") ?? "",
        JsonHelpers.Text(node, "order") ?? "");
}

/// <summary>One named signal channel of a record. Each signal owns its axis.</summary>
/// <param name="Name">Signal name within the record.</param>
/// <param name="Axis">Coordinate of the spectral (<c>x</c>) dimension.</param>
/// <param name="Values">Flat C-order (row-major) values; missing points are NaN.</param>
/// <param name="Shape">Extent of each dimension.</param>
/// <param name="Dims">Dimension names; exactly one is <c>x</c>.</param>
/// <param name="Coords">Coordinates of the non-<c>x</c> dimensions.</param>
/// <param name="SignalType">Signal type such as <c>absorbance</c> or <c>reflectance</c>.</param>
/// <param name="Unit">Physical unit of the values, or null.</param>
/// <param name="Role">Role such as <c>signal</c> or <c>white_ref</c>.</param>
/// <param name="Source"><c>file</c> or <c>derived</c>.</param>
public sealed record Signal(
    string Name,
    SpectralAxis Axis,
    double[] Values,
    int[] Shape,
    IReadOnlyList<string> Dims,
    IReadOnlyDictionary<string, SpectralAxis> Coords,
    string SignalType,
    string? Unit,
    string Role,
    string Source)
{
    /// <summary>Whether this is a plain spectrum (<c>dims == ["x"]</c>).</summary>
    public bool IsOneDimensional => Dims.Count == 1 && Dims[0] == "x";

    internal static Signal FromJson(string name, JsonNode node)
    {
        var coords = new Dictionary<string, SpectralAxis>();
        if (node["coords"] is JsonObject coordObject)
        {
            foreach (var (key, value) in coordObject)
            {
                coords[key] = SpectralAxis.FromJson(value!);
            }
        }
        return new Signal(
            name,
            SpectralAxis.FromJson(node["axis"]!),
            JsonHelpers.Doubles(node["values"]),
            node["shape"]!.AsArray().Select(v => v!.GetValue<int>()).ToArray(),
            JsonHelpers.Strings(node["dims"]),
            coords,
            JsonHelpers.Text(node, "signal_type") ?? "",
            JsonHelpers.Text(node, "unit"),
            JsonHelpers.Text(node, "role") ?? "",
            JsonHelpers.Text(node, "source") ?? "");
    }
}

/// <summary>One physical input file or archive member.</summary>
/// <param name="Path">File path or name.</param>
/// <param name="Archive">Enclosing archive, or null.</param>
/// <param name="Sha256">SHA-256 of the bytes, hexadecimal.</param>
/// <param name="Role"><c>primary</c> or a sidecar role.</param>
public sealed record SourceFile(string Path, string? Archive, string Sha256, string Role);

/// <summary>Where a record comes from.</summary>
/// <param name="Format">Detected format id, such as <c>jcamp-dx</c>.</param>
/// <param name="Reader">Reader that decoded the file.</param>
/// <param name="ReaderVersion">Library version of the reader.</param>
/// <param name="Sources">Input files with their SHA-256.</param>
/// <param name="ParsedAtUtc">Parse timestamp, or null.</param>
/// <param name="RecordSchemaVersion">Version of the record schema.</param>
/// <param name="Warnings">Decoding warnings.</param>
public sealed record Provenance(
    string Format,
    string Reader,
    string ReaderVersion,
    IReadOnlyList<SourceFile> Sources,
    string? ParsedAtUtc,
    string RecordSchemaVersion,
    IReadOnlyList<string> Warnings)
{
    internal static Provenance FromJson(JsonNode node) => new(
        JsonHelpers.Text(node, "format") ?? "",
        JsonHelpers.Text(node, "reader") ?? "",
        JsonHelpers.Text(node, "reader_version") ?? "",
        (node["sources"]?.AsArray() ?? new JsonArray())
            .Select(s => new SourceFile(
                JsonHelpers.Text(s!, "path") ?? "",
                JsonHelpers.Text(s!, "archive"),
                JsonHelpers.Text(s!, "sha256") ?? "",
                JsonHelpers.Text(s!, "role") ?? ""))
            .ToList(),
        JsonHelpers.Text(node, "parsed_at_utc"),
        JsonHelpers.Text(node, "record_schema_version") ?? "",
        JsonHelpers.Strings(node["warnings"]));
}

/// <summary>
/// One normalized sample or acquisition, as decoded by the Rust registry. The typed
/// properties are read-only views; <see cref="Json"/> is the complete record, which
/// <see cref="Nirs4allFormats.Write"/> exports unchanged.
/// </summary>
public sealed class SpectralRecord
{
    private readonly JsonObject _json;

    internal SpectralRecord(JsonObject json)
    {
        _json = json;
        Signals = json["signals"]!.AsObject().ToDictionary(e => e.Key, e => Signal.FromJson(e.Key, e.Value!));
        Targets = (json["targets"]?.AsObject() ?? new JsonObject())
            .ToDictionary(e => e.Key, e => e.Value?.DeepClone());
        Provenance = Provenance.FromJson(json["provenance"]!);
        QualityFlags = JsonHelpers.Strings(json["quality_flags"]);
    }

    /// <summary>Signals by name.</summary>
    public IReadOnlyDictionary<string, Signal> Signals { get; }

    /// <summary>Dominant signal type, such as <c>absorbance</c>.</summary>
    public string SignalType => JsonHelpers.Text(_json, "signal_type") ?? "";

    /// <summary>Reference values by name (numbers, strings, or null when missing).</summary>
    public IReadOnlyDictionary<string, JsonNode?> Targets { get; }

    /// <summary>Reader, format, source hashes and warnings.</summary>
    public Provenance Provenance { get; }

    /// <summary>Explicit caveats on the record.</summary>
    public IReadOnlyList<string> QualityFlags { get; }

    /// <summary>Acquisition, instrument and sample metadata (a copy).</summary>
    public JsonObject Metadata => _json["metadata"]?.DeepClone().AsObject() ?? new JsonObject();

    /// <summary>The complete record JSON (a copy).</summary>
    public JsonObject Json => _json.DeepClone().AsObject();

    /// <summary>A numeric target, or NaN when it is missing or not a number.</summary>
    public double NumericTarget(string name) =>
        Targets.TryGetValue(name, out JsonNode? value) && value is JsonValue v && v.TryGetValue(out double number)
            ? number
            : double.NaN;

    internal string ToJsonString() => _json.ToJsonString();

    /// <inheritdoc/>
    public override string ToString() =>
        $"SpectralRecord {{ Format = {Provenance.Format}, Signals = [{string.Join(", ", Signals.Keys)}] }}";
}

/// <summary>One signal of several records stacked into a matrix, one row per record.</summary>
/// <param name="SignalName">Signal name.</param>
/// <param name="Axis">Axis shared by every row.</param>
/// <param name="Values"><c>Values[record, point]</c>; missing points are NaN.</param>
public sealed record SignalMatrix(string SignalName, SpectralAxis Axis, double[,] Values)
{
    /// <summary>Number of rows (records).</summary>
    public int Rows => Values.GetLength(0);

    /// <summary>Number of columns (axis points).</summary>
    public int Columns => Values.GetLength(1);

    /// <summary>
    /// Stack the named 1-D signal of every record. The axes must be identical (values,
    /// unit and kind): nothing is resampled. Without a name, the only signal of the
    /// first record is used.
    /// </summary>
    /// <exception cref="ArgumentException">A record lacks the signal, the signal is not 1-D or the axes differ.</exception>
    public static SignalMatrix Stack(IReadOnlyList<SpectralRecord> records, string? signal = null)
    {
        if (records.Count == 0)
        {
            throw new ArgumentException("no records to stack", nameof(records));
        }
        if (signal is null)
        {
            var names = records[0].Signals.Keys.ToList();
            signal = names.Count == 1
                ? names[0]
                : throw new ArgumentException($"records have several signals [{string.Join(", ", names)}]; name one", nameof(signal));
        }
        SpectralAxis? axis = null;
        double[,]? values = null;
        for (int i = 0; i < records.Count; i++)
        {
            if (!records[i].Signals.TryGetValue(signal, out Signal? current))
            {
                throw new ArgumentException($"record {i} has no signal '{signal}'", nameof(records));
            }
            if (!current.IsOneDimensional)
            {
                throw new ArgumentException($"signal '{signal}' of record {i} is not 1-D", nameof(records));
            }
            if (axis is null)
            {
                axis = current.Axis;
                values = new double[records.Count, axis.Count];
            }
            else if (!axis.SameAs(current.Axis))
            {
                throw new ArgumentException($"the axis of signal '{signal}' in record {i} differs from record 0", nameof(records));
            }
            for (int j = 0; j < current.Values.Length; j++)
            {
                values![i, j] = current.Values[j];
            }
        }
        return new SignalMatrix(signal, axis!, values!);
    }
}

/// <summary>A reader that recognizes a file.</summary>
/// <param name="Format">Format id, such as <c>jcamp-dx</c>.</param>
/// <param name="Reader">Reader name.</param>
/// <param name="Confidence"><c>definite</c>, <c>likely</c> or <c>possible</c>.</param>
/// <param name="Reason">Why the reader matched.</param>
public sealed record FormatProbe(string Format, string Reader, string Confidence, string Reason);

internal static class JsonHelpers
{
    internal static string? Text(JsonNode node, string field) =>
        node[field] is JsonValue value ? value.ToString() : null;

    /// <summary>Numbers of a JSON array; null entries (missing values) become NaN.</summary>
    internal static double[] Doubles(JsonNode? array) =>
        array is JsonArray list ? list.Select(v => v is null ? double.NaN : v.GetValue<double>()).ToArray() : Array.Empty<double>();

    internal static IReadOnlyList<string> Strings(JsonNode? array) =>
        array is JsonArray list ? list.Select(v => v?.ToString() ?? "").ToList() : new List<string>();
}
