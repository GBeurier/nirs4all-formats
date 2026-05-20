# R Binding Plan

The R package exposes the Rust core with R-native ergonomics.

Required surfaces:

- raw record access;
- `matrix` plus wavelength vector exports;
- `data.frame`/tibble-compatible exports;
- target extraction helpers;
- local package build before registry publication.

Parser logic must stay in Rust.
