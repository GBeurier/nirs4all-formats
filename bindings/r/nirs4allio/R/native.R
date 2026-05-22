## Native FFI shims. The Rust static library produced under src/rust/ is
## registered into R during package load via src/entrypoint.c. When the shared
## object is not present (e.g. installation via plain copy without Cargo) the
## helpers below return FALSE and the CLI fallback in io.R takes over.

nirs4allio_native_available <- function() {
  isTRUE(is.loaded("nirs4allio_native_probe"))
}

nirs4allio_native_call <- function(symbol, ...) {
  if (!nirs4allio_native_available()) {
    return(NULL)
  }
  args <- list(symbol, ...)
  payload <- do.call(.Call, args)
  if (!is.character(payload) || length(payload) != 1L) {
    stop(sprintf("native symbol %s did not return a JSON string", symbol), call. = FALSE)
  }
  payload
}
