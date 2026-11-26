package org.nirs4all.formats;

/**
 * A reader that recognizes a file.
 *
 * @param format format id, such as {@code jcamp-dx}
 * @param reader reader name
 * @param confidence {@code definite}, {@code likely} or {@code possible}
 * @param reason why the reader matched
 */
public record FormatProbe(String format, String reader, String confidence, String reason) {}
