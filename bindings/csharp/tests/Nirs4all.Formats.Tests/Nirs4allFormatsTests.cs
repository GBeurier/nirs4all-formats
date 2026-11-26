using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Xunit;

namespace Nirs4all.Formats.Tests;

public sealed class Nirs4allFormatsTests : IDisposable
{
    private static readonly string Samples = FindSamples();
    private static readonly string Jcamp = Path.Combine(Samples, "jcamp_dx", "TESTSPEC.DX");
    private static readonly string Csv = Path.Combine(Samples, "csv_tsv", "synthetic_nirs.csv");
    private static readonly string Spc = Path.Combine(Samples, "galactic_spc", "nir.spc");

    private readonly string _tempDir = Directory.CreateTempSubdirectory("n4fmt-cs-").FullName;

    public void Dispose() => Directory.Delete(_tempDir, recursive: true);

    /// <summary>NIRS4ALL_FORMATS_SAMPLES, or the repository samples/ above the test binary.</summary>
    private static string FindSamples()
    {
        string? configured = Environment.GetEnvironmentVariable("NIRS4ALL_FORMATS_SAMPLES");
        if (!string.IsNullOrEmpty(configured))
        {
            return configured;
        }
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            string candidate = Path.Combine(dir.FullName, "samples", "jcamp_dx");
            if (Directory.Exists(candidate))
            {
                return Path.Combine(dir.FullName, "samples");
            }
        }
        throw new DirectoryNotFoundException("repository samples/ not found; set NIRS4ALL_FORMATS_SAMPLES");
    }

    [Fact]
    public void ReportsVersionsAndReaders()
    {
        Assert.Equal("0.2.0", Nirs4allFormats.AbiVersion);
        Assert.Matches(@"^\d+\.\d+\.\d+", Nirs4allFormats.Version);
        Assert.Contains("nirs4all_formats::readers::jcamp", Nirs4allFormats.Readers());
    }

    [Fact]
    public void ProbesPathsAndBytes()
    {
        Assert.Equal("jcamp-dx", Nirs4allFormats.Probe(Jcamp)[0].Format);
        Assert.Equal("jcamp-dx", Nirs4allFormats.Probe("TESTSPEC.DX", File.ReadAllBytes(Jcamp))[0].Format);
    }

    [Fact]
    public void OpensJcampWithTypedSignalAndProvenance()
    {
        var records = Nirs4allFormats.Open(Jcamp);
        var record = Assert.Single(records);
        Signal signal = record.Signals["signal"];
        Assert.True(signal.IsOneDimensional);
        Assert.Equal(signal.Axis.Count, signal.Values.Length);
        Assert.Equal("frequency", signal.Axis.Kind);
        Assert.Equal("jcamp-dx", record.Provenance.Format);
        Assert.Equal(64, record.Provenance.Sources[0].Sha256.Length);

        var fromBytes = Nirs4allFormats.Open("TESTSPEC.DX", File.ReadAllBytes(Jcamp));
        Assert.Equal(signal.Values, fromBytes[0].Signals["signal"].Values);
    }

    [Fact]
    public void StacksCsvSpectraWithTargetsAndMetadata()
    {
        var records = Nirs4allFormats.Open(Csv);
        var matrix = SignalMatrix.Stack(records);
        Assert.Equal(records.Count, matrix.Rows);
        Assert.Equal(200, matrix.Columns);
        Assert.Equal("nm", matrix.Axis.Unit);
        Assert.False(double.IsNaN(records[0].NumericTarget("protein")));
        Assert.Equal("S000", records[0].Metadata["sample_id"]!.ToString());
    }

    [Fact]
    public void RefusesToStackDifferentAxes()
    {
        var mixed = new List<SpectralRecord> { Nirs4allFormats.Open(Csv)[0], Nirs4allFormats.Open(Spc)[0] };
        string name = mixed[1].Signals.Keys.First();
        Assert.Throws<ArgumentException>(() => SignalMatrix.Stack(mixed, name));
    }

    [Fact]
    public void RoundTripsThroughJcampDx()
    {
        var records = Nirs4allFormats.Open(Spc);
        var output = Nirs4allFormats.Write(records, "jcamp-dx", new WriteOptions { BaseName = "nir" });
        Assert.Equal("jcamp-dx", output.Format);
        Assert.Equal(records.Count, output.Files.Count);
        var file = output.Files[0];
        Signal original = records[0].Signals.Values.First();
        Signal back = Nirs4allFormats.Open(file.Name, file.Bytes)[0].Signals.Values.First();
        Assert.Equal(original.Values.Length, back.Values.Length);
        double scale = original.Values.Max(Math.Abs);
        for (int i = 0; i < original.Values.Length; i++)
        {
            Assert.InRange(Math.Abs(original.Values[i] - back.Values[i]), 0, scale * 1e-6);
            Assert.InRange(Math.Abs(original.Axis.Values[i] - back.Axis.Values[i]), 0, 1e-6);
        }
    }

    [Fact]
    public void RoundTripsThroughCsvWithTargets()
    {
        var records = Nirs4allFormats.Open(Csv);
        var output = Nirs4allFormats.Write(records, "csv", new WriteOptions { Signal = "signal" });
        var file = Assert.Single(output.Files);
        Assert.Equal("text/csv", file.MediaType);
        var back = Nirs4allFormats.Open(file.Name, file.Bytes);
        Assert.Equal(records.Count, back.Count);
        var a = SignalMatrix.Stack(records);
        var b = SignalMatrix.Stack(back);
        Assert.Equal(a.Axis.Values, b.Axis.Values);
        Assert.Equal(a.Values, b.Values);
        for (int i = 0; i < records.Count; i++)
        {
            Assert.Equal(records[i].NumericTarget("protein"), back[i].NumericTarget("protein"));
        }
    }

    [Fact]
    public void ConvertsIntoDirectory()
    {
        var result = Nirs4allFormats.Convert(Jcamp, "opus", _tempDir);
        Assert.Equal("jcamp-dx", result.Format);
        Assert.True(File.Exists(result.Files[0]));
        Assert.Single(Nirs4allFormats.Open(result.Files[0]));
        var exists = Assert.Throws<FormatsException>(() => Nirs4allFormats.Convert(Jcamp, "opus", _tempDir));
        Assert.Equal(FormatsStatus.Io, exists.Status);
        Nirs4allFormats.Convert(Jcamp, "opus", _tempDir, new WriteOptions { Overwrite = true });
    }

    [Fact]
    public void ListsExportTargets()
    {
        var targets = Nirs4allFormats.ExportTargets();
        Assert.Contains(targets.Formats, f => f.Format == "jcamp-dx");
        Assert.Contains(targets.Software, s => s.Id == "opus" && !s.Tested);
        Assert.Equal(targets.Formats.Count, targets.Requirements.Count);
        Assert.True(targets.Requirements["foss-winisi-nir"].ContainsKey("signal_types"));
    }

    [Fact]
    public void PlansConversions()
    {
        var ready = Nirs4allFormats.PlanConversion(
            Nirs4allFormats.Open(Path.Combine(Samples, "ocean_optics", "whiteref.ProcSpec")), "winisi");
        Assert.Equal("ready", ready.Status);
        Assert.Equal("foss-winisi-nir", ready.Format);
        Assert.Equal("preprocessing.signal_conversion.to_absorbance", ready.Steps[0].MethodId);

        var specfile = Nirs4allFormats.Open(Path.Combine(Samples, "jcamp_dx", "SPECFILE.DX"));
        var asks = Nirs4allFormats.PlanConversion(specfile, "winisi");
        Assert.Equal("needs_answers", asks.Status);
        Assert.Equal("source_scale", asks.Questions[0].Id);
        Assert.NotEmpty(asks.Questions[0].Choices);

        var refused = Nirs4allFormats.PlanConversion(
            Nirs4allFormats.Open(Path.Combine(Samples, "raman_horiba", "jobinyvon_test_spec_3s_cm-1.xml")),
            null,
            new PlanOptions { AxisUnit = "nm" });
        Assert.Equal("refused", refused.Status);
        Assert.Equal("raman_shift", refused.Refusals[0].Code);

        var unknown = Assert.Throws<FormatsException>(() => Nirs4allFormats.PlanConversion(specfile, "no-such-target"));
        Assert.Equal(FormatsStatus.UnknownTarget, unknown.Status);
    }

    [Fact]
    public void ReportsFailuresWithStatus()
    {
        var missing = Assert.Throws<FormatsException>(() => Nirs4allFormats.Open("/nonexistent/x.dx"));
        Assert.Equal(FormatsStatus.Io, missing.Status);
        var unsupported = Assert.Throws<FormatsException>(() => Nirs4allFormats.Open("mystery.bin", new byte[] { 1, 2, 3 }));
        Assert.Equal(FormatsStatus.UnsupportedFormat, unsupported.Status);
        var records = Nirs4allFormats.Open(Jcamp);
        var target = Assert.Throws<FormatsException>(() => Nirs4allFormats.Write(records, "no-such-target"));
        Assert.Equal(FormatsStatus.UnknownTarget, target.Status);
        Assert.Contains("no-such-target", target.Message);
    }
}
