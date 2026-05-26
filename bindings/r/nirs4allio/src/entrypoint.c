/* Minimal R entrypoint that forwards module registration to the Rust */
/* static library produced under src/rust/. */

#include <R.h>
#include <Rinternals.h>
#include <R_ext/Rdynload.h>

extern SEXP R_init_nirs4allio_r_extendr(DllInfo *);

void R_init_nirs4allio(DllInfo *info) {
    R_init_nirs4allio_r_extendr(info);
}
