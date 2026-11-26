using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json.Nodes;

namespace Nirs4all.Formats;

/// <summary>Information the target does not carry, or carries approximately.</summary>
/// <param name="Code">Stable loss identifier.</param>
/// <param name="Message">Description.</param>
/// <param name="Count">Number of spectra or records concerned.</param>
/// <param name="MaxError">Largest absolute error when the loss is numerical, else null.</param>
public sealed record WriteLoss(string Code, string Message, long Count, double? MaxError)
{
    internal static IReadOnlyList<WriteLoss> ListFromJson(JsonNode? array) =>
        (array?.AsArray() ?? new JsonArray())
            .Select(l => new WriteLoss(
                JsonHelpers.Text(l!, "code") ?? "",
                JsonHelpers.Text(l!, "message") ?? "",
                l!["count"]?.GetValue<long>() ?? 0,
                l["max_error"]?.GetValue<double>()))
            .ToList();
}

/// <summary>One produced file.</summary>
/// <param name="Name">File name.</param>
/// <param name="MediaType">Media type.</param>
/// <param name="Bytes">File content.</param>
public sealed record WrittenFile(string Name, string MediaType, byte[] Bytes)
{
    /// <summary>Save into <paramref name="directory"/>, refusing to replace an existing file.</summary>
    public string Save(string directory)
    {
        Directory.CreateDirectory(directory);
        string path = Path.Combine(directory, Name);
        using var stream = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
        stream.Write(Bytes);
        return path;
    }
}

/// <summary>Files produced by an export, in memory, and what the target could not carry.</summary>
/// <param name="Format">Writer that produced the files.</param>
/// <param name="Files">Produced files.</param>
/// <param name="Losses">Declared losses.</param>
/// <param name="Conversions">Conversion steps recorded in the records' metadata, as JSON.</param>
public sealed record WriteResult(
    string Format,
    IReadOnlyList<WrittenFile> Files,
    IReadOnlyList<WriteLoss> Losses,
    IReadOnlyList<JsonNode> Conversions)
{
    internal static IReadOnlyList<JsonNode> Nodes(JsonNode? array) =>
        (array?.AsArray() ?? new JsonArray()).Select(n => n!.DeepClone()).ToList();

    internal static WriteResult FromJson(JsonNode node) => new(
        JsonHelpers.Text(node, "format") ?? "",
        node["files"]!.AsArray()
            .Select(f => new WrittenFile(
                JsonHelpers.Text(f!, "name") ?? "",
                JsonHelpers.Text(f!, "media_type") ?? "",
                Convert.FromBase64String(JsonHelpers.Text(f!, "bytes_base64") ?? "")))
            .ToList(),
        WriteLoss.ListFromJson(node["losses"]),
        Nodes(node["conversions"]));

    /// <summary>Save every file into <paramref name="directory"/>, refusing to replace existing files.</summary>
    public IReadOnlyList<string> Save(string directory)
    {
        foreach (var file in Files)
        {
            string path = Path.Combine(directory, file.Name);
            if (File.Exists(path))
            {
                throw new IOException($"file already exists: {path}");
            }
        }
        return Files.Select(file => file.Save(directory)).ToList();
    }
}

/// <summary>Files saved by <see cref="Nirs4allFormats.Convert"/> and the declared losses.</summary>
/// <param name="Format">Writer that produced the files.</param>
/// <param name="Files">Saved paths.</param>
/// <param name="Losses">Declared losses.</param>
/// <param name="Conversions">Conversion steps recorded in the records' metadata, as JSON.</param>
public sealed record ConvertResult(
    string Format,
    IReadOnlyList<string> Files,
    IReadOnlyList<WriteLoss> Losses,
    IReadOnlyList<JsonNode> Conversions)
{
    internal static ConvertResult FromJson(JsonNode node) => new(
        JsonHelpers.Text(node, "format") ?? "",
        JsonHelpers.Strings(node["files"]),
        WriteLoss.ListFromJson(node["losses"]),
        WriteResult.Nodes(node["conversions"]));
}

/// <summary>An export format.</summary>
/// <param name="Format">Target id, such as <c>jcamp-dx</c>.</param>
/// <param name="Label">Display name.</param>
/// <param name="Extension">Default extension, without the dot.</param>
/// <param name="MediaType">Media type of the files.</param>
/// <param name="MultiSpectrum">Whether one file can hold several spectra.</param>
/// <param name="Notes">Layout notes.</param>
public sealed record ExportFormat(string Format, string Label, string Extension, string MediaType, bool MultiSpectrum, string Notes);

/// <summary>A software preset: a format the software documents as importable.</summary>
/// <param name="Id">Target id, such as <c>opus</c>.</param>
/// <param name="Label">Software name.</param>
/// <param name="Format">Format written for it.</param>
/// <param name="Extension">Extension written for it.</param>
/// <param name="Alternatives">Other formats the software documents.</param>
/// <param name="Evidence">Where the import capability is described.</param>
/// <param name="Tested">Whether an import of our files was verified in the software.</param>
/// <param name="Notes">Notes.</param>
public sealed record SoftwarePreset(
    string Id,
    string Label,
    string Format,
    string Extension,
    IReadOnlyList<string> Alternatives,
    string Evidence,
    bool Tested,
    string Notes);

/// <summary>Export formats and software presets accepted as write targets.</summary>
/// <param name="Formats">Export formats.</param>
/// <param name="Software">Software presets.</param>
/// <param name="Requirements">What each format accepts (signal types, axis units, grids, missing values, ...), as JSON keyed by format id.</param>
public sealed record ExportTargets(
    IReadOnlyList<ExportFormat> Formats,
    IReadOnlyList<SoftwarePreset> Software,
    IReadOnlyDictionary<string, JsonObject> Requirements)
{
    internal static ExportTargets FromJson(JsonNode node) => new(
        node["formats"]!.AsArray()
            .Select(f => new ExportFormat(
                JsonHelpers.Text(f!, "format") ?? "",
                JsonHelpers.Text(f!, "label") ?? "",
                JsonHelpers.Text(f!, "extension") ?? "",
                JsonHelpers.Text(f!, "media_type") ?? "",
                f!["multi_spectrum"]?.GetValue<bool>() ?? false,
                JsonHelpers.Text(f, "notes") ?? ""))
            .ToList(),
        node["software"]!.AsArray()
            .Select(s => new SoftwarePreset(
                JsonHelpers.Text(s!, "id") ?? "",
                JsonHelpers.Text(s!, "label") ?? "",
                JsonHelpers.Text(s!, "format") ?? "",
                JsonHelpers.Text(s!, "extension") ?? "",
                JsonHelpers.Strings(s!["alternatives"]),
                JsonHelpers.Text(s, "evidence") ?? "",
                s["tested"]?.GetValue<bool>() ?? false,
                JsonHelpers.Text(s, "notes") ?? ""))
            .ToList(),
        (node["requirements"]?.AsArray() ?? new JsonArray())
            .ToDictionary(r => JsonHelpers.Text(r!, "format") ?? "", r => r!.DeepClone().AsObject()));
}
