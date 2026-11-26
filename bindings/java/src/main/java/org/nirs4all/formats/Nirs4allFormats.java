package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Read spectroscopy files and export records through the nirs4all-formats native library.
 *
 * <p>The native library {@code nirs4all_formats_capi} is located, in order, by the system
 * property {@code nirs4all.formats.library} (a file path), the environment variable {@code
 * NIRS4ALL_FORMATS_LIBRARY} (a file path), then JNA's search: {@code jna.library.path}, the
 * library bundled in the jar under {@code <os>-<arch>/}, and the system library path.
 *
 * <p>Every method throws {@link FormatsException} when the native call fails.
 */
public final class Nirs4allFormats {
    private Nirs4allFormats() {}

    /** Version of the native {@code nirs4all-formats} library. */
    public static String version() {
        NativeLibrary.Capi capi = NativeLibrary.capi();
        return NativeLibrary.take(capi, capi.n4fmt_library_version());
    }

    /** Version of the C ABI. */
    public static String abiVersion() {
        NativeLibrary.Capi capi = NativeLibrary.capi();
        return NativeLibrary.take(capi, capi.n4fmt_abi_version());
    }

    /** Readers compiled into the native library. */
    public static List<String> readers() {
        JsonNode list = Json.parse(NativeLibrary.call(NativeLibrary.Capi::n4fmt_reader_catalog));
        List<String> out = new ArrayList<>();
        list.forEach(entry -> out.add(Json.text(entry, "reader")));
        return Collections.unmodifiableList(out);
    }

    /** Readers that recognize the file, most confident first. */
    public static List<FormatProbe> probe(Path path) {
        String text = path.toString();
        return probes(NativeLibrary.call((capi, out, err) -> capi.n4fmt_probe_path(text, out, err)));
    }

    /** Readers that recognize in-memory content named {@code name}, most confident first. */
    public static List<FormatProbe> probe(String name, byte[] bytes) {
        return probes(NativeLibrary.call((capi, out, err) ->
                capi.n4fmt_probe_bytes(name, bytes, new NativeLibrary.SizeT(bytes.length), out, err)));
    }

    /** Read a file. Companion files of sidecar formats are found next to it. */
    public static List<SpectralRecord> open(Path path) {
        return open(path, new ReadOptions());
    }

    /** Read a file with read options. */
    public static List<SpectralRecord> open(Path path, ReadOptions options) {
        String text = path.toString();
        String json = options.toJson();
        return records(NativeLibrary.call((capi, out, err) -> capi.n4fmt_open_path(text, json, out, err)));
    }

    /**
     * Read in-memory content. {@code name} is the file name, used for sniffing and provenance.
     * Formats that need a companion file fail with {@link FormatsException.Status#UNSUPPORTED_SIDECAR}.
     */
    public static List<SpectralRecord> open(String name, byte[] bytes) {
        return open(name, bytes, new ReadOptions());
    }

    /** Read in-memory content with read options. */
    public static List<SpectralRecord> open(String name, byte[] bytes, ReadOptions options) {
        String json = options.toJson();
        return records(NativeLibrary.call((capi, out, err) ->
                capi.n4fmt_open_bytes(name, bytes, new NativeLibrary.SizeT(bytes.length), json, out, err)));
    }

    /** Export formats, software presets and the requirements of each format. */
    public static ExportTargets exportTargets() {
        return ExportTargets.fromJson(Json.parse(NativeLibrary.call(NativeLibrary.Capi::n4fmt_export_targets)));
    }

    /** Write records to an export format or software preset, in memory. */
    public static WriteResult write(List<SpectralRecord> records, String target) {
        return write(records, target, new WriteOptions());
    }

    /** Write records to an export format or software preset with options, in memory. */
    public static WriteResult write(List<SpectralRecord> records, String target, WriteOptions options) {
        String recordsJson = recordsJson(records);
        String optionsJson = options.toJson(false);
        return WriteResult.fromJson(Json.parse(NativeLibrary.call((capi, out, err) ->
                capi.n4fmt_write_records(recordsJson, target, optionsJson, out, err))));
    }

    /**
     * Plan the conversions that make {@code records} writable for {@code target} (an export
     * format or software preset), or for the options alone when {@code target} is {@code null}.
     *
     * <p>The plan computes no values: each numerical step names the nirs4all-methods method that
     * computes it. Run those methods yourself, or export records that are already converted.
     * Unknown targets and invalid options throw; what the records cannot become is in {@link
     * ConversionPlan#refusals()}.
     */
    public static ConversionPlan planConversion(List<SpectralRecord> records, String target, PlanOptions options) {
        String recordsJson = recordsJson(records);
        String optionsJson = options.toJson();
        return new ConversionPlan((ObjectNode) Json.parse(NativeLibrary.call((capi, out, err) ->
                capi.n4fmt_plan_conversion(recordsJson, target, optionsJson, out, err))));
    }

    /** Plan with default options; see {@link #planConversion(List, String, PlanOptions)}. */
    public static ConversionPlan planConversion(List<SpectralRecord> records, String target) {
        return planConversion(records, target, new PlanOptions());
    }

    /** Read {@code input} and save its export into the existing directory {@code outputDir}. */
    public static ConvertResult convert(Path input, String target, Path outputDir) {
        return convert(input, target, outputDir, new WriteOptions());
    }

    /**
     * Read {@code input} and save its export into the existing directory {@code outputDir}.
     * Existing files are kept, and nothing is written, unless {@link WriteOptions#overwrite} is set.
     */
    public static ConvertResult convert(Path input, String target, Path outputDir, WriteOptions options) {
        String in = input.toString();
        String dir = outputDir.toString();
        String optionsJson = options.toJson(true);
        return ConvertResult.fromJson(Json.parse(NativeLibrary.call((capi, out, err) ->
                capi.n4fmt_convert_path(in, target, dir, optionsJson, out, err))));
    }

    private static String recordsJson(List<SpectralRecord> records) {
        ArrayNode array = Json.MAPPER.createArrayNode();
        for (SpectralRecord record : records) {
            array.add(record.rawJson());
        }
        return Json.write(array);
    }

    private static List<SpectralRecord> records(String json) {
        JsonNode array = Json.parse(json);
        List<SpectralRecord> out = new ArrayList<>(array.size());
        array.forEach(node -> out.add(new SpectralRecord((ObjectNode) node)));
        return Collections.unmodifiableList(out);
    }

    private static List<FormatProbe> probes(String json) {
        List<FormatProbe> out = new ArrayList<>();
        Json.parse(json).forEach(node -> out.add(new FormatProbe(
                Json.text(node, "format"),
                Json.text(node, "reader"),
                Json.text(node, "confidence"),
                Json.text(node, "reason"))));
        return Collections.unmodifiableList(out);
    }
}
