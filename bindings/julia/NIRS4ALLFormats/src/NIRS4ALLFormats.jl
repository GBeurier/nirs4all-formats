"""
    NIRS4ALLFormats

Read NIRS and spectroscopy files and export records through the nirs4all-formats
native library (its C ABI). Parsing and writing happen in the Rust registry; this
package converts the record JSON into Julia values.

The native library is located by the `NIRS4ALL_FORMATS_LIBRARY` environment
variable (a file path) or, when it is installed, `nirs4all_formats_capi_jll`.
"""
module NIRS4ALLFormats

using Base64: base64decode
using JSON3
using Libdl

export SpectralAxis, Signal, SourceFile, Provenance, SpectralRecord, FormatProbe,
    WrittenFile, FormatsError,
    library_version, abi_version, readers, probe, read_records, signal_matrix,
    export_targets, write_records, convert_path

const LIBRARY_ENV = "NIRS4ALL_FORMATS_LIBRARY"
const JLL_NAME = "nirs4all_formats_capi_jll"

# ---------------------------------------------------------------------------
# Native library
# ---------------------------------------------------------------------------

const _handle = Ref{Ptr{Cvoid}}(C_NULL)
const _symbols = Dict{Symbol,Ptr{Cvoid}}()
const _lock = ReentrantLock()

"""Path of the JLL library when the JLL package can be loaded, else `nothing`."""
function _jll_library()
    id = Base.identify_package(@__MODULE__, JLL_NAME)
    id === nothing && (id = Base.identify_package(JLL_NAME))
    id === nothing && return nothing
    jll = Base.require(id)
    Base.invokelatest(jll.is_available) || return nothing
    return Base.invokelatest(getproperty, jll, :libnirs4all_formats_capi_path)
end

function _library_path()
    path = get(ENV, LIBRARY_ENV, "")
    isempty(path) || return path
    path = _jll_library()
    path === nothing || return path
    error("nirs4all-formats native library not found: set $LIBRARY_ENV to the path of " *
          "libnirs4all_formats_capi, or install $JLL_NAME")
end

function _sym(name::Symbol)
    lock(_lock) do
        get!(_symbols, name) do
            if _handle[] == C_NULL
                _handle[] = Libdl.dlopen(_library_path())
            end
            Libdl.dlsym(_handle[], name)
        end
    end
end

"""Copy and free a string returned by the C ABI."""
function _take(ptr::Ptr{UInt8})
    ptr == C_NULL && return nothing
    text = unsafe_string(ptr)
    ccall(_sym(:n4fmt_string_free), Cvoid, (Ptr{UInt8},), ptr)
    return text
end

const _STATUS = Dict{Int32,Symbol}(
    1 => :invalid_argument,
    2 => :io,
    3 => :unsupported_format,
    4 => :unsupported_sidecar,
    5 => :invalid_data,
    6 => :unknown_target,
    7 => :export_refused,
    99 => :internal,
)

"""
    FormatsError(status, message)

A nirs4all-formats call failed. `status` is one of `:invalid_argument`, `:io`,
`:unsupported_format`, `:unsupported_sidecar`, `:invalid_data`, `:unknown_target`,
`:export_refused` or `:internal`.
"""
struct FormatsError <: Exception
    status::Symbol
    message::String
end

Base.showerror(io::IO, e::FormatsError) = print(io, "FormatsError(", e.status, "): ", e.message)

"""Run a JSON-returning call `f(out_json, out_error)` and return the parsed JSON."""
function _call(f)
    out = Ref{Ptr{UInt8}}(C_NULL)
    err = Ref{Ptr{UInt8}}(C_NULL)
    status = f(out, err)
    if status == 0
        return JSON3.read(_take(out[]))
    end
    message = _take(err[])
    _take(out[])
    throw(FormatsError(get(_STATUS, status, :internal), something(message, "native call failed")))
end

"""Library version of the native nirs4all-formats."""
library_version() = _take(ccall(_sym(:n4fmt_library_version), Ptr{UInt8}, ()))

"""Version of the C ABI."""
abi_version() = _take(ccall(_sym(:n4fmt_abi_version), Ptr{UInt8}, ()))

# ---------------------------------------------------------------------------
# Records
# ---------------------------------------------------------------------------

"""
    SpectralAxis

Coordinate of one signal dimension: `values`, `unit` (`"nm"`, `"cm-1"`, ...),
`kind` (`"wavelength"`, `"wavenumber"`, ...) and `order` (`"ascending"`,
`"descending"` or `"non_monotonic"`).
"""
struct SpectralAxis
    values::Vector{Float64}
    unit::String
    kind::String
    order::String
end

"""
    Signal

One named signal channel; each signal owns its axis. `values` is the flat C-order
(row-major) buffer of `prod(shape)` numbers with NaN for missing points; for an
N-dimensional signal use `permutedims(reshape(values, reverse(shape)...), ndims:-1:1)`.
"""
struct Signal
    name::String
    axis::SpectralAxis
    values::Vector{Float64}
    shape::Vector{Int}
    dims::Vector{String}
    coords::Dict{String,SpectralAxis}
    signal_type::String
    unit::Union{Nothing,String}
    role::String
    source::String
end

"""One physical input file: `path`, `archive`, `sha256` and `role`."""
struct SourceFile
    path::String
    archive::Union{Nothing,String}
    sha256::String
    role::String
end

"""Reader, format, source hashes and warnings of a record."""
struct Provenance
    format::String
    reader::String
    reader_version::String
    sources::Vector{SourceFile}
    parsed_at_utc::Union{Nothing,String}
    record_schema_version::String
    warnings::Vector{String}
end

"""
    SpectralRecord

One normalized sample or acquisition: `signals` by name, `signal_type`, `targets`
(numbers, strings or `nothing` when missing), `metadata`, `provenance` and
`quality_flags`. `json` holds the complete record, which [`write_records`] exports
unchanged.
"""
struct SpectralRecord
    signals::Dict{String,Signal}
    signal_type::String
    targets::Dict{String,Any}
    metadata::Dict{String,Any}
    provenance::Provenance
    quality_flags::Vector{String}
    json::JSON3.Object
end

"""A reader that recognizes a file: `format`, `reader`, `confidence` and `reason`."""
struct FormatProbe
    format::String
    reader::String
    confidence::String
    reason::String
end

"""One produced file: `name`, `media_type` and `bytes`."""
struct WrittenFile
    name::String
    media_type::String
    bytes::Vector{UInt8}
end

_str(x) = x === nothing ? nothing : String(x)
_floats(values) = Float64[v === nothing ? NaN : Float64(v) for v in values]
_plain(x) = JSON3.read(JSON3.write(x), Dict{String,Any})

_axis(node) = SpectralAxis(_floats(node.values), String(node.unit), String(node.kind), String(node.order))

function _signal(name, node)
    coords = Dict{String,SpectralAxis}()
    if haskey(node, :coords)
        for (dim, axis) in pairs(node.coords)
            coords[String(dim)] = _axis(axis)
        end
    end
    return Signal(String(name), _axis(node.axis), _floats(node.values), Int.(collect(node.shape)),
        String.(collect(node.dims)), coords, String(node.signal_type), _str(get(node, :unit, nothing)),
        String(node.role), String(node.source))
end

function _record(node::JSON3.Object)
    p = node.provenance
    sources = [SourceFile(String(s.path), _str(get(s, :archive, nothing)), String(s.sha256), String(s.role))
               for s in p.sources]
    provenance = Provenance(String(p.format), String(p.reader), String(p.reader_version), sources,
        _str(get(p, :parsed_at_utc, nothing)), String(p.record_schema_version), String.(collect(p.warnings)))
    signals = Dict{String,Signal}(String(k) => _signal(k, v) for (k, v) in pairs(node.signals))
    return SpectralRecord(signals, String(node.signal_type), _plain(node.targets), _plain(node.metadata),
        provenance, String.(collect(node.quality_flags)), node)
end

_records(array) = SpectralRecord[_record(node) for node in array]

_probes(array) = FormatProbe[FormatProbe(String(p.format), String(p.reader), String(p.confidence), String(p.reason))
                             for p in array]

function _read_options(; single_record::Bool=false, rows=nothing, cols=nothing, pixels=nothing)
    options = Dict{String,Any}()
    single_record && (options["single_record"] = true)
    rows === nothing || (options["rows"] = collect(rows))
    cols === nothing || (options["cols"] = collect(cols))
    pixels === nothing || (options["pixels"] = [collect(p) for p in pixels])
    return JSON3.write(options)
end

# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

"""
    readers() -> Vector{String}

Readers compiled into the native library.
"""
readers() = String[String(r.reader) for r in _call((o, e) -> ccall(_sym(:n4fmt_reader_catalog), Int32,
    (Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), o, e))]

"""
    probe(path) -> Vector{FormatProbe}
    probe(name, bytes) -> Vector{FormatProbe}

Readers that recognize a file or in-memory content named `name`, most confident first.
"""
probe(path::AbstractString) = _probes(_call((o, e) -> ccall(_sym(:n4fmt_probe_path), Int32,
    (Cstring, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), path, o, e)))

probe(name::AbstractString, bytes::AbstractVector{UInt8}) = let data = Vector{UInt8}(bytes)
    _probes(_call((o, e) -> ccall(_sym(:n4fmt_probe_bytes), Int32,
        (Cstring, Ptr{UInt8}, Csize_t, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), name, data, length(data), o, e)))
end

"""
    read_records(path; single_record=false, rows=nothing, cols=nothing, pixels=nothing)
    read_records(name, bytes; kwargs...)

Read a file, or in-memory content named `name`, into `SpectralRecord`s. Image cubes
accept a half-open `rows`/`cols` window as `(start, stop)` (`stop` may be `nothing`)
or a sparse `pixels` list of `(row, col)` pairs, and `single_record=true` for one
N-dimensional record. Companion files of sidecar formats are found next to `path`;
in-memory content that needs one fails with `:unsupported_sidecar`.
"""
function read_records(path::AbstractString; kwargs...)
    options = _read_options(; kwargs...)
    return _records(_call((o, e) -> ccall(_sym(:n4fmt_open_path), Int32,
        (Cstring, Cstring, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), path, options, o, e)))
end

function read_records(name::AbstractString, bytes::AbstractVector{UInt8}; kwargs...)
    options = _read_options(; kwargs...)
    data = Vector{UInt8}(bytes)
    return _records(_call((o, e) -> ccall(_sym(:n4fmt_open_bytes), Int32,
        (Cstring, Ptr{UInt8}, Csize_t, Cstring, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}),
        name, data, length(data), options, o, e)))
end

"""
    signal_matrix(records, signal=nothing) -> (values, axis, signal)

Stack one 1-D signal of every record into a `length(records) × length(axis.values)`
matrix. The axes must be identical (values, unit and kind): nothing is resampled.
Without a name, the only signal of the first record is used.
"""
function signal_matrix(records::AbstractVector{SpectralRecord}, signal::Union{Nothing,AbstractString}=nothing)
    isempty(records) && throw(ArgumentError("no records to stack"))
    if signal === nothing
        names = collect(keys(records[1].signals))
        length(names) == 1 || throw(ArgumentError("records have several signals $(sort(names)); name one"))
        signal = only(names)
    end
    first_signal = get(records[1].signals, signal, nothing)
    first_signal === nothing && throw(ArgumentError("record 1 has no signal '$signal'"))
    axis = first_signal.axis
    values = Matrix{Float64}(undef, length(records), length(axis.values))
    for (i, record) in enumerate(records)
        current = get(record.signals, signal, nothing)
        current === nothing && throw(ArgumentError("record $i has no signal '$signal'"))
        current.dims == ["x"] || throw(ArgumentError("signal '$signal' of record $i has dims $(current.dims), not [x]"))
        same = current.axis.values == axis.values && current.axis.unit == axis.unit && current.axis.kind == axis.kind
        same || throw(ArgumentError("the axis of signal '$signal' in record $i differs from record 1"))
        values[i, :] = current.values
    end
    return (values=values, axis=axis, signal=String(signal))
end

"""
    export_targets()

Export formats (`formats`) and software presets (`software`) accepted as write targets.
"""
export_targets() = _call((o, e) -> ccall(_sym(:n4fmt_export_targets), Int32,
    (Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), o, e))

function _write_options(; signal=nothing, base_name=nothing, extension=nothing, axis_tolerance=nothing,
                        multi::Bool=false, delimiter=nothing, zip::Bool=false, overwrite=nothing)
    options = Dict{String,Any}()
    signal === nothing || (options["signal"] = signal)
    base_name === nothing || (options["base_name"] = base_name)
    extension === nothing || (options["extension"] = extension)
    axis_tolerance === nothing || (options["axis_tolerance"] = axis_tolerance)
    multi && (options["multi"] = true)
    delimiter === nothing || (options["delimiter"] = string(delimiter))
    zip && (options["zip"] = true)
    overwrite === nothing || (options["overwrite"] = overwrite)
    return JSON3.write(options)
end

_losses(array) = [Dict{String,Any}(String(k) => v for (k, v) in pairs(loss)) for loss in array]

"""
    write_records(records, target; signal, base_name, extension, axis_tolerance, multi, delimiter, zip)
        -> (format, files::Vector{WrittenFile}, losses)

Write records to an export format or software preset (see [`export_targets`]), in
memory. Nothing is resampled or converted; what the target cannot carry is refused
or listed in `losses`.
"""
function write_records(records::AbstractVector{SpectralRecord}, target::AbstractString; kwargs...)
    payload = JSON3.write([record.json for record in records])
    options = _write_options(; kwargs...)
    result = _call((o, e) -> ccall(_sym(:n4fmt_write_records), Int32,
        (Cstring, Cstring, Cstring, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}), payload, target, options, o, e))
    files = WrittenFile[WrittenFile(String(f.name), String(f.media_type), base64decode(String(f.bytes_base64)))
                        for f in result.files]
    return (format=String(result.format), files=files, losses=_losses(result.losses))
end

"""
    convert_path(input, target, output_dir; overwrite=false, kwargs...) -> (format, files, losses)

Read `input` and save its export into the existing directory `output_dir`. Existing
files are kept, and nothing is written, unless `overwrite=true`. Other keywords are
those of [`write_records`].
"""
function convert_path(input::AbstractString, target::AbstractString, output_dir::AbstractString;
                      overwrite::Bool=false, kwargs...)
    options = _write_options(; overwrite=overwrite, kwargs...)
    result = _call((o, e) -> ccall(_sym(:n4fmt_convert_path), Int32,
        (Cstring, Cstring, Cstring, Cstring, Ptr{Ptr{UInt8}}, Ptr{Ptr{UInt8}}),
        input, target, output_dir, options, o, e))
    return (format=String(result.format), files=String.(collect(result.files)), losses=_losses(result.losses))
end

end # module
