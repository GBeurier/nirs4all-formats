package org.nirs4all.formats;

import com.fasterxml.jackson.core.JsonFactory;
import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.core.StreamReadConstraints;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Shared JSON helpers: the C ABI exchanges UTF-8 JSON. */
final class Json {
    // Exported files travel base64-encoded: lift Jackson's 20 M character string cap.
    static final ObjectMapper MAPPER = new ObjectMapper(JsonFactory.builder()
            .streamReadConstraints(StreamReadConstraints.builder()
                    .maxStringLength(Integer.MAX_VALUE).build())
            .build());

    private Json() {}

    static JsonNode parse(String text) {
        try {
            return MAPPER.readTree(text);
        } catch (JsonProcessingException error) {
            throw new IllegalStateException("the native library returned invalid JSON", error);
        }
    }

    static String write(JsonNode node) {
        try {
            return MAPPER.writeValueAsString(node);
        } catch (JsonProcessingException error) {
            throw new IllegalArgumentException("cannot serialize JSON", error);
        }
    }

    static ObjectNode object() {
        return MAPPER.createObjectNode();
    }

    static String text(JsonNode node, String field) {
        JsonNode value = node.get(field);
        return value == null || value.isNull() ? null : value.asText();
    }

    /** Numbers of a JSON array; {@code null} entries (missing values) become NaN. */
    static double[] doubles(JsonNode array) {
        if (array == null || array.isNull()) {
            return new double[0];
        }
        double[] out = new double[array.size()];
        for (int i = 0; i < out.length; i++) {
            JsonNode value = array.get(i);
            out[i] = value.isNull() ? Double.NaN : value.asDouble();
        }
        return out;
    }

    static List<String> strings(JsonNode array) {
        if (array == null || array.isNull()) {
            return List.of();
        }
        List<String> out = new ArrayList<>(array.size());
        array.forEach(value -> out.add(value.asText()));
        return Collections.unmodifiableList(out);
    }
}
