package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import java.util.Arrays;

/**
 * Coordinate of one signal dimension.
 *
 * @param values native axis values, in the stored order
 * @param unit unit such as {@code nm}, {@code cm-1} or {@code index}
 * @param kind {@code wavelength}, {@code wavenumber}, {@code frequency}, {@code energy},
 *     {@code time} or {@code index}
 * @param order {@code ascending}, {@code descending} or {@code non_monotonic}
 */
public record SpectralAxis(double[] values, String unit, String kind, String order) {
    static SpectralAxis fromJson(JsonNode node) {
        return new SpectralAxis(
                Json.doubles(node.get("values")),
                Json.text(node, "unit"),
                Json.text(node, "kind"),
                Json.text(node, "order"));
    }

    /** Number of axis points. */
    public int size() {
        return values.length;
    }

    /** Whether both axes have the same values, unit and kind. */
    public boolean sameAs(SpectralAxis other) {
        return other != null
                && Arrays.equals(values, other.values)
                && java.util.Objects.equals(unit, other.unit)
                && java.util.Objects.equals(kind, other.kind);
    }
}
