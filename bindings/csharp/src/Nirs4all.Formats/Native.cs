using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;

namespace Nirs4all.Formats;

/// <summary>P/Invoke mapping of the nirs4all-formats C ABI (<c>nirs4all_formats.h</c>).</summary>
internal static class Native
{
    /// <summary>Environment variable naming the native library file.</summary>
    internal const string LibraryEnv = "NIRS4ALL_FORMATS_LIBRARY";

    private const string Library = "nirs4all_formats_capi";

    static Native()
    {
        NativeLibrary.SetDllImportResolver(typeof(Native).Assembly, Resolve);
    }

    private static IntPtr Resolve(string name, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (name != Library)
        {
            return IntPtr.Zero;
        }
        string? explicitPath = Environment.GetEnvironmentVariable(LibraryEnv);
        if (!string.IsNullOrWhiteSpace(explicitPath))
        {
            if (!File.Exists(explicitPath))
            {
                throw new DllNotFoundException($"{LibraryEnv} names a missing file: {explicitPath}");
            }
            return NativeLibrary.Load(explicitPath);
        }
        // Default probing: runtimes/<rid>/native of the package, the application
        // directory, then the system library path.
        return IntPtr.Zero;
    }

    internal delegate int JsonCall(out IntPtr json, out IntPtr error);

    /// <summary>Run a JSON-returning call, returning the JSON or throwing <see cref="FormatsException"/>.</summary>
    internal static string Call(JsonCall call)
    {
        int status = call(out IntPtr json, out IntPtr error);
        if (status == 0)
        {
            return Take(json) ?? throw new InvalidOperationException("the native library returned no result");
        }
        string? message = Take(error);
        Take(json);
        throw new FormatsException(status, message ?? "native call failed");
    }

    /// <summary>Copy and free a string returned by the C ABI.</summary>
    internal static string? Take(IntPtr pointer)
    {
        if (pointer == IntPtr.Zero)
        {
            return null;
        }
        try
        {
            return Marshal.PtrToStringUTF8(pointer);
        }
        finally
        {
            n4fmt_string_free(pointer);
        }
    }

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr n4fmt_abi_version();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr n4fmt_library_version();

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void n4fmt_string_free(IntPtr ptr);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_reader_catalog(out IntPtr outJson, out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_probe_path(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_probe_bytes(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        byte[] bytes,
        nuint len,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_open_path(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? optionsJson,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_open_bytes(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string name,
        byte[] bytes,
        nuint len,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? optionsJson,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_export_targets(out IntPtr outJson, out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_write_records(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string recordsJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string target,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? optionsJson,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_plan_conversion(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string recordsJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? target,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? optionsJson,
        out IntPtr outJson,
        out IntPtr outError);

    [DllImport(Library, CallingConvention = CallingConvention.Cdecl)]
    internal static extern int n4fmt_convert_path(
        [MarshalAs(UnmanagedType.LPUTF8Str)] string inputPath,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string target,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string outputDir,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string? optionsJson,
        out IntPtr outJson,
        out IntPtr outError);
}
