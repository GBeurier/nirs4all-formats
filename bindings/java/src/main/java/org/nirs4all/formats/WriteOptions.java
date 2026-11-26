package org.nirs4all.formats;

import com.fasterxml.jackson.databind.node.ObjectNode;

/** Export controls. Defaults write every signal of every record. */
public final class WriteOptions {
    private String signal;
    private String baseName;
    private String extension;
    private Double axisTolerance;
    private boolean multi;
    private Character delimiter;
    private boolean zip;
    private boolean overwrite;

    /** Write this signal only. */
    public WriteOptions signal(String name) {
        this.signal = name;
        return this;
    }

    /** Stem of the produced file names (default: the source file stem). */
    public WriteOptions baseName(String stem) {
        this.baseName = stem;
        return this;
    }

    /** File extension override, without the dot. */
    public WriteOptions extension(String value) {
        this.extension = value;
        return this;
    }

    /**
     * Largest absolute axis deviation, in axis units, accepted when an almost evenly spaced axis
     * is written as even; the deviation is declared as a loss.
     */
    public WriteOptions axisTolerance(double value) {
        this.axisTolerance = value;
        return this;
    }

    /** Group spectra sharing one axis into multi-spectrum files when the target supports it. */
    public WriteOptions multi(boolean value) {
        this.multi = value;
        return this;
    }

    /** Column separator of delimited targets: {@code ','}, {@code ';'} or {@code '\t'}. */
    public WriteOptions delimiter(char value) {
        this.delimiter = value;
        return this;
    }

    /** Return one ZIP archive instead of separate files. */
    public WriteOptions zip(boolean value) {
        this.zip = value;
        return this;
    }

    /** Replace existing files when saving ({@link Nirs4allFormats#convert} only). */
    public WriteOptions overwrite(boolean value) {
        this.overwrite = value;
        return this;
    }

    String toJson(boolean withOverwrite) {
        ObjectNode node = Json.object();
        if (signal != null) {
            node.put("signal", signal);
        }
        if (baseName != null) {
            node.put("base_name", baseName);
        }
        if (extension != null) {
            node.put("extension", extension);
        }
        if (axisTolerance != null) {
            node.put("axis_tolerance", axisTolerance);
        }
        if (multi) {
            node.put("multi", true);
        }
        if (delimiter != null) {
            node.put("delimiter", delimiter.toString());
        }
        if (zip) {
            node.put("zip", true);
        }
        if (withOverwrite && overwrite) {
            node.put("overwrite", true);
        }
        return Json.write(node);
    }
}
