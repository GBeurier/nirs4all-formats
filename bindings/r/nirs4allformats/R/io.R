#' Read a NIRS/spectroscopy file into raw records
#'
#' @title Read a spectroscopy file into normalized records
#' @description
#' Decodes a single file through the Rust `nirs4all-formats` registry and returns the
#' normalized records exactly as the core emits them, as nested R lists. Format
#' detection is content-based: the file is sniffed and dispatched to the
#' highest-confidence reader. No reshaping, alignment or column-building is done
#' here -- this is the faithful, lossless view of the Rust `SpectralRecord`
#' model. For a flat spectral matrix use [nirs4allformats_open_dataset()].
#'
#' Parser logic lives only in Rust; this function never parses bytes itself. It
#' dispatches through the native extendr library when available and otherwise
#' through the `nirs4all-formats` CLI (see *Transport* in [nirs4allformats_open_dataset()]).
#'
#' @param path Character scalar. Path to the input file. It is resolved with
#'   [normalizePath()] (`mustWork = TRUE`), so the file must exist.
#'
#' @return A list of records. Each record is a named list mirroring the Rust
#'   `SpectralRecord`:
#'   \describe{
#'     \item{`signals`}{Named list of signal channels. Each channel carries
#'       `values` (flat C-order buffer), `shape`, `dims` (exactly one is `"x"`),
#'       optional `coords`, `signal_type`, `unit`, `role`, `source` and an
#'       `axis` (`values`, `unit`, `kind`, `order`).}
#'     \item{`signal_type`}{Record-level signal type (e.g. `"absorbance"`,
#'       `"reflectance"`, `"unknown"`).}
#'     \item{`targets`}{Named list of reference values parsed from the file.}
#'     \item{`metadata`}{Named list of typed metadata key/value pairs.}
#'     \item{`provenance`}{Reader name/version, per-source SHA-256 (`sources`),
#'       `format`, `record_schema_version` and `warnings`.}
#'     \item{`quality_flags`}{Character vector of quality annotations.}
#'   }
#'
#' @examples
#' \dontrun{
#' records <- nirs4allformats_open_records("samples/csv_tsv/synthetic_nirs.csv")
#' length(records)
#' records[[1]]$provenance$format
#' names(records[[1]]$signals)
#' }
#'
#' @seealso [nirs4allformats_open_dataset()] for a flat matrix view,
#'   [nirs4allformats_probe_path()] to inspect candidate readers,
#'   [nirs4allformats_walk_path()] to scan a directory.
#' @export
nirs4allformats_open_records <- function(path) {
  payload <- nirs4allformats_run_reader(path)
  records <- jsonlite::fromJSON(payload, simplifyVector = FALSE)
  if (!is.list(records)) {
    stop("Rust reader returned a non-list JSON payload", call. = FALSE)
  }
  records
}

#' Read a file into an `nirs4allformats_dataset`
#'
#' @title Read a spectroscopy file into a flat spectral dataset
#' @description
#' Loads a file with [nirs4allformats_open_records()] and projects one signal per
#' record into a rectangular, R-friendly dataset: a samples-by-wavelengths
#' matrix plus sample IDs, targets and metadata. All records must share the same
#' spectral axis (an error is raised otherwise), so this is intended for a
#' homogeneous set of spectra. For heterogeneous or N-dimensional data, work
#' from [nirs4allformats_open_records()] directly.
#'
#' Parsing happens only in Rust; the R layer just selects a signal and reshapes
#' the JSON the core returns.
#'
#' @section Signal selection:
#' When `signal` is `NULL` the channel is chosen per record in this order:
#' \enumerate{
#'   \item the first signal whose `signal_type` equals the record-level
#'     `signal_type`;
#'   \item otherwise the first present of `"reflectance"`, `"absorbance"`,
#'     `"transmittance"`, `"signal"`;
#'   \item otherwise the alphabetically first signal name.
#' }
#' Passing an explicit `signal` name selects that channel and errors if a record
#' lacks it.
#'
#' @section Sample IDs:
#' Each row's identifier is taken from `metadata$sample_id` when present;
#' otherwise it is derived from the source file basename and 0-based row index
#' (`"<basename>:<i>"`), falling back to `"record:<i>"` when no source path is
#' known.
#'
#' @section Targets and metadata:
#' Reference values found under each record's `targets` are gathered into a
#' `data.frame` (missing values become `NA`). The full per-record metadata
#' lists are preserved verbatim in the `metadata` field of the returned object.
#'
#' @section Transport:
#' The call is served by the native extendr static library when it is present
#' (built by `R CMD INSTALL` with Cargo on `PATH`). Otherwise it shells out to
#' the `nirs4all-formats` CLI: the `NIRS4ALL_FORMATS_CLI` environment variable may point
#' to a prebuilt binary (it is whitespace-split into command + arguments), a
#' `nirs4all-formats` binary on `PATH` is used if found, and in a source checkout it
#' falls back to `cargo run -p nirs4all-formats-cli`.
#'
#' @param path Character scalar. Path to the input file (resolved with
#'   [normalizePath()], `mustWork = TRUE`).
#' @param signal Optional character scalar naming the signal channel to project.
#'   When `NULL` (default) the channel is auto-selected (see *Signal selection*).
#'
#' @return An object of class `nirs4allformats_dataset`: a named list with
#'   \describe{
#'     \item{`x`}{Numeric matrix of spectra, `n_samples` x `n_wavelengths`.}
#'     \item{`wavelengths`}{Numeric vector of axis coordinates (length
#'       `n_wavelengths`).}
#'     \item{`targets`}{`data.frame` of reference values, one column per target
#'       key (zero columns when none were parsed).}
#'     \item{`sample_ids`}{Character vector of per-row identifiers.}
#'     \item{`metadata`}{List of per-record metadata lists.}
#'     \item{`signal_type`}{Signal type of the selected channel.}
#'     \item{`axis_unit`}{Unit string of the spectral axis (e.g. `"nm"`).}
#'     \item{`formats`}{Character vector of the source format per row.}
#'   }
#'   Use [as.matrix()] / [as.data.frame()] / [nirs4allformats_as_tibble()] to project
#'   it into common R shapes.
#'
#' @examples
#' \dontrun{
#' ds <- nirs4allformats_open_dataset("samples/csv_tsv/synthetic_nirs.csv")
#' dim(as.matrix(ds))
#' head(as.data.frame(ds))
#' ds$wavelengths[1:5]
#'
#' # Select a specific channel by name
#' ds_abs <- nirs4allformats_open_dataset("spectrum.dx", signal = "absorbance")
#' }
#'
#' @seealso [nirs4allformats_open_records()] for the lossless record view,
#'   [as.matrix.nirs4allformats_dataset()], [as.data.frame.nirs4allformats_dataset()],
#'   [nirs4allformats_as_tibble()].
#' @export
nirs4allformats_open_dataset <- function(path, signal = NULL) {
  records <- nirs4allformats_open_records(path)
  if (length(records) == 0) {
    stop("Rust reader returned no records", call. = FALSE)
  }

  rows <- list()
  sample_ids <- character()
  metadata <- list()
  formats <- character()
  targets <- list()
  wavelengths <- NULL
  axis_unit <- "index"
  signal_type <- "unknown"

  for (row_index in seq_along(records)) {
    record <- records[[row_index]]
    selected <- nirs4allformats_select_signal(record, signal)
    signal_payload <- selected$payload
    values <- as.numeric(unlist(signal_payload$values, use.names = FALSE))
    axis_values <- as.numeric(unlist(signal_payload$axis$values, use.names = FALSE))
    if (length(values) == 0 || length(axis_values) == 0) {
      stop(sprintf("Record %d contains an empty signal", row_index), call. = FALSE)
    }
    if (length(values) != length(axis_values)) {
      stop(sprintf("Record %d has mismatched axis length", row_index), call. = FALSE)
    }
    if (is.null(wavelengths)) {
      wavelengths <- axis_values
      axis_unit <- signal_payload$axis$unit %||% "index"
      signal_type <- signal_payload$signal_type %||% record$signal_type %||% "unknown"
    } else if (!identical(axis_values, wavelengths)) {
      stop("Cannot build one dataset from records with different axes", call. = FALSE)
    }

    rows[[row_index]] <- values
    metadata[[row_index]] <- record$metadata %||% list()
    sample_ids[[row_index]] <- nirs4allformats_sample_id(record, metadata[[row_index]], row_index)
    formats[[row_index]] <- record$provenance$format %||% "unknown"

    record_targets <- record$targets %||% list()
    for (key in names(targets)) {
      targets[[key]][row_index] <- list(record_targets[[key]])
    }
    for (key in names(record_targets)) {
      if (is.null(targets[[key]])) {
        targets[[key]] <- vector("list", length(records))
      }
      targets[[key]][row_index] <- list(record_targets[[key]])
    }
  }

  target_frame <- as.data.frame(
    lapply(targets, nirs4allformats_flatten_column),
    optional = TRUE,
    stringsAsFactors = FALSE
  )
  structure(
    list(
      x = do.call(rbind, rows),
      wavelengths = wavelengths,
      targets = target_frame,
      sample_ids = unlist(sample_ids, use.names = FALSE),
      metadata = metadata,
      signal_type = signal_type,
      axis_unit = axis_unit,
      formats = unlist(formats, use.names = FALSE)
    ),
    class = "nirs4allformats_dataset"
  )
}

#' Coerce an `nirs4allformats_dataset` to a spectral matrix
#'
#' @title Extract the spectral matrix from a dataset
#' @description
#' [as.matrix()] method for [nirs4allformats_dataset][nirs4allformats_open_dataset]
#' objects. Returns the stored `n_samples` x `n_wavelengths` numeric matrix of
#' spectra. Rows correspond to `x$sample_ids` and columns to `x$wavelengths`.
#'
#' @param x An `nirs4allformats_dataset` from [nirs4allformats_open_dataset()].
#' @param ... Ignored; present for S3 method consistency.
#'
#' @return A numeric matrix with one row per sample and one column per
#'   wavelength.
#'
#' @examples
#' \dontrun{
#' ds <- nirs4allformats_open_dataset("samples/csv_tsv/synthetic_nirs.csv")
#' m <- as.matrix(ds)
#' dim(m)
#' }
#'
#' @seealso [nirs4allformats_open_dataset()], [as.data.frame.nirs4allformats_dataset()].
#' @exportS3Method base::as.matrix
as.matrix.nirs4allformats_dataset <- function(x, ...) {
  x$x
}

#' Coerce an `nirs4allformats_dataset` to a data frame
#'
#' @title Build a wide data frame from a dataset
#' @description
#' [as.data.frame()] method for [nirs4allformats_dataset][nirs4allformats_open_dataset]
#' objects. Returns a wide table whose first column is `sample_id`, followed by
#' any target columns, followed by one spectral column per wavelength. Spectral
#' columns are named `x_<wavelength>` (the axis value formatted without
#' scientific notation).
#'
#' @param x An `nirs4allformats_dataset` from [nirs4allformats_open_dataset()].
#' @param row.names Ignored; present for S3 method signature compatibility.
#' @param optional Ignored; present for S3 method signature compatibility.
#' @param ... Ignored; present for S3 method consistency.
#'
#' @return A `data.frame` with columns `sample_id`, the target columns (if any),
#'   and `x_<wavelength>` spectral columns.
#'
#' @examples
#' \dontrun{
#' ds <- nirs4allformats_open_dataset("samples/csv_tsv/synthetic_nirs.csv")
#' df <- as.data.frame(ds)
#' names(df)[1:5]
#' }
#'
#' @seealso [nirs4allformats_open_dataset()], [as.matrix.nirs4allformats_dataset()],
#'   [nirs4allformats_as_tibble()].
#' @exportS3Method base::as.data.frame
as.data.frame.nirs4allformats_dataset <- function(x, row.names = NULL, optional = FALSE, ...) {
  out <- data.frame(sample_id = x$sample_ids, stringsAsFactors = FALSE)
  if (ncol(x$targets) > 0) {
    out <- cbind(out, x$targets)
  }
  spectral <- as.data.frame(x$x, optional = TRUE, stringsAsFactors = FALSE)
  names(spectral) <- paste0("x_", format(x$wavelengths, trim = TRUE, scientific = FALSE))
  cbind(out, spectral)
}

#' Coerce an `nirs4allformats_dataset` to a tibble
#'
#' @title Convert a dataset to a tibble
#' @description
#' Convenience wrapper that converts an
#' [nirs4allformats_dataset][nirs4allformats_open_dataset] to a
#' [tibble][tibble::tibble] via [as.data.frame.nirs4allformats_dataset()]. The
#' optional `tibble` package must be installed.
#'
#' @param dataset An `nirs4allformats_dataset` from [nirs4allformats_open_dataset()].
#'
#' @return A [tibble::tibble] with the same columns as
#'   [as.data.frame.nirs4allformats_dataset()] (`sample_id`, target columns,
#'   `x_<wavelength>` spectral columns).
#'
#' @examples
#' \dontrun{
#' ds <- nirs4allformats_open_dataset("samples/csv_tsv/synthetic_nirs.csv")
#' nirs4allformats_as_tibble(ds)
#' }
#'
#' @seealso [nirs4allformats_open_dataset()], [as.data.frame.nirs4allformats_dataset()].
#' @export
nirs4allformats_as_tibble <- function(dataset) {
  if (!requireNamespace("tibble", quietly = TRUE)) {
    stop("Package 'tibble' is required for nirs4allformats_as_tibble()", call. = FALSE)
  }
  tibble::as_tibble(as.data.frame(dataset))
}

nirs4allformats_run_reader <- function(path) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allformats_native_call(
    "nirs4allformats_native_read",
    resolved,
    NULL,
    NULL,
    NULL
  )
  if (!is.null(payload)) {
    return(payload)
  }
  nirs4allformats_run_cli(c("read-json", resolved))
}

#' Probe a file for candidate readers
#'
#' @title List candidate readers for a file
#' @description
#' Sniffs a file (reading only its head, not a full parse) and returns the
#' ordered list of readers that recognize it, highest confidence first. Useful
#' for diagnosing format detection without decoding the whole file.
#'
#' Sniffing is performed entirely in Rust. The native extendr library is used
#' when present; otherwise the `nirs4all-formats probe` CLI command is invoked (see
#' *Transport* in [nirs4allformats_open_dataset()]).
#'
#' @param path Character scalar. Path to the file to probe (resolved with
#'   [normalizePath()], `mustWork = TRUE`).
#'
#' @return A list of candidate descriptors. Each entry includes at least a
#'   `format` name and a confidence indication, ordered from most to least
#'   confident. The list is empty when no reader recognizes the file.
#'
#' @examples
#' \dontrun{
#' probes <- nirs4allformats_probe_path("samples/csv_tsv/synthetic_nirs.csv")
#' probes[[1]]$format
#' }
#'
#' @seealso [nirs4allformats_open_records()], [nirs4allformats_walk_path()].
#' @export
nirs4allformats_probe_path <- function(path) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allformats_native_call("nirs4allformats_native_probe", resolved)
  if (is.null(payload)) {
    payload <- nirs4allformats_run_cli(c("probe", resolved))
  }
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

#' Decode in-memory bytes into records (native only)
#'
#' @title Decode raw bytes through the native registry
#' @description
#' Decodes an in-memory byte buffer through the Rust registry and returns the
#' normalized records as nested R lists, without touching the filesystem. The
#' `name` drives extension-based sniffing and provenance. This path requires the
#' native extendr static library; it is unavailable through the CLI fallback and
#' raises an error when the native library is absent.
#'
#' Formats that need companion files (sidecars) are rejected here; use
#' [nirs4allformats_open_with_sidecars()] for those.
#'
#' @param name Character scalar. Logical file name (with extension) used for
#'   format sniffing and recorded in provenance, e.g. `"spectrum.dx"`.
#' @param bytes A `raw` vector containing the file contents.
#'
#' @return A list of records, identical in shape to [nirs4allformats_open_records()].
#'
#' @examples
#' \dontrun{
#' bytes <- readBin("spectrum.dx", what = "raw",
#'                  n = file.info("spectrum.dx")$size)
#' records <- nirs4allformats_open_bytes("spectrum.dx", bytes)
#' }
#'
#' @seealso [nirs4allformats_open_with_sidecars()], [nirs4allformats_open_records()],
#'   [nirs4allformats_native_available()].
#' @export
nirs4allformats_open_bytes <- function(name, bytes) {
  if (!nirs4allformats_native_available()) {
    stop(
      "open_bytes requires the native extendr static library. Reinstall the ",
      "package via `R CMD INSTALL` with Cargo on PATH.",
      call. = FALSE
    )
  }
  if (!is.raw(bytes)) {
    stop("bytes must be a raw vector", call. = FALSE)
  }
  payload <- nirs4allformats_native_call(
    "nirs4allformats_native_read_bytes",
    as.character(name),
    bytes,
    NULL,
    NULL,
    NULL
  )
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

#' Decode in-memory bytes plus sidecar files (native only)
#'
#' @title Decode raw bytes with companion sidecar files
#' @description
#' Decodes an in-memory primary buffer together with a named map of companion
#' files (sidecars) through the Rust registry, returning normalized records as
#' nested R lists. This serves formats that split a measurement across multiple
#' files, such as ENVI Standard cubes (`.img` + `.hdr`) or ERDAS LAN. Sidecar
#' names are interpreted as paths relative to the primary file.
#'
#' This path requires the native extendr static library and raises an error when
#' it is absent; it has no CLI fallback.
#'
#' @param name Character scalar. Logical file name of the primary file (with
#'   extension), used for sniffing and provenance, e.g. `"cube.img"`.
#' @param bytes A `raw` vector with the primary file contents.
#' @param sidecars A named list of `raw` vectors. Each name is a companion file
#'   path relative to the primary file (e.g. `"cube.hdr"`); each value is that
#'   file's bytes. Defaults to an empty list.
#'
#' @return A list of records, identical in shape to [nirs4allformats_open_records()].
#'
#' @examples
#' \dontrun{
#' read_raw <- function(p) readBin(p, "raw", n = file.info(p)$size)
#' records <- nirs4allformats_open_with_sidecars(
#'   "cube.img",
#'   read_raw("cube.img"),
#'   list("cube.hdr" = read_raw("cube.hdr"))
#' )
#' }
#'
#' @seealso [nirs4allformats_open_bytes()], [nirs4allformats_open_records()],
#'   [nirs4allformats_native_available()].
#' @export
nirs4allformats_open_with_sidecars <- function(name, bytes, sidecars = list()) {
  if (!nirs4allformats_native_available()) {
    stop(
      "open_with_sidecars requires the native extendr static library. ",
      "Reinstall the package via `R CMD INSTALL` with Cargo on PATH.",
      call. = FALSE
    )
  }
  if (!is.raw(bytes)) {
    stop("bytes must be a raw vector", call. = FALSE)
  }
  if (!is.list(sidecars) || is.null(names(sidecars))) {
    stop("sidecars must be a named list of raw vectors", call. = FALSE)
  }
  for (key in names(sidecars)) {
    if (!is.raw(sidecars[[key]])) {
      stop(sprintf("sidecar '%s' must be a raw vector", key), call. = FALSE)
    }
  }
  payload <- nirs4allformats_native_call(
    "nirs4allformats_native_read_with_sidecars",
    as.character(name),
    bytes,
    sidecars,
    NULL,
    NULL,
    NULL
  )
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

#' Recursively scan a directory or file
#'
#' @title Walk a directory and report per-file outcomes
#' @description
#' Recursively visits a directory (or a single file) and reports the detection
#' outcome for each visited file: whether it parsed, errored, or is unsupported,
#' together with its detected format. Only sniffing and walking happen here; no
#' file is fully decoded.
#'
#' The walk runs in Rust. The native extendr library is used when present;
#' otherwise the `nirs4all-formats scan --json` CLI command is invoked and its
#' `entries` are returned (see *Transport* in [nirs4allformats_open_dataset()]).
#'
#' @param path Character scalar. Directory or file to scan (resolved with
#'   [normalizePath()], `mustWork = TRUE`).
#' @param max_depth Optional integer. Maximum recursion depth; `NULL` (default)
#'   means unlimited.
#' @param include_hidden Logical. Include hidden files/directories. Defaults to
#'   `FALSE`.
#' @param follow_symlinks Logical. Follow symbolic links during the walk.
#'   Defaults to `FALSE`.
#' @param include_unsupported Logical. Include entries for files no reader
#'   recognizes. Defaults to `FALSE`.
#'
#' @return A list of per-file outcome entries. Each entry includes at least a
#'   `status` (e.g. `"parsed"`, `"error"`, `"unsupported"`) and, when detected,
#'   a `format`.
#'
#' @examples
#' \dontrun{
#' entries <- nirs4allformats_walk_path("samples/asd")
#' length(entries)
#' entries[[1]]$status
#' entries[[1]]$format
#'
#' # Limit recursion depth and include unsupported files
#' nirs4allformats_walk_path("samples", max_depth = 1, include_unsupported = TRUE)
#' }
#'
#' @seealso [nirs4allformats_probe_path()], [nirs4allformats_open_records()].
#' @export
nirs4allformats_walk_path <- function(path,
                                  max_depth = NULL,
                                  include_hidden = FALSE,
                                  follow_symlinks = FALSE,
                                  include_unsupported = FALSE) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allformats_native_call(
    "nirs4allformats_native_walk",
    resolved,
    if (is.null(max_depth)) NULL else as.integer(max_depth),
    isTRUE(include_hidden),
    isTRUE(follow_symlinks),
    isTRUE(include_unsupported)
  )
  if (!is.null(payload)) {
    return(jsonlite::fromJSON(payload, simplifyVector = FALSE))
  }
  args <- c("scan", resolved)
  if (!is.null(max_depth)) {
    args <- c(args, "--max-depth", as.character(as.integer(max_depth)))
  }
  if (isTRUE(include_hidden)) {
    args <- c(args, "--include-hidden")
  }
  if (isTRUE(follow_symlinks)) {
    args <- c(args, "--follow-symlinks")
  }
  if (isTRUE(include_unsupported)) {
    args <- c(args, "--include-unsupported")
  }
  args <- c(args, "--json")
  parsed <- jsonlite::fromJSON(nirs4allformats_run_cli(args), simplifyVector = FALSE)
  parsed$entries %||% list()
}

nirs4allformats_run_cli <- function(args) {
  command <- nirs4allformats_reader_command()
  stdout <- tempfile("nirs4allformats-stdout-")
  stderr <- tempfile("nirs4allformats-stderr-")
  on.exit(unlink(c(stdout, stderr)), add = TRUE)
  status <- system2(
    command[[1]],
    c(command[-1], args),
    stdout = stdout,
    stderr = stderr
  )
  if (!identical(status, 0L)) {
    message <- paste(readLines(stderr, warn = FALSE), collapse = "\n")
    stop(if (nzchar(message)) message else sprintf("Rust reader failed with status %s", status), call. = FALSE)
  }
  paste(readLines(stdout, warn = FALSE), collapse = "\n")
}

nirs4allformats_reader_command <- function() {
  explicit <- Sys.getenv("NIRS4ALL_FORMATS_CLI", unset = "")
  if (nzchar(explicit)) {
    return(strsplit(explicit, "\\s+")[[1]])
  }

  binary <- Sys.which("nirs4all-formats")
  if (nzchar(binary)) {
    return(unname(binary))
  }

  cargo <- Sys.which("cargo")
  if (!nzchar(cargo)) {
    rustup_cargo <- file.path(Sys.getenv("HOME"), ".cargo", "bin", "cargo")
    if (file.exists(rustup_cargo)) {
      cargo <- rustup_cargo
    }
  }
  root <- nirs4allformats_repo_root()
  if (nzchar(cargo) && !is.null(root)) {
    return(c(unname(cargo), "run", "-q", "-p", "nirs4all-formats-cli", "--manifest-path", file.path(root, "Cargo.toml"), "--"))
  }

  stop("Cannot find nirs4all-formats CLI binary or source workspace", call. = FALSE)
}

nirs4allformats_repo_root <- function() {
  starts <- unique(c(
    Sys.getenv("NIRS4ALL_FORMATS_REPO", unset = ""),
    getwd(),
    system.file(package = "nirs4allformats")
  ))
  starts <- starts[nzchar(starts)]
  for (start in starts) {
    current <- normalizePath(start, mustWork = FALSE)
    repeat {
      if (
        file.exists(file.path(current, "Cargo.toml")) &&
          dir.exists(file.path(current, "crates", "nirs4all-formats-cli"))
      ) {
        return(current)
      }
      parent <- dirname(current)
      if (identical(parent, current)) {
        break
      }
      current <- parent
    }
  }
  NULL
}

nirs4allformats_select_signal <- function(record, requested = NULL) {
  signals <- record$signals
  if (!is.list(signals) || length(signals) == 0) {
    stop("Record has no signals", call. = FALSE)
  }
  if (!is.null(requested)) {
    if (is.null(signals[[requested]])) {
      stop(sprintf("Record does not contain signal '%s'", requested), call. = FALSE)
    }
    return(list(name = requested, payload = signals[[requested]]))
  }
  preferred <- record$signal_type
  for (name in names(signals)) {
    if (identical(signals[[name]]$signal_type, preferred)) {
      return(list(name = name, payload = signals[[name]]))
    }
  }
  for (name in c("reflectance", "absorbance", "transmittance", "signal")) {
    if (!is.null(signals[[name]])) {
      return(list(name = name, payload = signals[[name]]))
    }
  }
  name <- sort(names(signals))[[1]]
  list(name = name, payload = signals[[name]])
}

nirs4allformats_sample_id <- function(record, metadata, row_index) {
  if (!is.null(metadata$sample_id)) {
    return(as.character(metadata$sample_id))
  }
  source_path <- record$provenance$sources[[1]]$path
  if (!is.null(source_path)) {
    return(sprintf("%s:%d", tools::file_path_sans_ext(basename(source_path)), row_index - 1L))
  }
  sprintf("record:%d", row_index - 1L)
}

nirs4allformats_flatten_column <- function(values) {
  unlist(
    lapply(values, function(value) {
      if (is.null(value)) NA else value
    }),
    use.names = FALSE
  )
}

`%||%` <- function(lhs, rhs) {
  if (is.null(lhs)) rhs else lhs
}
