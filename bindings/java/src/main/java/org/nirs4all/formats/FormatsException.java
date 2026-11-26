package org.nirs4all.formats;

/** A nirs4all-formats call failed; {@link #status()} tells why. */
public final class FormatsException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    /** Failure categories of the C ABI ({@code N4FMT_STATUS_*}). */
    public enum Status {
        /** A required argument is missing, or an option or record is invalid. */
        INVALID_ARGUMENT(1),
        /** A file could not be read or saved. */
        IO(2),
        /** No reader recognizes the input. */
        UNSUPPORTED_FORMAT(3),
        /** The format needs a companion file that is not available. */
        UNSUPPORTED_SIDECAR(4),
        /** The input was recognized but cannot be decoded. */
        INVALID_DATA(5),
        /** The target is neither an export format nor a software preset. */
        UNKNOWN_TARGET(6),
        /** The records or options cannot be represented in the target. */
        EXPORT_REFUSED(7),
        /** An internal error of the native library. */
        INTERNAL(99);

        private final int code;

        Status(int code) {
            this.code = code;
        }

        /** The numeric {@code N4FMT_STATUS_*} code. */
        public int code() {
            return code;
        }

        static Status of(int code) {
            for (Status status : values()) {
                if (status.code == code) {
                    return status;
                }
            }
            return INTERNAL;
        }
    }

    private final Status status;

    FormatsException(int code, String message) {
        super(message);
        this.status = Status.of(code);
    }

    /** Why the call failed. */
    public Status status() {
        return status;
    }
}
