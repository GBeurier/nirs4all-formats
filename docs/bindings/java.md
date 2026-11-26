# Java

`org.nirs4all:nirs4all-formats` reads every supported file into typed records
and exports them, through the [C ABI](capi.md) with
[JNA](https://github.com/java-native-access/jna) and Jackson. It needs Java 17
or later. Parsing and writing happen in the Rust library.

## Install

From Maven Central, once published (the jar bundles the native library for
Linux x86-64/aarch64, macOS x86-64/arm64 and Windows x86-64):

```xml
<dependency>
  <groupId>org.nirs4all</groupId>
  <artifactId>nirs4all-formats</artifactId>
  <version>0.2.20</version>
</dependency>
```

From source:

```bash
cargo build --release -p nirs4all-formats-capi
cd bindings/java
NIRS4ALL_FORMATS_LIBRARY=$PWD/../../target/release/libnirs4all_formats_capi.so mvn test install
```

## Native library

The library `nirs4all_formats_capi` is found, in order, through:

1. the system property `nirs4all.formats.library` (a file path),
2. the environment variable `NIRS4ALL_FORMATS_LIBRARY` (a file path),
3. JNA's search: `jna.library.path`, a copy bundled in the jar under
   `<os>-<arch>/` (`linux-x86-64/libnirs4all_formats_capi.so`,
   `darwin-aarch64/libnirs4all_formats_capi.dylib`,
   `win32-x86-64/nirs4all_formats_capi.dll`, ...), then the system library path.

## Read and access data

```java
import org.nirs4all.formats.*;
import java.nio.file.Path;
import java.util.List;

List<SpectralRecord> records = Nirs4allFormats.open(Path.of("set.csv"));
SpectralRecord first = records.get(0);
Signal signal = first.signal("signal");          // each signal owns its axis
double[] x = signal.axis().values();              // signal.axis().unit(), .kind()
double[] y = signal.values();                     // NaN for missing points
double protein = first.numericTarget("protein");  // NaN when missing
String sample = first.metadata().path("sample_id").asText();
String sha256 = first.provenance().sources().get(0).sha256();

SignalMatrix m = SignalMatrix.stack(records, "signal");  // axes must be identical
double[][] X = m.values();                                // one row per record
```

`Nirs4allFormats.open(String name, byte[] bytes)` reads in-memory content;
`ReadOptions` selects cube windows, pixels or `singleRecord(true)`.
`Nirs4allFormats.probe(path)` lists the matching readers and
`Nirs4allFormats.readers()` the compiled ones.

## Export and convert

```java
ExportTargets targets = Nirs4allFormats.exportTargets();   // formats + software presets

WriteResult out = Nirs4allFormats.write(records, "jcamp-dx", new WriteOptions().baseName("set"));
for (WriteResult.Loss loss : out.losses()) {
    System.out.println(loss.code() + ": " + loss.message());
}
out.save(Path.of("exported"));                             // refuses to replace files

ConvertResult done = Nirs4allFormats.convert(Path.of("spectrum.spc"), "opus", Path.of("exported"));
```

Writers never resample or convert units; what a target cannot carry is
refused or listed in `losses` (see [Exports](../EXPORTS.md)). Failures throw
`FormatsException`, whose `status()` is the [C ABI status](capi.md#errors).

## Plan a conversion

```java
List<SpectralRecord> records = Nirs4allFormats.open(Path.of("SPECFILE.DX"));
ConversionPlan plan = Nirs4allFormats.planConversion(records, "winisi");
plan.status();                                     // "needs_answers"
plan.questions().get(0).id();                      // "source_scale"
plan = Nirs4allFormats.planConversion(records, "winisi", new PlanOptions().sourceScale("percent"));
for (ConversionPlan.Step step : plan.steps()) {
    System.out.println(step.methodId() + " " + step.params());
}
ExportTargets targets = Nirs4allFormats.exportTargets();
targets.requirements().get("foss-winisi-nir");     // what the format accepts, as JSON
```

The plan computes no values: each step names the nirs4all-methods catalog
method that computes it, with its parameters. Run those methods with
nirs4all-methods (for example its C ABI or Python package) and write the
converted records, or export records that are already converted. A
`needs_answers` plan lists questions; answer each through the `PlanOptions`
setter of the same name. `refused` plans explain why in `refusals()`.

## Publishing

`.github/workflows/release-bindings.yml` builds the native library for the five
platforms, bundles it under the JNA resource layout, runs the tests against the
bundled Linux library and builds the jar with sources, javadoc and GPG
signatures. On a `v<version>` tag it uploads to Maven Central through the
Sonatype Central Portal when the secrets below exist, and otherwise skips with
a notice.

One-time setup by the maintainer:

1. Create an account on <https://central.sonatype.com> and verify the namespace
   `org.nirs4all` (DNS TXT record on `nirs4all.org`). If the domain cannot be
   verified, use the automatically verified `io.github.gbeurier` namespace and
   change `groupId` in `bindings/java/pom.xml`.
2. Generate a Central Portal user token.
3. Create a GPG key, publish its public key to `keyserver.ubuntu.com`, and
   export the private key in ASCII armor.
4. In the repository settings (Secrets and variables, Actions), set
   `MAVEN_CENTRAL_USERNAME` and `MAVEN_CENTRAL_PASSWORD` (the token),
   `MAVEN_GPG_PRIVATE_KEY` and `MAVEN_GPG_PASSPHRASE`.

Local dry run without signing or upload:

```bash
mvn -B -P release -Dgpg.skip verify
```
