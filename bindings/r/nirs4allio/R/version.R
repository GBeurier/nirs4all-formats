#' Package version string
#'
#' @title Report the nirs4allio binding version
#' @description
#' Returns the version of the `nirs4allio` R binding as a character scalar.
#' This is the binding's own version and is independent of the underlying Rust
#' `nirs4all-io` core version.
#'
#' @return A length-one character vector with the binding version.
#'
#' @examples
#' nirs4allio_version()
#'
#' @seealso [nirs4allio_native_available()].
#' @export
nirs4allio_version <- function() {
  "0.1.0.9000"
}
