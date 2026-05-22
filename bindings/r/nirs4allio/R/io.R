nirs4allio_open_records <- function(path) {
  payload <- nirs4allio_run_reader(path)
  records <- jsonlite::fromJSON(payload, simplifyVector = FALSE)
  if (!is.list(records)) {
    stop("Rust reader returned a non-list JSON payload", call. = FALSE)
  }
  records
}

nirs4allio_open_dataset <- function(path, signal = NULL) {
  records <- nirs4allio_open_records(path)
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
    selected <- nirs4allio_select_signal(record, signal)
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
    sample_ids[[row_index]] <- nirs4allio_sample_id(record, metadata[[row_index]], row_index)
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
    lapply(targets, nirs4allio_flatten_column),
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
    class = "nirs4allio_dataset"
  )
}

as.matrix.nirs4allio_dataset <- function(x, ...) {
  x$x
}

as.data.frame.nirs4allio_dataset <- function(x, row.names = NULL, optional = FALSE, ...) {
  out <- data.frame(sample_id = x$sample_ids, stringsAsFactors = FALSE)
  if (ncol(x$targets) > 0) {
    out <- cbind(out, x$targets)
  }
  spectral <- as.data.frame(x$x, optional = TRUE, stringsAsFactors = FALSE)
  names(spectral) <- paste0("x_", format(x$wavelengths, trim = TRUE, scientific = FALSE))
  cbind(out, spectral)
}

nirs4allio_as_tibble <- function(dataset) {
  if (!requireNamespace("tibble", quietly = TRUE)) {
    stop("Package 'tibble' is required for nirs4allio_as_tibble()", call. = FALSE)
  }
  tibble::as_tibble(as.data.frame(dataset))
}

nirs4allio_run_reader <- function(path) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allio_native_call(
    "nirs4allio_native_read",
    resolved,
    NULL,
    NULL,
    NULL
  )
  if (!is.null(payload)) {
    return(payload)
  }
  nirs4allio_run_cli(c("read-json", resolved))
}

nirs4allio_probe_path <- function(path) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allio_native_call("nirs4allio_native_probe", resolved)
  if (is.null(payload)) {
    payload <- nirs4allio_run_cli(c("probe", resolved))
  }
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

nirs4allio_open_bytes <- function(name, bytes) {
  if (!nirs4allio_native_available()) {
    stop(
      "open_bytes requires the native extendr static library. Reinstall the ",
      "package via `R CMD INSTALL` with Cargo on PATH.",
      call. = FALSE
    )
  }
  if (!is.raw(bytes)) {
    stop("bytes must be a raw vector", call. = FALSE)
  }
  payload <- nirs4allio_native_call(
    "nirs4allio_native_read_bytes",
    as.character(name),
    bytes,
    NULL,
    NULL,
    NULL
  )
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

nirs4allio_open_with_sidecars <- function(name, bytes, sidecars = list()) {
  if (!nirs4allio_native_available()) {
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
  payload <- nirs4allio_native_call(
    "nirs4allio_native_read_with_sidecars",
    as.character(name),
    bytes,
    sidecars,
    NULL,
    NULL,
    NULL
  )
  jsonlite::fromJSON(payload, simplifyVector = FALSE)
}

nirs4allio_walk_path <- function(path,
                                  max_depth = NULL,
                                  include_hidden = FALSE,
                                  follow_symlinks = FALSE,
                                  include_unsupported = FALSE) {
  resolved <- normalizePath(path, mustWork = TRUE)
  payload <- nirs4allio_native_call(
    "nirs4allio_native_walk",
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
  parsed <- jsonlite::fromJSON(nirs4allio_run_cli(args), simplifyVector = FALSE)
  parsed$entries %||% list()
}

nirs4allio_run_cli <- function(args) {
  command <- nirs4allio_reader_command()
  stdout <- tempfile("nirs4allio-stdout-")
  stderr <- tempfile("nirs4allio-stderr-")
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

nirs4allio_reader_command <- function() {
  explicit <- Sys.getenv("NIRS4ALL_IO_CLI", unset = "")
  if (nzchar(explicit)) {
    return(strsplit(explicit, "\\s+")[[1]])
  }

  binary <- Sys.which("nirs4all-io")
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
  root <- nirs4allio_repo_root()
  if (nzchar(cargo) && !is.null(root)) {
    return(c(unname(cargo), "run", "-q", "-p", "nirs4all-io-cli", "--manifest-path", file.path(root, "Cargo.toml"), "--"))
  }

  stop("Cannot find nirs4all-io CLI binary or source workspace", call. = FALSE)
}

nirs4allio_repo_root <- function() {
  starts <- unique(c(
    Sys.getenv("NIRS4ALL_IO_REPO", unset = ""),
    getwd(),
    system.file(package = "nirs4allio")
  ))
  starts <- starts[nzchar(starts)]
  for (start in starts) {
    current <- normalizePath(start, mustWork = FALSE)
    repeat {
      if (
        file.exists(file.path(current, "Cargo.toml")) &&
          dir.exists(file.path(current, "crates", "nirs4all-io-cli"))
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

nirs4allio_select_signal <- function(record, requested = NULL) {
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

nirs4allio_sample_id <- function(record, metadata, row_index) {
  if (!is.null(metadata$sample_id)) {
    return(as.character(metadata$sample_id))
  }
  source_path <- record$provenance$sources[[1]]$path
  if (!is.null(source_path)) {
    return(sprintf("%s:%d", tools::file_path_sans_ext(basename(source_path)), row_index - 1L))
  }
  sprintf("record:%d", row_index - 1L)
}

nirs4allio_flatten_column <- function(values) {
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
