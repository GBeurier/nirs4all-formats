package org.nirs4all.formats;

import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

/**
 * Read controls for image cubes. Defaults read every record.
 *
 * <p>A rectangular window ({@link #rows}, {@link #cols}) and a sparse {@link #pixels} mask cannot
 * be combined.
 */
public final class ReadOptions {
    private boolean singleRecord;
    private Integer rowStart;
    private Integer rowEnd;
    private Integer colStart;
    private Integer colEnd;
    private int[][] pixels;

    /** Emit an image cube as one N-dimensional record instead of one record per pixel. */
    public ReadOptions singleRecord(boolean value) {
        this.singleRecord = value;
        return this;
    }

    /** Half-open row window; {@code end} may be {@code null} for "to the last row". */
    public ReadOptions rows(int start, Integer end) {
        this.rowStart = start;
        this.rowEnd = end;
        return this;
    }

    /** Half-open column window; {@code end} may be {@code null} for "to the last column". */
    public ReadOptions cols(int start, Integer end) {
        this.colStart = start;
        this.colEnd = end;
        return this;
    }

    /** Sparse pixel mask as {@code {row, col}} pairs. */
    public ReadOptions pixels(int[][] pairs) {
        this.pixels = pairs.clone();
        return this;
    }

    String toJson() {
        ObjectNode node = Json.object();
        if (singleRecord) {
            node.put("single_record", true);
        }
        if (rowStart != null) {
            node.set("rows", window(rowStart, rowEnd));
        }
        if (colStart != null) {
            node.set("cols", window(colStart, colEnd));
        }
        if (pixels != null) {
            ArrayNode list = node.putArray("pixels");
            for (int[] pair : pixels) {
                list.addArray().add(pair[0]).add(pair[1]);
            }
        }
        return Json.write(node);
    }

    private static ArrayNode window(int start, Integer end) {
        ArrayNode pair = Json.MAPPER.createArrayNode().add(start);
        return end == null ? pair.addNull() : pair.add(end);
    }
}
