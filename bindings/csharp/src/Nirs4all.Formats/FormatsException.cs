using System;

namespace Nirs4all.Formats;

/// <summary>Failure categories of the C ABI (<c>N4FMT_STATUS_*</c>).</summary>
public enum FormatsStatus
{
    /// <summary>A required argument is missing, or an option or record is invalid.</summary>
    InvalidArgument = 1,
    /// <summary>A file could not be read or saved.</summary>
    Io = 2,
    /// <summary>No reader recognizes the input.</summary>
    UnsupportedFormat = 3,
    /// <summary>The format needs a companion file that is not available.</summary>
    UnsupportedSidecar = 4,
    /// <summary>The input was recognized but cannot be decoded.</summary>
    InvalidData = 5,
    /// <summary>The target is neither an export format nor a software preset.</summary>
    UnknownTarget = 6,
    /// <summary>The records or options cannot be represented in the target.</summary>
    ExportRefused = 7,
    /// <summary>An internal error of the native library.</summary>
    Internal = 99,
}

/// <summary>A nirs4all-formats call failed; <see cref="Status"/> tells why.</summary>
public sealed class FormatsException : Exception
{
    internal FormatsException(int status, string message)
        : base(message)
    {
        Status = Enum.IsDefined(typeof(FormatsStatus), status) ? (FormatsStatus)status : FormatsStatus.Internal;
    }

    /// <summary>Why the call failed.</summary>
    public FormatsStatus Status { get; }
}
