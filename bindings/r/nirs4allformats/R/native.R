## Native FFI shims. The Rust static library produced under src/rust/ is
## registered into R during package load via src/entrypoint.c. When the shared
## object is not present (e.g. installation via plain copy without Cargo) the
## helpers below return FALSE and the CLI fallback in io.R takes over.

#' Is the native extendr backend available?
#'
#' @title Report whether the native Rust backend is loaded
#' @description
#' Returns `TRUE` when the compiled extendr static library is registered in the
#' running R session, i.e. the package was installed with Cargo on `PATH` and
#' the Rust core is callable directly. When `FALSE`, filesystem reads fall back
#' to the `nirs4all-formats` CLI, and the in-memory paths
#' ([nirs4allformats_open_bytes()], [nirs4allformats_open_with_sidecars()]) are
#' unavailable.
#'
#' @return A length-one logical: `TRUE` if the native backend is loaded,
#'   otherwise `FALSE`.
#'
#' @examples
#' \dontrun{
#' if (nirs4allformats_native_available()) {
#'   message("native extendr backend active")
#' } else {
#'   message("using nirs4all-formats CLI fallback")
#' }
#' }
#'
#' @seealso [nirs4allformats_open_bytes()], [nirs4allformats_open_with_sidecars()].
#' @export
nirs4allformats_native_available <- function() {
  isTRUE(is.loaded("wrap__nirs4allformats_native_probe", PACKAGE = "nirs4allformats"))
}

nirs4allformats_native_call <- function(symbol, ...) {
  if (!nirs4allformats_native_available()) {
    return(NULL)
  }
  args <- list(paste0("wrap__", symbol), ..., PACKAGE = "nirs4allformats")
  payload <- do.call(.Call, args)
  if (!is.character(payload) || length(payload) != 1L) {
    stop(sprintf("native symbol %s did not return a JSON string", symbol), call. = FALSE)
  }
  payload
}
