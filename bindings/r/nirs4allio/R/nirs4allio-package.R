#' nirs4allio: R Binding for nirs4all-io
#'
#' @description
#' Thin R binding for the Rust-first `nirs4all-io` NIRS/spectroscopy file
#' reading core. It auto-detects each file by content, decodes it through a
#' single Rust registry, and surfaces the canonical, provenance-tracked records
#' with R-native ergonomics (lists, matrices, data frames, tibbles).
#'
#' Parser logic lives only in Rust: this package never reimplements any format
#' parsing. A new format is a new Rust reader, not new R code. The binding only
#' dispatches calls and converts the result.
#'
#' @section Transport (native vs CLI):
#' When the package is installed via `R CMD INSTALL` with Cargo on `PATH`, it
#' compiles a native extendr static library from `src/rust/` and dispatches
#' probe/read/walk calls directly through Rust. Without Cargo it falls back to
#' invoking the `nirs4all-io` CLI binary. The `NIRS4ALL_IO_CLI` environment
#' variable may point to a prebuilt CLI binary; in a source checkout the binding
#' can also fall back to `cargo run -p nirs4all-io-cli`. In-memory decoding
#' ([nirs4allio_open_bytes()], [nirs4allio_open_with_sidecars()]) is available
#' only on the native path.
#'
#' @section Main functions:
#' \describe{
#'   \item{[nirs4allio_open_records()]}{Lossless records as nested R lists.}
#'   \item{[nirs4allio_open_dataset()]}{Flat `nirs4allio_dataset` (matrix +
#'     targets + metadata).}
#'   \item{[nirs4allio_probe_path()]}{Ordered candidate readers for a file.}
#'   \item{[nirs4allio_walk_path()]}{Recursive per-file scan outcomes.}
#'   \item{[nirs4allio_open_bytes()] / [nirs4allio_open_with_sidecars()]}{
#'     In-memory decoding (native backend only).}
#'   \item{[nirs4allio_native_available()] / [nirs4allio_version()]}{Backend and
#'     version introspection.}
#' }
#'
#' @seealso [nirs4allio_open_dataset()], [nirs4allio_open_records()].
#' @keywords internal
"_PACKAGE"
