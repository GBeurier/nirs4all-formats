package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Files saved by {@link Nirs4allFormats#convert} and the declared losses.
 *
 * @param format writer that produced the files
 * @param files saved paths
 * @param losses declared losses
 * @param conversions conversion steps recorded in the records' metadata, as JSON
 */
public record ConvertResult(
        String format, List<Path> files, List<WriteResult.Loss> losses, List<JsonNode> conversions) {
    static ConvertResult fromJson(JsonNode node) {
        List<Path> files = new ArrayList<>();
        node.get("files").forEach(file -> files.add(Path.of(file.asText())));
        return new ConvertResult(
                Json.text(node, "format"),
                Collections.unmodifiableList(files),
                WriteResult.Loss.listFromJson(node.get("losses")),
                WriteResult.nodes(node.get("conversions")));
    }
}
