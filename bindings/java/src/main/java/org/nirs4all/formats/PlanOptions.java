package org.nirs4all.formats;

import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

/** Choices and answers of a conversion plan. Defaults plan automatically. */
public final class PlanOptions {
    private final ObjectNode node = Json.object();

    /** Signal to convert (default: the best candidate for the target). */
    public PlanOptions signal(String name) {
        node.put("signal", name);
        return this;
    }

    /** Wanted signal type, such as {@code absorbance}, for targets that accept several or none. */
    public PlanOptions signalType(String type) {
        node.put("signal_type", type);
        return this;
    }

    /** Wanted axis unit: {@code nm}, {@code um} or {@code cm-1}. */
    public PlanOptions axisUnit(String unit) {
        node.put("axis_unit", unit);
        return this;
    }

    /** {@code measured} (no resampling), {@code even}, or a grid id of the target. */
    public PlanOptions grid(String grid) {
        node.put("grid", grid);
        return this;
    }

    /** Step of an {@code even} grid, in the output axis unit. */
    public PlanOptions gridStep(double step) {
        node.put("grid_step", step);
        return this;
    }

    /** Limits of an {@code even} grid, within the measured range. */
    public PlanOptions gridRange(double min, double max) {
        node.put("grid_min", min);
        node.put("grid_max", max);
        return this;
    }

    /** Resampling method: {@code linear} (default), {@code nearest} or {@code cubic}. */
    public PlanOptions interpolation(String method) {
        node.put("interpolation", method);
        return this;
    }

    /** Answer to the {@code source_scale} question: {@code percent} or {@code fraction}. */
    public PlanOptions sourceScale(String scale) {
        node.put("source_scale", scale);
        return this;
    }

    /** Answer to the {@code signal_type} question for signals of unknown type. */
    public PlanOptions assumeSignalType(String type) {
        node.put("assume_signal_type", type);
        return this;
    }

    /** Answer to the {@code absorbance_origin} question: {@code reflectance} or {@code transmittance}. */
    public PlanOptions absorbanceOrigin(String type) {
        node.put("absorbance_origin", type);
        return this;
    }

    /** Axis deviation accepted as even, as in {@link WriteOptions#axisTolerance}. */
    public PlanOptions axisTolerance(double tolerance) {
        node.put("axis_tolerance", tolerance);
        return this;
    }

    /** First axis value of each following detector, for the splice correction. */
    public PlanOptions splices(double... starts) {
        ArrayNode list = node.putArray("splices");
        for (double start : starts) {
            list.add(start);
        }
        return this;
    }

    String toJson() {
        return Json.write(node);
    }
}
