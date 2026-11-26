using System.Collections.Generic;
using System.Linq;
using System.Text.Json.Nodes;

namespace Nirs4all.Formats;

/// <summary>Choices and answers of a conversion plan. Defaults plan automatically.</summary>
public sealed class PlanOptions
{
    /// <summary>Signal to convert (default: the best candidate for the target).</summary>
    public string? Signal { get; init; }

    /// <summary>Wanted signal type, such as <c>absorbance</c>, for targets that accept several or none.</summary>
    public string? SignalType { get; init; }

    /// <summary>Wanted axis unit: <c>nm</c>, <c>um</c> or <c>cm-1</c>.</summary>
    public string? AxisUnit { get; init; }

    /// <summary><c>measured</c> (no resampling), <c>even</c>, or a grid id of the target.</summary>
    public string? Grid { get; init; }

    /// <summary>Step of an <c>even</c> grid, in the output axis unit.</summary>
    public double? GridStep { get; init; }

    /// <summary>Lower limit of an <c>even</c> grid, within the measured range.</summary>
    public double? GridMin { get; init; }

    /// <summary>Upper limit of an <c>even</c> grid, within the measured range.</summary>
    public double? GridMax { get; init; }

    /// <summary>Resampling method: <c>linear</c> (default), <c>nearest</c> or <c>cubic</c>.</summary>
    public string? Interpolation { get; init; }

    /// <summary>Answer to the <c>source_scale</c> question: <c>percent</c> or <c>fraction</c>.</summary>
    public string? SourceScale { get; init; }

    /// <summary>Answer to the <c>signal_type</c> question for signals of unknown type.</summary>
    public string? AssumeSignalType { get; init; }

    /// <summary>Answer to the <c>absorbance_origin</c> question: <c>reflectance</c> or <c>transmittance</c>.</summary>
    public string? AbsorbanceOrigin { get; init; }

    /// <summary>Axis deviation accepted as even, as in <see cref="WriteOptions.AxisTolerance"/>.</summary>
    public double? AxisTolerance { get; init; }

    /// <summary>First axis value of each following detector, for the splice correction.</summary>
    public IReadOnlyList<double>? Splices { get; init; }

    internal string ToJson()
    {
        var node = new JsonObject();
        void Set(string key, JsonNode? value)
        {
            if (value is not null)
            {
                node[key] = value;
            }
        }
        Set("signal", Signal);
        Set("signal_type", SignalType);
        Set("axis_unit", AxisUnit);
        Set("grid", Grid);
        Set("grid_step", GridStep);
        Set("grid_min", GridMin);
        Set("grid_max", GridMax);
        Set("interpolation", Interpolation);
        Set("source_scale", SourceScale);
        Set("assume_signal_type", AssumeSignalType);
        Set("absorbance_origin", AbsorbanceOrigin);
        Set("axis_tolerance", AxisTolerance);
        if (Splices is not null)
        {
            node["splices"] = new JsonArray(Splices.Select(v => (JsonNode?)v).ToArray());
        }
        return node.ToJsonString();
    }
}

/// <summary>One step of a conversion plan, in execution order.</summary>
/// <param name="Step"><c>signal_conversion</c>, <c>resample</c>, <c>axis_unit_conversion</c>, <c>select_points</c>, <c>declare_signal_type</c> or <c>reflectance_from_references</c>.</param>
/// <param name="MethodId">nirs4all-methods catalog method, or null for selections and declarations.</param>
/// <param name="Params">Method parameters.</param>
/// <param name="Description">What the step does.</param>
/// <param name="Notes">Limits of the step.</param>
public sealed record PlanStep(string Step, string? MethodId, JsonObject Params, string Description, IReadOnlyList<string> Notes);

/// <summary>A choice offered by a question.</summary>
/// <param name="Value">Option value to set.</param>
/// <param name="Label">Description.</param>
public sealed record PlanChoice(string Value, string Label);

/// <summary>Information the records do not hold; answer it through <see cref="PlanOptions"/> and plan again.</summary>
/// <param name="Id">Option that answers it, such as <c>source_scale</c>.</param>
/// <param name="Prompt">Question.</param>
/// <param name="Choices">Accepted answers.</param>
/// <param name="Why">Why it is asked.</param>
public sealed record PlanQuestion(string Id, string Prompt, IReadOnlyList<PlanChoice> Choices, string Why);

/// <summary>Why the records cannot be converted for the target.</summary>
/// <param name="Code">Stable identifier, such as <c>raman_shift</c>.</param>
/// <param name="Reason">Explanation.</param>
/// <param name="Fix">What would make it possible.</param>
public sealed record PlanRefusal(string Code, string Reason, string Fix);

/// <summary>
/// Steps that make records writable for a target, from <see cref="Nirs4allFormats.PlanConversion"/>.
/// The plan computes nothing: each numerical step names the nirs4all-methods catalog method that
/// computes it. Run those methods yourself, or export records that are already converted.
/// </summary>
/// <param name="Status"><c>ready</c>, <c>needs_answers</c>, <c>refused</c> or <c>unchanged</c>.</param>
/// <param name="Format">Writer format of the target, or null when planning the options alone.</param>
/// <param name="SourceSignal">Signal the steps start from.</param>
/// <param name="Steps">Steps to run, in order.</param>
/// <param name="Questions">Questions to answer through <see cref="PlanOptions"/>.</param>
/// <param name="Refusals">Reasons the conversion is refused.</param>
/// <param name="Json">The complete plan (output signal, candidates, grid, notes).</param>
public sealed record ConversionPlan(
    string Status,
    string? Format,
    string? SourceSignal,
    IReadOnlyList<PlanStep> Steps,
    IReadOnlyList<PlanQuestion> Questions,
    IReadOnlyList<PlanRefusal> Refusals,
    JsonObject Json)
{
    internal static ConversionPlan FromJson(JsonObject node) => new(
        JsonHelpers.Text(node, "status") ?? "",
        JsonHelpers.Text(node, "format"),
        JsonHelpers.Text(node, "source_signal"),
        (node["steps"]?.AsArray() ?? new JsonArray())
            .Select(s => new PlanStep(
                JsonHelpers.Text(s!, "step") ?? "",
                JsonHelpers.Text(s!, "method_id"),
                s!["params"]?.DeepClone().AsObject() ?? new JsonObject(),
                JsonHelpers.Text(s, "description") ?? "",
                JsonHelpers.Strings(s["notes"])))
            .ToList(),
        (node["questions"]?.AsArray() ?? new JsonArray())
            .Select(q => new PlanQuestion(
                JsonHelpers.Text(q!, "id") ?? "",
                JsonHelpers.Text(q!, "prompt") ?? "",
                (q!["choices"]?.AsArray() ?? new JsonArray())
                    .Select(c => new PlanChoice(JsonHelpers.Text(c!, "value") ?? "", JsonHelpers.Text(c!, "label") ?? ""))
                    .ToList(),
                JsonHelpers.Text(q, "why") ?? ""))
            .ToList(),
        (node["refusals"]?.AsArray() ?? new JsonArray())
            .Select(r => new PlanRefusal(
                JsonHelpers.Text(r!, "code") ?? "",
                JsonHelpers.Text(r!, "reason") ?? "",
                JsonHelpers.Text(r!, "fix") ?? ""))
            .ToList(),
        node);
}
