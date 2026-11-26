package org.nirs4all.formats;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class Nirs4allFormatsTest {
    private static final Path SAMPLES = Path.of(System.getProperty("nirs4all.formats.samples", "../../samples"));
    private static final Path JCAMP = SAMPLES.resolve("jcamp_dx/TESTSPEC.DX");
    private static final Path CSV = SAMPLES.resolve("csv_tsv/synthetic_nirs.csv");
    private static final Path SPC = SAMPLES.resolve("galactic_spc/nir.spc");

    @Test
    void reportsVersionsAndReaders() {
        assertEquals("0.2.0", Nirs4allFormats.abiVersion());
        assertTrue(Nirs4allFormats.version().matches("\\d+\\.\\d+\\.\\d+.*"));
        assertTrue(Nirs4allFormats.readers().contains("nirs4all_formats::readers::jcamp"));
    }

    @Test
    void probesPathsAndBytes() throws IOException {
        assertEquals("jcamp-dx", Nirs4allFormats.probe(JCAMP).get(0).format());
        byte[] bytes = Files.readAllBytes(JCAMP);
        assertEquals("jcamp-dx", Nirs4allFormats.probe("TESTSPEC.DX", bytes).get(0).format());
    }

    @Test
    void opensJcampWithTypedSignalAndProvenance() throws IOException {
        List<SpectralRecord> records = Nirs4allFormats.open(JCAMP);
        assertEquals(1, records.size());
        SpectralRecord record = records.get(0);
        Signal signal = record.signal("signal");
        assertTrue(signal.isOneDimensional());
        assertEquals(signal.axis().size(), signal.values().length);
        assertEquals("frequency", signal.axis().kind());
        assertEquals("jcamp-dx", record.provenance().format());
        assertEquals(64, record.provenance().sources().get(0).sha256().length());

        List<SpectralRecord> fromBytes = Nirs4allFormats.open("TESTSPEC.DX", Files.readAllBytes(JCAMP));
        assertArrayEquals(signal.values(), fromBytes.get(0).signal("signal").values());
    }

    @Test
    void stacksCsvSpectraWithTargetsAndMetadata() {
        List<SpectralRecord> records = Nirs4allFormats.open(CSV);
        SignalMatrix matrix = SignalMatrix.stack(records);
        assertEquals(records.size(), matrix.rows());
        assertEquals(200, matrix.columns());
        assertEquals("nm", matrix.axis().unit());
        assertFalse(Double.isNaN(records.get(0).numericTarget("protein")));
        assertEquals("S000", records.get(0).metadata().get("sample_id").asText());
    }

    @Test
    void refusesToStackDifferentAxes() {
        List<SpectralRecord> mixed = List.of(Nirs4allFormats.open(CSV).get(0), Nirs4allFormats.open(SPC).get(0));
        String name = mixed.get(1).signals().keySet().iterator().next();
        assertThrows(IllegalArgumentException.class, () -> SignalMatrix.stack(mixed, name));
    }

    @Test
    void roundTripsThroughJcampDx() {
        List<SpectralRecord> records = Nirs4allFormats.open(SPC);
        WriteResult output = Nirs4allFormats.write(records, "jcamp-dx", new WriteOptions().baseName("nir"));
        assertEquals("jcamp-dx", output.format());
        assertEquals(records.size(), output.files().size());
        WriteResult.File file = output.files().get(0);
        Signal original = records.get(0).signals().values().iterator().next();
        Signal back = Nirs4allFormats.open(file.name(), file.bytes()).get(0).signals().values().iterator().next();
        assertEquals(original.values().length, back.values().length);
        double scale = 0;
        for (double value : original.values()) {
            scale = Math.max(scale, Math.abs(value));
        }
        for (int i = 0; i < original.values().length; i++) {
            assertEquals(original.values()[i], back.values()[i], scale * 1e-6);
            assertEquals(original.axis().values()[i], back.axis().values()[i], 1e-6);
        }
    }

    @Test
    void roundTripsThroughCsvWithTargets() {
        List<SpectralRecord> records = Nirs4allFormats.open(CSV);
        WriteResult output = Nirs4allFormats.write(records, "csv", new WriteOptions().signal("signal"));
        assertEquals(1, output.files().size());
        WriteResult.File file = output.files().get(0);
        assertEquals("text/csv", file.mediaType());
        List<SpectralRecord> back = Nirs4allFormats.open(file.name(), file.bytes());
        assertEquals(records.size(), back.size());
        SignalMatrix a = SignalMatrix.stack(records);
        SignalMatrix b = SignalMatrix.stack(back);
        assertArrayEquals(a.axis().values(), b.axis().values());
        for (int i = 0; i < a.rows(); i++) {
            assertArrayEquals(a.values()[i], b.values()[i]);
            assertEquals(records.get(i).numericTarget("protein"), back.get(i).numericTarget("protein"));
        }
    }

    @Test
    void convertsIntoDirectory(@TempDir Path out) throws IOException {
        ConvertResult result = Nirs4allFormats.convert(JCAMP, "opus", out);
        assertEquals("jcamp-dx", result.format());
        Path saved = result.files().get(0);
        assertTrue(Files.exists(saved));
        assertEquals(1, Nirs4allFormats.open(saved).size());
        FormatsException exists = assertThrows(FormatsException.class, () -> Nirs4allFormats.convert(JCAMP, "opus", out));
        assertEquals(FormatsException.Status.IO, exists.status());
        Nirs4allFormats.convert(JCAMP, "opus", out, new WriteOptions().overwrite(true));
    }

    @Test
    void listsExportTargets() {
        ExportTargets targets = Nirs4allFormats.exportTargets();
        assertTrue(targets.formats().stream().anyMatch(f -> f.format().equals("jcamp-dx")));
        assertTrue(targets.software().stream().anyMatch(s -> s.id().equals("opus") && !s.tested()));
        assertEquals(targets.formats().size(), targets.requirements().size());
        assertTrue(targets.requirements().get("foss-winisi-nir").has("signal_types"));
    }

    @Test
    void plansConversions() {
        ConversionPlan ready = Nirs4allFormats.planConversion(
                Nirs4allFormats.open(SAMPLES.resolve("ocean_optics/whiteref.ProcSpec")), "winisi");
        assertEquals("ready", ready.status());
        assertEquals("foss-winisi-nir", ready.format());
        assertFalse(ready.steps().isEmpty());
        assertEquals("preprocessing.signal_conversion.to_absorbance", ready.steps().get(0).methodId());

        List<SpectralRecord> specfile = Nirs4allFormats.open(SAMPLES.resolve("jcamp_dx/SPECFILE.DX"));
        ConversionPlan asks = Nirs4allFormats.planConversion(specfile, "winisi");
        assertEquals("needs_answers", asks.status());
        assertEquals("source_scale", asks.questions().get(0).id());
        assertFalse(asks.questions().get(0).choices().isEmpty());

        ConversionPlan refused = Nirs4allFormats.planConversion(
                Nirs4allFormats.open(SAMPLES.resolve("raman_horiba/jobinyvon_test_spec_3s_cm-1.xml")),
                null,
                new PlanOptions().axisUnit("nm"));
        assertEquals("refused", refused.status());
        assertEquals("raman_shift", refused.refusals().get(0).code());

        FormatsException unknown =
                assertThrows(FormatsException.class, () -> Nirs4allFormats.planConversion(specfile, "no-such-target"));
        assertEquals(FormatsException.Status.UNKNOWN_TARGET, unknown.status());
    }

    @Test
    void reportsFailuresWithStatus() {
        FormatsException missing =
                assertThrows(FormatsException.class, () -> Nirs4allFormats.open(Path.of("/nonexistent/x.dx")));
        assertEquals(FormatsException.Status.IO, missing.status());
        FormatsException unsupported = assertThrows(
                FormatsException.class, () -> Nirs4allFormats.open("mystery.bin", new byte[] {1, 2, 3}));
        assertEquals(FormatsException.Status.UNSUPPORTED_FORMAT, unsupported.status());
        List<SpectralRecord> records = Nirs4allFormats.open(JCAMP);
        FormatsException target =
                assertThrows(FormatsException.class, () -> Nirs4allFormats.write(records, "no-such-target"));
        assertEquals(FormatsException.Status.UNKNOWN_TARGET, target.status());
        assertTrue(target.getMessage().contains("no-such-target"));
    }
}
