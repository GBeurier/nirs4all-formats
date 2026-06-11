/* Minimal R entrypoint that forwards module registration to the Rust */
/* static library produced under src/rust/. */

#include <R.h>
#include <Rinternals.h>
#include <R_ext/Rdynload.h>

extern SEXP R_init_nirs4allformats_r_extendr(DllInfo *);

/* Package name is `nirs4allformats.full`; R maps the `.` in the package name to
 * `_` for the C init symbol (Writing R Extensions, "Registering native
 * routines"), so the entry point is R_init_nirs4allformats_full. The extendr
 * static library and its init routine keep the crate name (nirs4allformats_r). */
void R_init_nirs4allformats_full(DllInfo *info) {
    R_init_nirs4allformats_r_extendr(info);
}
