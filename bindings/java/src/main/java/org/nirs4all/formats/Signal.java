package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * One named signal channel of a record. Each signal owns its axis.
 *
 * @param name signal name within the record
 * @param axis coordinate of the spectral ({@code x}) dimension
 * @param values flat C-order (row-major) values; missing points are NaN
 * @param shape extent of each dimension
 * @param dims dimension names; exactly one is {@code x}
 * @param coords coordinates of the non-{@code x} dimensions
 * @param signalType signal type such as {@code absorbance} or {@code reflectance}
 * @param unit physical unit of the values, or {@code null}
 * @param role role such as {@code signal}, {@code white_ref} or {@code absorbance}
 * @param source {@code file} or {@code derived}
 */
public record Signal(
        String name,
        SpectralAxis axis,
        double[] values,
        int[] shape,
        List<String> dims,
        Map<String, SpectralAxis> coords,
        String signalType,
        String unit,
        String role,
        String source) {

    static Signal fromJson(String name, JsonNode node) {
        JsonNode shapeNode = node.get("shape");
        int[] shape = new int[shapeNode == null ? 0 : shapeNode.size()];
        for (int i = 0; i < shape.length; i++) {
            shape[i] = shapeNode.get(i).asInt();
        }
        Map<String, SpectralAxis> coords = new LinkedHashMap<>();
        JsonNode coordsNode = node.get("coords");
        if (coordsNode != null && coordsNode.isObject()) {
            coordsNode.fields().forEachRemaining(e -> coords.put(e.getKey(), SpectralAxis.fromJson(e.getValue())));
        }
        return new Signal(
                name,
                SpectralAxis.fromJson(node.get("axis")),
                Json.doubles(node.get("values")),
                shape,
                Json.strings(node.get("dims")),
                Collections.unmodifiableMap(coords),
                Json.text(node, "signal_type"),
                Json.text(node, "unit"),
                Json.text(node, "role"),
                Json.text(node, "source"));
    }

    /** Whether this is a plain spectrum ({@code dims == ["x"]}). */
    public boolean isOneDimensional() {
        return dims.size() == 1 && "x".equals(dims.get(0));
    }
}
