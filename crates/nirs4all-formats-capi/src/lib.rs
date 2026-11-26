//! C ABI over the `nirs4all-formats` registry and writers.
//!
//! Every entry point exchanges UTF-8 JSON: the record JSON is the one emitted
//! by `nirs4all-formats read-json` and by the Python and WASM bindings. The
//! functions that can fail return an [`n4fmt_status_t`] code, write their
//! result to `*out_json` on success and, when `out_error` is not null, a
//! message to `*out_error` on failure. Every returned string is owned by the
//! caller and released with [`n4fmt_string_free`]. Panics never cross the
//! boundary: they are reported as [`N4FMT_STATUS_INTERNAL`].

use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use base64::Engine;
use nirs4all_formats::{
    builtin_probes, export_targets, open_bytes_with_options, open_path_with_options,
    plan_conversion, probe_path, reader_catalog, write_records, AppliedConversion, CubeMask,
    CubeWindow, Error, FormatProbe, PlanOptions, ReadOptions, SpectralRecord, WriteError,
    WriteOptions, WriteOutput,
};
use serde::{Deserialize, Serialize};

/// ABI version of the C surface, independent from the crate semver.
///
/// `0.2.0` adds the JSON read, export, convert and conversion-planning entry
/// points to the `0.1.0` scaffold; every `0.1.0` symbol is unchanged.
pub const N4FMT_ABI_VERSION: &str = "0.2.0";

/// Status code returned by the fallible entry points.
#[allow(non_camel_case_types)]
pub type n4fmt_status_t = i32;

/// The call succeeded and `*out_json` holds the result.
pub const N4FMT_STATUS_OK: n4fmt_status_t = 0;
/// A required pointer is null, a string is not UTF-8, or the options or
/// records JSON is invalid.
pub const N4FMT_STATUS_INVALID_ARGUMENT: n4fmt_status_t = 1;
/// A file could not be read or saved.
pub const N4FMT_STATUS_IO: n4fmt_status_t = 2;
/// No reader recognizes the input.
pub const N4FMT_STATUS_UNSUPPORTED_FORMAT: n4fmt_status_t = 3;
/// The format needs a companion file that is not available on this path.
pub const N4FMT_STATUS_UNSUPPORTED_SIDECAR: n4fmt_status_t = 4;
/// The input was recognized but its content cannot be decoded.
pub const N4FMT_STATUS_INVALID_DATA: n4fmt_status_t = 5;
/// The target is neither an export format nor a software preset.
pub const N4FMT_STATUS_UNKNOWN_TARGET: n4fmt_status_t = 6;
/// The records or options cannot be represented in the target.
pub const N4FMT_STATUS_EXPORT_REFUSED: n4fmt_status_t = 7;
/// A panic was caught inside the library; this is a bug.
pub const N4FMT_STATUS_INTERNAL: n4fmt_status_t = 99;

/// Return the C ABI version string.
#[no_mangle]
pub extern "C" fn n4fmt_abi_version() -> *mut c_char {
    owned(N4FMT_ABI_VERSION)
}

/// Return the `nirs4all-formats` library version string.
#[no_mangle]
pub extern "C" fn n4fmt_library_version() -> *mut c_char {
    owned(env!("CARGO_PKG_VERSION"))
}

/// Free strings returned by this ABI.
///
/// # Safety
///
/// `ptr` must either be null or a pointer previously returned by a
/// `nirs4all-formats` C ABI function that transfers string ownership to the caller.
/// It must not be freed more than once.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Placeholder hook used by bindings to detect that the native core is loaded.
#[no_mangle]
pub extern "C" fn n4fmt_core_is_available() -> bool {
    true
}

/// List the readers compiled into this library as `[{"reader": "..."}]`.
///
/// # Safety
///
/// `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_reader_catalog(
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || to_json(&reader_catalog()))
}

/// Probe a file and return every candidate reader, most confident first, as
/// `[{"format", "reader", "confidence", "reason"}]`.
///
/// # Safety
///
/// `path` must be null or a NUL-terminated string; `out_json` must be a valid
/// pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_probe_path(
    path: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let path = required_str(path, "path")?;
        to_json(&probe_path(path).map_err(Failure::from)?)
    })
}

/// Probe an in-memory file. `name` is the file name, used by extension-aware
/// sniffers; only the first 8192 bytes are inspected.
///
/// # Safety
///
/// `name` must be null or a NUL-terminated string; `bytes` must point to `len`
/// readable bytes (it may be null when `len` is 0); `out_json` must be a valid
/// pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_probe_bytes(
    name: *const c_char,
    bytes: *const u8,
    len: usize,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let name = required_str(name, "name")?;
        let bytes = byte_slice(bytes, len)?;
        let mut probes = builtin_probes(&bytes[..bytes.len().min(8192)], Path::new(name));
        sort_probes(&mut probes);
        to_json(&probes)
    })
}

/// Read a file and return its records as a JSON array.
///
/// `options_json` may be null or a JSON object with any of `single_record`
/// (bool), `rows` and `cols` (`[start, end]` half-open windows, `end` may be
/// null) or `pixels` (`[[row, col], ...]`); windows and pixels are exclusive.
/// Companion files of sidecar formats are resolved next to `path`.
///
/// # Safety
///
/// `path` and `options_json` must be null or NUL-terminated strings;
/// `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_open_path(
    path: *const c_char,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let path = required_str(path, "path")?;
        let options = read_options(optional_str(options_json, "options_json")?)?;
        to_json(&open_path_with_options(path, &options).map_err(Failure::from)?)
    })
}

/// Read an in-memory file and return its records as a JSON array.
///
/// `name` is the file name, used for sniffing and provenance. Options are
/// those of [`n4fmt_open_path`]. Formats that need a companion file fail with
/// [`N4FMT_STATUS_UNSUPPORTED_SIDECAR`]; read them with [`n4fmt_open_path`].
///
/// # Safety
///
/// `name` and `options_json` must be null or NUL-terminated strings; `bytes`
/// must point to `len` readable bytes (it may be null when `len` is 0);
/// `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_open_bytes(
    name: *const c_char,
    bytes: *const u8,
    len: usize,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let name = required_str(name, "name")?;
        let bytes = byte_slice(bytes, len)?;
        let options = read_options(optional_str(options_json, "options_json")?)?;
        to_json(&open_bytes_with_options(name, bytes, &options).map_err(Failure::from)?)
    })
}

/// Return the export formats, software presets and per-format requirements
/// as `{"formats": [...], "software": [...], "requirements": [...]}`, the
/// listing every binding returns.
///
/// # Safety
///
/// `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_export_targets(
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || to_json(&export_targets()))
}

/// Plan the conversions that make records writable for a target.
///
/// `records_json` is a JSON array of records; `target` is an export format,
/// a software preset, or null to plan the options alone (for example
/// `{"signal_type": "absorbance"}`). `options_json` may be null or a JSON
/// object of plan options (`signal`, `signal_type`, `axis_unit`, `grid`,
/// `grid_step`, `grid_min`, `grid_max`, `interpolation`, `source_scale`,
/// `assume_signal_type`, `absorbance_origin`, `axis_tolerance`, `splices`).
/// The result is the plan: `status` (`ready`, `needs_answers`, `refused` or
/// `unchanged`), the `steps` naming the nirs4all-methods method that computes
/// each, the `questions` to answer through the options and the `refusals`.
/// The plan computes no values.
///
/// # Safety
///
/// `records_json`, `target` and `options_json` must be null or NUL-terminated
/// strings; `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_plan_conversion(
    records_json: *const c_char,
    target: *const c_char,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let records = parse_records(required_str(records_json, "records_json")?)?;
        let target = optional_str(target, "target")?;
        let options: PlanOptions = serde_json::from_value(serde_json::Value::Object(parse_object(
            optional_str(options_json, "options_json")?,
        )?))
        .map_err(|error| invalid(format!("options_json: {error}")))?;
        to_json(&plan_conversion(&records, target, &options).map_err(Failure::from)?)
    })
}

/// Write records to an export format or software preset, in memory.
///
/// `records_json` is a JSON array of records as returned by the open
/// functions. `options_json` may be null or a JSON object with any of
/// `signal`, `base_name`, `extension`, `axis_tolerance`, `multi`, `delimiter`
/// and `zip`. The result is `{"format", "files": [{"name", "media_type",
/// "size", "bytes_base64"}], "losses": [{"code", "message", "count",
/// "max_error"?}], "conversions": [...]}`, where `conversions` lists the
/// conversion steps recorded in the records' metadata; nothing is written to
/// disk.
///
/// # Safety
///
/// `records_json`, `target` and `options_json` must be null or NUL-terminated
/// strings; `out_json` must be a valid pointer; `out_error` may be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_write_records(
    records_json: *const c_char,
    target: *const c_char,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let records = parse_records(required_str(records_json, "records_json")?)?;
        let target = required_str(target, "target")?;
        let options = write_options(optional_str(options_json, "options_json")?)?;
        let output = write_records(&records, target, &options).map_err(Failure::from)?;
        to_json(&MemoryOutput::from(&output))
    })
}

/// Read a file and save its export into an existing directory.
///
/// `options_json` takes the options of [`n4fmt_write_records`] plus
/// `overwrite` (bool, default false: nothing is written when any produced
/// file already exists). The result is `{"format", "files": [paths], "conversions",
/// "losses": [...]}`, as printed by `nirs4all-formats convert`.
///
/// # Safety
///
/// `input_path`, `target`, `output_dir` and `options_json` must be null or
/// NUL-terminated strings; `out_json` must be a valid pointer; `out_error` may
/// be null.
#[no_mangle]
pub unsafe extern "C" fn n4fmt_convert_path(
    input_path: *const c_char,
    target: *const c_char,
    output_dir: *const c_char,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> n4fmt_status_t {
    run(out_json, out_error, || {
        let input_path = required_str(input_path, "input_path")?;
        let target = required_str(target, "target")?;
        let output_dir = required_str(output_dir, "output_dir")?;
        let mut options = parse_object(optional_str(options_json, "options_json")?)?;
        let overwrite = match options.remove("overwrite") {
            None | Some(serde_json::Value::Null) => false,
            Some(serde_json::Value::Bool(value)) => value,
            Some(_) => return Err(invalid("options_json: overwrite must be a boolean")),
        };
        let options: WriteOptions = serde_json::from_value(serde_json::Value::Object(options))
            .map_err(|error| invalid(format!("options_json: {error}")))?;
        let records = nirs4all_formats::open_path(input_path).map_err(Failure::from)?;
        let output = write_records(&records, target, &options).map_err(Failure::from)?;
        let files = output.save(output_dir, overwrite).map_err(Failure::from)?;
        to_json(&SavedOutput {
            format: output.format,
            files,
            losses: &output.losses,
            conversions: &output.conversions,
        })
    })
}

/// Reserved opaque handle type for future records/collections.
#[repr(C)]
pub struct n4fmt_handle_t {
    _private: [u8; 0],
    _marker: std::marker::PhantomData<(*mut c_void, std::marker::PhantomPinned)>,
}

/// A failed call: status code and message.
struct Failure {
    status: n4fmt_status_t,
    message: String,
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure {
        status: N4FMT_STATUS_INVALID_ARGUMENT,
        message: message.into(),
    }
}

impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        let status = match &error {
            Error::UnsupportedFormat { .. } | Error::AmbiguousFormat { .. } => {
                N4FMT_STATUS_UNSUPPORTED_FORMAT
            }
            Error::Io { .. } => N4FMT_STATUS_IO,
            Error::UnsupportedSidecar { .. } => N4FMT_STATUS_UNSUPPORTED_SIDECAR,
            Error::InvalidRecord(_) | Error::Json(_) => N4FMT_STATUS_INVALID_DATA,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

impl From<WriteError> for Failure {
    fn from(error: WriteError) -> Self {
        let status = match &error {
            WriteError::UnknownTarget(_) => N4FMT_STATUS_UNKNOWN_TARGET,
            WriteError::Refused(_) => N4FMT_STATUS_EXPORT_REFUSED,
            WriteError::Io { .. } => N4FMT_STATUS_IO,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}

/// Run `body`, translating its result and any panic into the C contract.
fn run(
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
    body: impl FnOnce() -> Result<String, Failure>,
) -> n4fmt_status_t {
    // SAFETY: the caller passes valid (or null) out-pointers, per each entry
    // point's contract.
    unsafe {
        if !out_error.is_null() {
            *out_error = std::ptr::null_mut();
        }
        if out_json.is_null() {
            set_error(out_error, "out_json must not be null");
            return N4FMT_STATUS_INVALID_ARGUMENT;
        }
        *out_json = std::ptr::null_mut();
    }
    let result = catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|payload| {
        let detail = payload
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        Err(Failure {
            status: N4FMT_STATUS_INTERNAL,
            message: format!("internal error: {detail}"),
        })
    });
    match result {
        Ok(json) => {
            // SAFETY: `out_json` was checked non-null above.
            unsafe { *out_json = owned(&json) };
            N4FMT_STATUS_OK
        }
        Err(failure) => {
            // SAFETY: `out_error` is null or valid, per the contract.
            unsafe { set_error(out_error, &failure.message) };
            failure.status
        }
    }
}

unsafe fn set_error(out_error: *mut *mut c_char, message: &str) {
    if !out_error.is_null() {
        *out_error = owned(message);
    }
}

/// Copy `text` into a caller-owned C string, replacing interior NULs.
fn owned(text: &str) -> *mut c_char {
    CString::new(text.replace('\0', "\u{FFFD}"))
        .expect("interior NULs were replaced")
        .into_raw()
}

unsafe fn optional_str<'a>(ptr: *const c_char, name: &str) -> Result<Option<&'a str>, Failure> {
    if ptr.is_null() {
        return Ok(None);
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map(Some)
        .map_err(|_| invalid(format!("{name} is not valid UTF-8")))
}

unsafe fn required_str<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, Failure> {
    optional_str(ptr, name)?.ok_or_else(|| invalid(format!("{name} must not be null")))
}

unsafe fn byte_slice<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], Failure> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(invalid("bytes must not be null when len > 0"));
    }
    Ok(std::slice::from_raw_parts(ptr, len))
}

fn to_json<T: Serialize + ?Sized>(value: &T) -> Result<String, Failure> {
    serde_json::to_string(value).map_err(|error| Failure {
        status: N4FMT_STATUS_INTERNAL,
        message: format!("cannot serialize the result: {error}"),
    })
}

fn sort_probes(probes: &mut [FormatProbe]) {
    probes.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then(a.format.cmp(&b.format))
    });
}

fn parse_object(text: Option<&str>) -> Result<serde_json::Map<String, serde_json::Value>, Failure> {
    let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
        return Ok(serde_json::Map::new());
    };
    match serde_json::from_str(text) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(serde_json::Value::Null) => Ok(serde_json::Map::new()),
        Ok(_) => Err(invalid("options_json must be a JSON object")),
        Err(error) => Err(invalid(format!("options_json: {error}"))),
    }
}

fn parse_records(text: &str) -> Result<Vec<SpectralRecord>, Failure> {
    serde_json::from_str(text).map_err(|error| invalid(format!("records_json: {error}")))
}

fn write_options(text: Option<&str>) -> Result<WriteOptions, Failure> {
    serde_json::from_value(serde_json::Value::Object(parse_object(text)?))
        .map_err(|error| invalid(format!("options_json: {error}")))
}

/// Read options accepted as JSON, mirroring the Python keyword arguments.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReadOptionsJson {
    single_record: bool,
    rows: Option<(usize, Option<usize>)>,
    cols: Option<(usize, Option<usize>)>,
    pixels: Option<Vec<(usize, usize)>>,
}

fn read_options(text: Option<&str>) -> Result<ReadOptions, Failure> {
    let parsed: ReadOptionsJson =
        serde_json::from_value(serde_json::Value::Object(parse_object(text)?))
            .map_err(|error| invalid(format!("options_json: {error}")))?;
    let has_window = parsed.rows.is_some() || parsed.cols.is_some();
    let mut options = match parsed.pixels {
        Some(_) if has_window => {
            return Err(invalid(
                "options_json: rows/cols cannot be combined with pixels",
            ))
        }
        Some(pixels) => ReadOptions::default().with_cube_mask(CubeMask::new(pixels)),
        None if has_window => {
            let (row_start, row_end) = parsed.rows.unwrap_or((0, None));
            let (col_start, col_end) = parsed.cols.unwrap_or((0, None));
            ReadOptions::default()
                .with_cube_window(CubeWindow::new(row_start, row_end, col_start, col_end))
        }
        None => ReadOptions::default(),
    };
    if parsed.single_record {
        options = options.single_record();
    }
    Ok(options)
}

#[derive(Serialize)]
struct MemoryOutput<'a> {
    format: &'a str,
    files: Vec<MemoryFile<'a>>,
    losses: &'a [nirs4all_formats::WriteLoss],
    conversions: &'a [AppliedConversion],
}

#[derive(Serialize)]
struct MemoryFile<'a> {
    name: &'a str,
    media_type: &'a str,
    size: usize,
    bytes_base64: String,
}

impl<'a> From<&'a WriteOutput> for MemoryOutput<'a> {
    fn from(output: &'a WriteOutput) -> Self {
        Self {
            format: output.format,
            files: output
                .files
                .iter()
                .map(|file| MemoryFile {
                    name: &file.name,
                    media_type: file.media_type,
                    size: file.bytes.len(),
                    bytes_base64: base64::engine::general_purpose::STANDARD.encode(&file.bytes),
                })
                .collect(),
            losses: &output.losses,
            conversions: &output.conversions,
        }
    }
}

#[derive(Serialize)]
struct SavedOutput<'a> {
    format: &'a str,
    files: Vec<PathBuf>,
    losses: &'a [nirs4all_formats::WriteLoss],
    conversions: &'a [AppliedConversion],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    fn sample(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn take(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        // SAFETY: `ptr` comes from this ABI and is freed once here.
        unsafe {
            let text = CStr::from_ptr(ptr).to_str().unwrap().to_string();
            n4fmt_string_free(ptr);
            text
        }
    }

    /// Call `f(out_json, out_error)`, returning the status and the owned text.
    fn call(
        f: impl FnOnce(*mut *mut c_char, *mut *mut c_char) -> n4fmt_status_t,
    ) -> (n4fmt_status_t, String) {
        let mut out = ptr::null_mut();
        let mut err = ptr::null_mut();
        let status = f(&mut out, &mut err);
        if status == N4FMT_STATUS_OK {
            assert!(err.is_null());
            (status, take(out))
        } else {
            assert!(out.is_null());
            (status, take(err))
        }
    }

    fn cstring(text: &str) -> CString {
        CString::new(text).unwrap()
    }

    #[test]
    fn versions() {
        assert_eq!(take(n4fmt_abi_version()), "0.2.0");
        assert_eq!(take(n4fmt_library_version()), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn reader_catalog_lists_jcamp_reader() {
        let (status, json) = call(|o, e| unsafe { n4fmt_reader_catalog(o, e) });
        assert_eq!(status, N4FMT_STATUS_OK);
        let readers: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert!(
            readers
                .iter()
                .any(|r| r["reader"] == "nirs4all_formats::readers::jcamp"),
            "{json}"
        );
    }

    #[test]
    fn opens_path_and_bytes_identically() {
        let path = sample("samples/jcamp_dx/TESTSPEC.DX");
        let c_path = cstring(path.to_str().unwrap());
        let (status, from_path) =
            call(|o, e| unsafe { n4fmt_open_path(c_path.as_ptr(), ptr::null(), o, e) });
        assert_eq!(status, N4FMT_STATUS_OK, "{from_path}");
        let bytes = std::fs::read(&path).unwrap();
        let name = cstring("TESTSPEC.DX");
        let options = cstring("{}");
        let (status, from_bytes) = call(|o, e| unsafe {
            n4fmt_open_bytes(
                name.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
                options.as_ptr(),
                o,
                e,
            )
        });
        assert_eq!(status, N4FMT_STATUS_OK, "{from_bytes}");
        let a: Vec<SpectralRecord> = serde_json::from_str(&from_path).unwrap();
        let b: Vec<SpectralRecord> = serde_json::from_str(&from_bytes).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].signals, b[0].signals);
        assert_eq!(a[0].provenance.format, "jcamp-dx");
    }

    #[test]
    fn probes_bytes_most_confident_first() {
        let head = b"##TITLE=Water\n##JCAMP-DX=5.01\n";
        let name = cstring("x.txt");
        let (status, json) = call(|o, e| unsafe {
            n4fmt_probe_bytes(name.as_ptr(), head.as_ptr(), head.len(), o, e)
        });
        assert_eq!(status, N4FMT_STATUS_OK);
        let probes: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(probes[0]["format"], "jcamp-dx");
    }

    #[test]
    fn reports_errors_with_status_and_message() {
        let missing = cstring("/nonexistent/file.dx");
        let (status, message) =
            call(|o, e| unsafe { n4fmt_open_path(missing.as_ptr(), ptr::null(), o, e) });
        assert_eq!(status, N4FMT_STATUS_IO);
        assert!(message.contains("/nonexistent/file.dx"), "{message}");

        let (status, message) =
            call(|o, e| unsafe { n4fmt_open_path(ptr::null(), ptr::null(), o, e) });
        assert_eq!(status, N4FMT_STATUS_INVALID_ARGUMENT);
        assert!(message.contains("path"), "{message}");

        let path = cstring(sample("samples/jcamp_dx/TESTSPEC.DX").to_str().unwrap());
        let bad = cstring(r#"{"single_recod": true}"#);
        let (status, message) =
            call(|o, e| unsafe { n4fmt_open_path(path.as_ptr(), bad.as_ptr(), o, e) });
        assert_eq!(status, N4FMT_STATUS_INVALID_ARGUMENT);
        assert!(message.contains("single_recod"), "{message}");

        let unknown = b"not a spectrum at all";
        let name = cstring("mystery.bin");
        let (status, _) = call(|o, e| unsafe {
            n4fmt_open_bytes(
                name.as_ptr(),
                unknown.as_ptr(),
                unknown.len(),
                ptr::null(),
                o,
                e,
            )
        });
        assert_eq!(status, N4FMT_STATUS_UNSUPPORTED_FORMAT);

        // A null error pointer is allowed; the status still reports the failure.
        let mut out = ptr::null_mut();
        let status =
            unsafe { n4fmt_open_path(missing.as_ptr(), ptr::null(), &mut out, ptr::null_mut()) };
        assert_eq!(status, N4FMT_STATUS_IO);
        assert!(out.is_null());
        // A null result pointer is refused.
        let status = unsafe { n4fmt_export_targets(ptr::null_mut(), ptr::null_mut()) };
        assert_eq!(status, N4FMT_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn records_json_round_trips_values_bit_for_bit() {
        let records =
            nirs4all_formats::open_path(sample("samples/csv_tsv/synthetic_nirs.csv")).unwrap();
        let back = parse_records(&serde_json::to_string(&records).unwrap())
            .unwrap_or_else(|failure| panic!("{}", failure.message));
        for (a, b) in records.iter().zip(&back) {
            let (a, b) = (&a.signals["signal"].values, &b.signals["signal"].values);
            assert!(a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()));
        }
    }

    fn plan(file: &str, target: Option<&str>, options: &str) -> (n4fmt_status_t, String) {
        let path = cstring(sample(file).to_str().unwrap());
        let (status, records) =
            call(|o, e| unsafe { n4fmt_open_path(path.as_ptr(), ptr::null(), o, e) });
        assert_eq!(status, N4FMT_STATUS_OK, "{records}");
        let records = cstring(&records);
        let target = target.map(cstring);
        let options = cstring(options);
        call(|o, e| unsafe {
            n4fmt_plan_conversion(
                records.as_ptr(),
                target.as_ref().map_or(ptr::null(), |t| t.as_ptr()),
                options.as_ptr(),
                o,
                e,
            )
        })
    }

    #[test]
    fn plans_conversions() {
        let (status, json) = plan(
            "samples/ocean_optics/whiteref.ProcSpec",
            Some("winisi"),
            "{}",
        );
        assert_eq!(status, N4FMT_STATUS_OK, "{json}");
        let ready: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(ready["status"], "ready");
        assert!(ready["steps"][0]["method_id"].is_string(), "{json}");

        let (_, json) = plan("samples/jcamp_dx/SPECFILE.DX", Some("winisi"), "");
        let asks: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(asks["status"], "needs_answers");
        assert_eq!(asks["questions"][0]["id"], "source_scale");

        let (_, json) = plan(
            "samples/raman_horiba/jobinyvon_test_spec_3s_cm-1.xml",
            None,
            r#"{"axis_unit": "nm"}"#,
        );
        let refused: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(refused["status"], "refused");
        assert_eq!(refused["refusals"][0]["code"], "raman_shift");

        let (status, _) = plan("samples/jcamp_dx/SPECFILE.DX", Some("no-such-target"), "{}");
        assert_eq!(status, N4FMT_STATUS_UNKNOWN_TARGET);
        let (status, message) = plan("samples/jcamp_dx/SPECFILE.DX", None, r#"{"grid": 3}"#);
        assert_eq!(status, N4FMT_STATUS_INVALID_ARGUMENT, "{message}");
    }

    #[test]
    fn export_targets_include_requirements() {
        let (status, json) = call(|o, e| unsafe { n4fmt_export_targets(o, e) });
        assert_eq!(status, N4FMT_STATUS_OK);
        let targets: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(targets, nirs4all_formats::export_targets());
        assert!(!targets["requirements"].as_array().unwrap().is_empty());
    }

    #[test]
    fn catches_panics() {
        let mut out = ptr::null_mut();
        let mut err = ptr::null_mut();
        let status = run(&mut out, &mut err, || panic!("boom"));
        assert_eq!(status, N4FMT_STATUS_INTERNAL);
        assert!(out.is_null());
        assert!(take(err).contains("boom"));
    }

    #[test]
    fn writes_records_round_trip_through_jcamp() {
        let path = cstring(sample("samples/jcamp_dx/TESTSPEC.DX").to_str().unwrap());
        let (_, records) =
            call(|o, e| unsafe { n4fmt_open_path(path.as_ptr(), ptr::null(), o, e) });
        let records_c = cstring(&records);
        let target = cstring("jcamp-dx");
        let options = cstring(r#"{"base_name": "roundtrip"}"#);
        let (status, json) = call(|o, e| unsafe {
            n4fmt_write_records(records_c.as_ptr(), target.as_ptr(), options.as_ptr(), o, e)
        });
        assert_eq!(status, N4FMT_STATUS_OK, "{json}");
        let output: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(output["format"], "jcamp-dx");
        let file = &output["files"][0];
        assert_eq!(file["name"], "roundtrip.jdx");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(file["bytes_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(file["size"], bytes.len());
        let back = nirs4all_formats::open_bytes("roundtrip.jdx", &bytes).unwrap();
        let original: Vec<SpectralRecord> = serde_json::from_str(&records).unwrap();
        let (a, b) = (
            original[0].signals.values().next().unwrap(),
            back[0].signals.values().next().unwrap(),
        );
        assert_eq!(a.axis.values.len(), b.axis.values.len());
        assert!(output["losses"].is_array());

        let unknown = cstring("no-such-target");
        let (status, message) = call(|o, e| unsafe {
            n4fmt_write_records(records_c.as_ptr(), unknown.as_ptr(), ptr::null(), o, e)
        });
        assert_eq!(status, N4FMT_STATUS_UNKNOWN_TARGET);
        assert!(message.contains("no-such-target"));

        let not_records = cstring("[{}]");
        let (status, _) = call(|o, e| unsafe {
            n4fmt_write_records(not_records.as_ptr(), target.as_ptr(), ptr::null(), o, e)
        });
        assert_eq!(status, N4FMT_STATUS_INVALID_ARGUMENT);
    }

    #[test]
    fn converts_path_into_directory() {
        let dir = std::env::temp_dir().join(format!("n4fmt-capi-convert-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = cstring(
            sample("samples/csv_tsv/synthetic_nirs.csv")
                .to_str()
                .unwrap(),
        );
        let target = cstring("csv");
        let out_dir = cstring(dir.to_str().unwrap());
        let options = cstring(r#"{"base_name": "converted"}"#);
        let (status, json) = call(|o, e| unsafe {
            n4fmt_convert_path(
                input.as_ptr(),
                target.as_ptr(),
                out_dir.as_ptr(),
                options.as_ptr(),
                o,
                e,
            )
        });
        assert_eq!(status, N4FMT_STATUS_OK, "{json}");
        let report: serde_json::Value = serde_json::from_str(&json).unwrap();
        let first = report["files"][0].as_str().unwrap();
        assert!(Path::new(first).exists());

        // Existing files are kept unless overwrite is set.
        let (status, _) = call(|o, e| unsafe {
            n4fmt_convert_path(
                input.as_ptr(),
                target.as_ptr(),
                out_dir.as_ptr(),
                options.as_ptr(),
                o,
                e,
            )
        });
        assert_eq!(status, N4FMT_STATUS_IO);
        let overwrite = cstring(r#"{"base_name": "converted", "overwrite": true}"#);
        let (status, _) = call(|o, e| unsafe {
            n4fmt_convert_path(
                input.as_ptr(),
                target.as_ptr(),
                out_dir.as_ptr(),
                overwrite.as_ptr(),
                o,
                e,
            )
        });
        assert_eq!(status, N4FMT_STATUS_OK);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
