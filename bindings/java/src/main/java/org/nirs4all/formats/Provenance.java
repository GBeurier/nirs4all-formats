package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Where a record comes from.
 *
 * @param format detected format id, such as {@code jcamp-dx}
 * @param reader reader that decoded the file
 * @param readerVersion library version of the reader
 * @param sources input files with their SHA-256
 * @param parsedAtUtc parse timestamp, or {@code null}
 * @param recordSchemaVersion version of the record schema
 * @param warnings decoding warnings
 */
public record Provenance(
        String format,
        String reader,
        String readerVersion,
        List<Source> sources,
        String parsedAtUtc,
        String recordSchemaVersion,
        List<String> warnings) {

    /**
     * One physical input file or archive member.
     *
     * @param path file path or name
     * @param archive enclosing archive, or {@code null}
     * @param sha256 SHA-256 of the bytes, hexadecimal
     * @param role {@code primary} or a sidecar role
     */
    public record Source(String path, String archive, String sha256, String role) {}

    static Provenance fromJson(JsonNode node) {
        List<Source> sources = new ArrayList<>();
        JsonNode list = node.get("sources");
        if (list != null) {
            list.forEach(s -> sources.add(new Source(
                    Json.text(s, "path"), Json.text(s, "archive"), Json.text(s, "sha256"), Json.text(s, "role"))));
        }
        return new Provenance(
                Json.text(node, "format"),
                Json.text(node, "reader"),
                Json.text(node, "reader_version"),
                Collections.unmodifiableList(sources),
                Json.text(node, "parsed_at_utc"),
                Json.text(node, "record_schema_version"),
                Json.strings(node.get("warnings")));
    }
}
