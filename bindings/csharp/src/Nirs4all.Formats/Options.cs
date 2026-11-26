using System.Collections.Generic;
using System.Text.Json.Nodes;

namespace Nirs4all.Formats;

/// <summary>
/// Read controls for image cubes. Defaults read every record. A rectangular window
/// (<see cref="Rows"/>, <see cref="Cols"/>) and a sparse <see cref="Pixels"/> mask
/// cannot be combined.
/// </summary>
public sealed class ReadOptions
{
    /// <summary>Emit an image cube as one N-dimensional record instead of one record per pixel.</summary>
    public bool SingleRecord { get; init; }

    /// <summary>Half-open row window; a null end means "to the last row".</summary>
    public (int Start, int? End)? Rows { get; init; }

    /// <summary>Half-open column window; a null end means "to the last column".</summary>
    public (int Start, int? End)? Cols { get; init; }

    /// <summary>Sparse pixel mask as (row, col) pairs.</summary>
    public IReadOnlyList<(int Row, int Col)>? Pixels { get; init; }

    internal string ToJson()
    {
        var node = new JsonObject();
        if (SingleRecord)
        {
            node["single_record"] = true;
        }
        if (Rows is { } rows)
        {
            node["rows"] = new JsonArray(rows.Start, rows.End);
        }
        if (Cols is { } cols)
        {
            node["cols"] = new JsonArray(cols.Start, cols.End);
        }
        if (Pixels is { } pixels)
        {
            var list = new JsonArray();
            foreach (var (row, col) in pixels)
            {
                list.Add(new JsonArray(row, col));
            }
            node["pixels"] = list;
        }
        return node.ToJsonString();
    }
}

/// <summary>Export controls. Defaults write every signal of every record.</summary>
public sealed class WriteOptions
{
    /// <summary>Write this signal only.</summary>
    public string? Signal { get; init; }

    /// <summary>Stem of the produced file names (default: the source file stem).</summary>
    public string? BaseName { get; init; }

    /// <summary>File extension override, without the dot.</summary>
    public string? Extension { get; init; }

    /// <summary>
    /// Largest absolute axis deviation, in axis units, accepted when an almost evenly
    /// spaced axis is written as even; the deviation is declared as a loss.
    /// </summary>
    public double? AxisTolerance { get; init; }

    /// <summary>Group spectra sharing one axis into multi-spectrum files when the target supports it.</summary>
    public bool Multi { get; init; }

    /// <summary>Column separator of delimited targets: <c>','</c>, <c>';'</c> or <c>'\t'</c>.</summary>
    public char? Delimiter { get; init; }

    /// <summary>Return one ZIP archive instead of separate files.</summary>
    public bool Zip { get; init; }

    /// <summary>Replace existing files when saving (<see cref="Nirs4allFormats.Convert"/> only).</summary>
    public bool Overwrite { get; init; }

    internal string ToJson(bool withOverwrite)
    {
        var node = new JsonObject();
        if (Signal is not null)
        {
            node["signal"] = Signal;
        }
        if (BaseName is not null)
        {
            node["base_name"] = BaseName;
        }
        if (Extension is not null)
        {
            node["extension"] = Extension;
        }
        if (AxisTolerance is { } tolerance)
        {
            node["axis_tolerance"] = tolerance;
        }
        if (Multi)
        {
            node["multi"] = true;
        }
        if (Delimiter is { } delimiter)
        {
            node["delimiter"] = delimiter.ToString();
        }
        if (Zip)
        {
            node["zip"] = true;
        }
        if (withOverwrite && Overwrite)
        {
            node["overwrite"] = true;
        }
        return node.ToJsonString();
    }
}
