# cran-comments.md

## Not a CRAN package

`nirs4allformats.full` is the **full-reader** build of the `nirs4all-formats`
R binding and is **not submitted to CRAN**. It ships every reader, including the
optional large ones (HDF5/netCDF, Parquet/Arrow, MATLAB). Its self-contained
vendored Rust closure is ~14 MB compressed — well over CRAN's hard 5 MB
source-tarball auto-reject — because the heavy readers pull in Apache
Arrow/Parquet, pure-Rust HDF5/netCDF and the MATLAB + xz codecs.

The size-trimmed **CRAN** package is the sibling
[`nirs4allformats`](../nirs4allformats) (core readers only, default Cargo
features off). Its `cran-comments.md` is the one used for CRAN submission. The
two packages share the same Rust core and the same exported R API; only the
compiled reader set and the resulting tarball size differ.

## Distribution

`nirs4allformats.full` is built and distributed through **R-universe**
(`https://gbeurier.r-universe.dev`), which builds binaries straight from Git and
does not gate on source-tarball size:

```r
install.packages(
  "nirs4allformats.full",
  repos = c(nirs4all = "https://gbeurier.r-universe.dev",
            CRAN = "https://cloud.r-project.org")
)
```

## Build notes

The package self-vendors its Rust closure exactly like the CRAN sibling (see
`./configure` and `bindings/r/nirs4allformats/cran-comments.md` for the vendoring
mechanics) — the only difference is that this build keeps the facade's default
`formats-all` features, so `cargo vendor` collects the full reader closure.
