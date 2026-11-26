package org.nirs4all.formats;

import com.sun.jna.IntegerType;
import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;
import java.nio.charset.StandardCharsets;
import java.util.Map;

/** JNA mapping of the nirs4all-formats C ABI ({@code nirs4all_formats.h}). */
final class NativeLibrary {
    /** System property naming the native library file. */
    static final String LIBRARY_PROPERTY = "nirs4all.formats.library";
    /** Environment variable naming the native library file. */
    static final String LIBRARY_ENV = "NIRS4ALL_FORMATS_LIBRARY";
    /** Library name searched by JNA when no explicit file is given. */
    static final String LIBRARY_NAME = "nirs4all_formats_capi";

    private NativeLibrary() {}

    /** {@code size_t}. */
    public static final class SizeT extends IntegerType {
        private static final long serialVersionUID = 1L;

        public SizeT() {
            this(0);
        }

        public SizeT(long value) {
            super(Native.SIZE_T_SIZE, value, true);
        }
    }

    interface Capi extends Library {
        Pointer n4fmt_abi_version();

        Pointer n4fmt_library_version();

        void n4fmt_string_free(Pointer ptr);

        int n4fmt_reader_catalog(PointerByReference outJson, PointerByReference outError);

        int n4fmt_probe_path(String path, PointerByReference outJson, PointerByReference outError);

        int n4fmt_probe_bytes(
                String name,
                byte[] bytes,
                SizeT len,
                PointerByReference outJson,
                PointerByReference outError);

        int n4fmt_open_path(
                String path, String optionsJson, PointerByReference outJson, PointerByReference outError);

        int n4fmt_open_bytes(
                String name,
                byte[] bytes,
                SizeT len,
                String optionsJson,
                PointerByReference outJson,
                PointerByReference outError);

        int n4fmt_export_targets(PointerByReference outJson, PointerByReference outError);

        int n4fmt_write_records(
                String recordsJson,
                String target,
                String optionsJson,
                PointerByReference outJson,
                PointerByReference outError);

        int n4fmt_plan_conversion(
                String recordsJson,
                String target,
                String optionsJson,
                PointerByReference outJson,
                PointerByReference outError);

        int n4fmt_convert_path(
                String inputPath,
                String target,
                String outputDir,
                String optionsJson,
                PointerByReference outJson,
                PointerByReference outError);
    }

    private static final class Holder {
        static final Capi CAPI = load();
    }

    static Capi capi() {
        return Holder.CAPI;
    }

    private static Capi load() {
        String explicit = System.getProperty(LIBRARY_PROPERTY);
        if (explicit == null || explicit.isBlank()) {
            explicit = System.getenv(LIBRARY_ENV);
        }
        String name = explicit == null || explicit.isBlank() ? LIBRARY_NAME : explicit;
        Map<String, Object> options = Map.of(Library.OPTION_STRING_ENCODING, "UTF-8");
        return Native.load(name, Capi.class, options);
    }

    /** A JSON-returning native call. */
    @FunctionalInterface
    interface Call {
        int invoke(Capi capi, PointerByReference outJson, PointerByReference outError);
    }

    /** Run {@code call}, returning the result JSON or throwing {@link FormatsException}. */
    static String call(Call call) {
        Capi capi = capi();
        PointerByReference out = new PointerByReference();
        PointerByReference error = new PointerByReference();
        int status = call.invoke(capi, out, error);
        if (status == 0) {
            return take(capi, out.getValue());
        }
        String message = take(capi, error.getValue());
        take(capi, out.getValue());
        throw new FormatsException(status, message == null ? "native call failed" : message);
    }

    /** Copy and free a string returned by the C ABI. */
    static String take(Capi capi, Pointer pointer) {
        if (pointer == null) {
            return null;
        }
        try {
            return pointer.getString(0, StandardCharsets.UTF_8.name());
        } finally {
            capi.n4fmt_string_free(pointer);
        }
    }
}
