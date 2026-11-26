package org.nirs4all.formats;

import java.util.List;

/**
 * One signal of several records stacked into a matrix, one row per record.
 *
 * @param signal signal name
 * @param axis axis shared by every row
 * @param values {@code values[record][point]}; missing points are NaN
 */
public record SignalMatrix(String signal, SpectralAxis axis, double[][] values) {

    /**
     * Stack the named 1-D signal of every record. The axes must be identical (values, unit and
     * kind): nothing is resampled.
     *
     * @throws IllegalArgumentException when a record lacks the signal, the signal is not 1-D or
     *     the axes differ
     */
    public static SignalMatrix stack(List<SpectralRecord> records, String signal) {
        if (records.isEmpty()) {
            throw new IllegalArgumentException("no records to stack");
        }
        SpectralAxis axis = null;
        double[][] values = new double[records.size()][];
        for (int i = 0; i < records.size(); i++) {
            Signal current = records.get(i).signals().get(signal);
            if (current == null) {
                throw new IllegalArgumentException("record " + i + " has no signal '" + signal + "'");
            }
            if (!current.isOneDimensional()) {
                throw new IllegalArgumentException(
                        "signal '" + signal + "' of record " + i + " has dims " + current.dims() + ", not [x]");
            }
            if (axis == null) {
                axis = current.axis();
            } else if (!axis.sameAs(current.axis())) {
                throw new IllegalArgumentException(
                        "the axis of signal '" + signal + "' in record " + i + " differs from record 0");
            }
            values[i] = current.values();
        }
        return new SignalMatrix(signal, axis, values);
    }

    /** Stack the only signal name of the first record; see {@link #stack(List, String)}. */
    public static SignalMatrix stack(List<SpectralRecord> records) {
        if (records.isEmpty()) {
            throw new IllegalArgumentException("no records to stack");
        }
        var names = records.get(0).signals().keySet();
        if (names.size() != 1) {
            throw new IllegalArgumentException("records have several signals " + names + "; name one");
        }
        return stack(records, names.iterator().next());
    }

    /** Number of rows (records). */
    public int rows() {
        return values.length;
    }

    /** Number of columns (axis points). */
    public int columns() {
        return axis.size();
    }
}
