package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Export formats and software presets accepted as write targets.
 *
 * @param formats export formats
 * @param software software presets, each naming a format the software documents as importable
 * @param requirements what each format accepts (signal types, axis units, grids, missing values,
 *     ...), as JSON keyed by format id
 */
public record ExportTargets(List<Format> formats, List<Software> software, Map<String, JsonNode> requirements) {

    /**
     * An export format.
     *
     * @param format target id, such as {@code jcamp-dx}
     * @param label display name
     * @param extension default extension, without the dot
     * @param mediaType media type of the files
     * @param multiSpectrum whether one file can hold several spectra
     * @param notes layout notes
     */
    public record Format(
            String format, String label, String extension, String mediaType, boolean multiSpectrum, String notes) {}

    /**
     * A software preset.
     *
     * @param id target id, such as {@code opus}
     * @param label software name
     * @param format format written for it
     * @param extension extension written for it
     * @param alternatives other formats the software documents
     * @param evidence where the import capability is described
     * @param tested whether an import of our files was verified in the software
     * @param notes notes
     */
    public record Software(
            String id,
            String label,
            String format,
            String extension,
            List<String> alternatives,
            String evidence,
            boolean tested,
            String notes) {}

    static ExportTargets fromJson(JsonNode node) {
        List<Format> formats = new ArrayList<>();
        node.get("formats").forEach(f -> formats.add(new Format(
                Json.text(f, "format"),
                Json.text(f, "label"),
                Json.text(f, "extension"),
                Json.text(f, "media_type"),
                f.path("multi_spectrum").asBoolean(),
                Json.text(f, "notes"))));
        List<Software> software = new ArrayList<>();
        node.get("software").forEach(s -> software.add(new Software(
                Json.text(s, "id"),
                Json.text(s, "label"),
                Json.text(s, "format"),
                Json.text(s, "extension"),
                Json.strings(s.get("alternatives")),
                Json.text(s, "evidence"),
                s.path("tested").asBoolean(),
                Json.text(s, "notes"))));
        Map<String, JsonNode> requirements = new LinkedHashMap<>();
        node.path("requirements").forEach(r -> requirements.put(Json.text(r, "format"), r));
        return new ExportTargets(
                Collections.unmodifiableList(formats),
                Collections.unmodifiableList(software),
                Collections.unmodifiableMap(requirements));
    }
}
