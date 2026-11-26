# C#

`Nirs4all.Formats` is a .NET 8 library that reads every supported file into
typed records and exports them, through the [C ABI](capi.md) with P/Invoke and
`System.Text.Json`. Parsing and writing happen in the Rust library.

## Install

From NuGet.org, once published (the package carries the native library under
`runtimes/<rid>/native/` for `linux-x64`, `linux-arm64`, `osx-x64`, `osx-arm64`
and `win-x64`):

```bash
dotnet add package Nirs4all.Formats --version 0.2.20
```

From source:

```bash
cargo build --release -p nirs4all-formats-capi
export NIRS4ALL_FORMATS_LIBRARY=$PWD/target/release/libnirs4all_formats_capi.so
dotnet test bindings/csharp/tests/Nirs4all.Formats.Tests
dotnet pack bindings/csharp/src/Nirs4all.Formats -c Release -o nupkg -p:NativeAssetsDir=DIR
```

`NativeAssetsDir` holds `<rid>/native/<library>` and is packed under `runtimes/`.

## Native library

When `NIRS4ALL_FORMATS_LIBRARY` names a file, that file is loaded. Otherwise
.NET probes the package's `runtimes/<rid>/native/` folder, the application
directory and the system library path for `nirs4all_formats_capi`.

## Read and access data

```csharp
using Nirs4all.Formats;

var records = Nirs4allFormats.Open("set.csv");
var first = records[0];
Signal signal = first.Signals["signal"];          // each signal owns its axis
double[] x = signal.Axis.Values;                   // signal.Axis.Unit, .Kind
double[] y = signal.Values;                        // NaN for missing points
double protein = first.NumericTarget("protein");   // NaN when missing
string? sample = first.Metadata["sample_id"]?.ToString();
string sha256 = first.Provenance.Sources[0].Sha256;

var m = SignalMatrix.Stack(records, "signal");     // axes must be identical
double[,] X = m.Values;                            // one row per record
```

`Nirs4allFormats.Open(name, bytes)` reads in-memory content; `ReadOptions`
selects cube windows, pixels or `SingleRecord`. `Nirs4allFormats.Probe(path)`
lists the matching readers and `Nirs4allFormats.Readers()` the compiled ones.

## Export and convert

```csharp
var targets = Nirs4allFormats.ExportTargets();     // Formats + Software presets

var output = Nirs4allFormats.Write(records, "jcamp-dx", new WriteOptions { BaseName = "set" });
foreach (var loss in output.Losses)
{
    Console.WriteLine($"{loss.Code}: {loss.Message}");
}
output.Save("exported");                           // refuses to replace files

var done = Nirs4allFormats.Convert("spectrum.spc", "opus", "exported");
```

Writers never resample or convert units; what a target cannot carry is
refused or listed in `Losses` (see [Exports](../EXPORTS.md)). Failures throw
`FormatsException`, whose `Status` is the [C ABI status](capi.md#errors).

## Publishing

`.github/workflows/release-bindings.yml` builds the native library for the five
runtimes, tests the package against the Linux library, packs
`Nirs4all.Formats.<version>.nupkg` and, on a `v<version>` tag, pushes it to
NuGet.org. Without credentials it skips with a notice.

One-time setup by the maintainer:

1. Sign in to <https://www.nuget.org> and check that the id `Nirs4all.Formats`
   is free (optionally reserve the `Nirs4all.` prefix).
2. Preferred, trusted publishing: in nuget.org, Trusted Publishing, add a
   policy for owner `GBeurier`, repository `nirs4all-formats` and workflow file
   `release-bindings.yml`. Then set the repository variable `NUGET_USER` to the
   nuget.org account name (Settings, Secrets and variables, Actions,
   Variables).
3. Fallback: create an API key scoped to push `Nirs4all.Formats` and store it
   as the repository secret `NUGET_API_KEY`.
