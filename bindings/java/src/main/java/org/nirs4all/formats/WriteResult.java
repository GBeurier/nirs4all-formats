package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;

/**
 * Files produced by an export, in memory, and what the target could not carry.
 *
 * @param format writer that produced the files
 * @param files produced files
 * @param losses declared losses
 * @param conversions conversion steps recorded in the records' metadata, as JSON
 */
public record WriteResult(String format, List<File> files, List<Loss> losses, List<JsonNode> conversions) {

    /**
     * One produced file.
     *
     * @param name file name
     * @param mediaType media type
     * @param bytes file content
     */
    public record File(String name, String mediaType, byte[] bytes) {
        /** Save into {@code directory}, refusing to replace an existing file. */
        public Path save(Path directory) throws IOException {
            Files.createDirectories(directory);
            return Files.write(directory.resolve(name), bytes, StandardOpenOption.CREATE_NEW);
        }
    }

    /**
     * Information the target does not carry, or carries approximately.
     *
     * @param code stable loss identifier
     * @param message description
     * @param count number of spectra or records concerned
     * @param maxError largest absolute error when the loss is numerical, else {@code null}
     */
    public record Loss(String code, String message, long count, Double maxError) {
        static List<Loss> listFromJson(JsonNode array) {
            List<Loss> out = new ArrayList<>();
            if (array != null) {
                array.forEach(node -> {
                    JsonNode maxError = node.get("max_error");
                    out.add(new Loss(
                            Json.text(node, "code"),
                            Json.text(node, "message"),
                            node.path("count").asLong(),
                            maxError == null || maxError.isNull() ? null : maxError.asDouble()));
                });
            }
            return Collections.unmodifiableList(out);
        }
    }

    static WriteResult fromJson(JsonNode node) {
        Base64.Decoder decoder = Base64.getDecoder();
        List<File> files = new ArrayList<>();
        node.get("files").forEach(file -> files.add(new File(
                Json.text(file, "name"),
                Json.text(file, "media_type"),
                decoder.decode(Json.text(file, "bytes_base64")))));
        return new WriteResult(
                Json.text(node, "format"),
                Collections.unmodifiableList(files),
                Loss.listFromJson(node.get("losses")),
                nodes(node.get("conversions")));
    }

    static List<JsonNode> nodes(JsonNode array) {
        List<JsonNode> out = new ArrayList<>();
        if (array != null) {
            array.forEach(out::add);
        }
        return Collections.unmodifiableList(out);
    }

    /** Save every file into {@code directory}, refusing to replace existing files. */
    public List<Path> save(Path directory) throws IOException {
        for (File file : files) {
            if (Files.exists(directory.resolve(file.name()))) {
                throw new IOException("file already exists: " + directory.resolve(file.name()));
            }
        }
        List<Path> paths = new ArrayList<>();
        for (File file : files) {
            paths.add(file.save(directory));
        }
        return paths;
    }
}
