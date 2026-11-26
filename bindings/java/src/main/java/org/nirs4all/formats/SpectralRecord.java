package org.nirs4all.formats;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.NoSuchElementException;

/**
 * One normalized sample or acquisition, as decoded by the Rust registry.
 *
 * <p>The typed accessors are read-only views. {@link #json()} returns the complete record JSON;
 * {@link Nirs4allFormats#write} exports that JSON unchanged.
 */
public final class SpectralRecord {
    private final ObjectNode json;
    private final Map<String, Signal> signals;
    private final Map<String, JsonNode> targets;
    private final Provenance provenance;

    SpectralRecord(ObjectNode json) {
        this.json = json;
        Map<String, Signal> signalMap = new LinkedHashMap<>();
        json.get("signals").fields().forEachRemaining(e -> signalMap.put(e.getKey(), Signal.fromJson(e.getKey(), e.getValue())));
        this.signals = Collections.unmodifiableMap(signalMap);
        Map<String, JsonNode> targetMap = new LinkedHashMap<>();
        JsonNode targetNode = json.get("targets");
        if (targetNode != null) {
            targetNode.fields().forEachRemaining(e -> targetMap.put(e.getKey(), e.getValue()));
        }
        this.targets = Collections.unmodifiableMap(targetMap);
        this.provenance = Provenance.fromJson(json.get("provenance"));
    }

    /** Signals by name. */
    public Map<String, Signal> signals() {
        return signals;
    }

    /** The named signal. */
    public Signal signal(String name) {
        Signal signal = signals.get(name);
        if (signal == null) {
            throw new NoSuchElementException("no signal '" + name + "'; signals: " + signals.keySet());
        }
        return signal;
    }

    /** Dominant signal type, such as {@code absorbance}. */
    public String signalType() {
        return Json.text(json, "signal_type");
    }

    /** Reference values by name (numbers, strings or {@code null} when missing). */
    public Map<String, JsonNode> targets() {
        return targets;
    }

    /** A numeric target, or NaN when it is missing or not a number. */
    public double numericTarget(String name) {
        JsonNode value = targets.get(name);
        return value != null && value.isNumber() ? value.asDouble() : Double.NaN;
    }

    /** Acquisition, instrument and sample metadata (a copy). */
    public ObjectNode metadata() {
        JsonNode metadata = json.get("metadata");
        return metadata instanceof ObjectNode object ? object.deepCopy() : Json.object();
    }

    /** Reader, format, source hashes and warnings. */
    public Provenance provenance() {
        return provenance;
    }

    /** Explicit caveats on the record. */
    public java.util.List<String> qualityFlags() {
        return Json.strings(json.get("quality_flags"));
    }

    /** The complete record JSON (a copy). */
    public ObjectNode json() {
        return json.deepCopy();
    }

    ObjectNode rawJson() {
        return json;
    }

    @Override
    public String toString() {
        return "SpectralRecord[format=" + provenance.format() + ", signals=" + signals.keySet() + "]";
    }
}
