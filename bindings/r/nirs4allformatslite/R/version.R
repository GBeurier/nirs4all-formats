#' Package version string
#'
#' @title Report the nirs4allformats binding version
#' @description
#' Returns the version of the `nirs4allformats` R binding as a character scalar.
#' This is the binding's own version and is independent of the underlying Rust
#' `nirs4all-formats` core version.
#'
#' @return A length-one character vector with the binding version.
#'
#' @examples
#' nirs4allformats_version()
#'
#' @seealso [nirs4allformats_native_available()].
#' @export
nirs4allformats_version <- function() {
  "0.1.0.9000"
}
