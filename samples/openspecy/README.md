# OpenSpecy `.rds`

[OpenSpecy](https://openspecy.org) is an R toolkit for Raman and (FT)IR
spectroscopy of microplastics and environmental particles. It serialises spectra
with R's `saveRDS()` - a gzip-compressed XDR stream. The native
`nirs4all-formats` reader (`fmt-openspecy` feature) decodes that container with
the pure-Rust [`rds2rust`](https://crates.io/crates/rds2rust) parser (no R, no
subprocess) and maps the canonical `OpenSpecy` object onto `SpectralRecord`s.

## Canonical object

`as_OpenSpecy()` builds an S3-classed three-part list:

- `wavenumber` - numeric vector, the shared x-axis (cm^-1), length `W`.
- `spectra` - `data.table`/`data.frame`, **one column per spectrum**, `W` rows.
- `metadata` - `data.table`/`data.frame`, **one row per spectrum**
  (`spectrum_type` = `ftir`/`raman`, `spectrum_identity`, `sample_name`, ...).

Each spectra column becomes one record over the shared wavenumber axis, with its
metadata row attached (`metadata.fields`) and the polymer/material identity
surfaced into `targets`.

## Samples

Generated locally (CC0 / public domain):

| File | Notes |
|---|---|
| `synthetic_minilib.rds` | A minimal canonical `OpenSpecy` object: 3 spectra (2 `ftir`, 1 `raman`) over 6 wavenumbers, with a `metadata` table (`col_id`, `spectrum_type`, `spectrum_identity`, `sample_name`). Written with `rds2rust`'s RDS writer so the test suite never touches the network or R. |

Real OpenSpecy reference libraries (e.g. the `nobaseline.rds` FTIR/Raman
libraries distributed via OSF, tens of thousands of spectra each) are large and
license-restricted, so they are **not** vendored here; the reader is exercised
against them out-of-tree.

## Parser hints

- Reference reader: `OpenSpecy::read_spec()` / `readRDS()` in R.
- Containers decoded: gzip-compressed and uncompressed XDR `.rds` (the
  `saveRDS()` defaults). `bzip2`/`xz`-compressed `.rds` are not decoded.
- The legacy single-spectrum form (a `data.frame` with `wavenumber` +
  `intensity` columns) is also accepted.
- When a very large library comes back from the RDS parser without its
  `names`/`class` attributes, the three parts are located structurally by shape,
  so the spectra still load (metadata may then be absent).
- Intensity semantics are reported as `unknown` unless the metadata names them
  via an `intensity_units` field.
