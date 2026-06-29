# OpenSpecy Raman / (FT)IR `.rds`

> **Status:** Supported (scoped) | **Vendor:** OpenSpecy (R) | **Extensions:** `.rds` | **Feature flag:** `fmt-openspecy`

[OpenSpecy](https://openspecy.org) is an open R toolkit for Raman and (FT)IR
spectroscopy of microplastics and environmental particles. It stores spectra and
reference libraries as R `saveRDS()` objects (`.rds`). This reader decodes that
container with the pure-Rust [`rds2rust`](https://crates.io/crates/rds2rust)
parser - no R runtime, no subprocess - and maps the canonical `OpenSpecy` object
onto records.

## Instruments & software

Vendor-neutral: any FTIR or Raman spectrometer whose spectra are imported into
the OpenSpecy R package (or the [openspecy.org](https://openspecy.org) web app),
including the bundled `nobaseline` reference libraries downloaded from OSF.

## File structure

The reader is gated behind the `fmt-openspecy` feature, which pulls in the
pure-Rust `rds2rust` crate. Dispatch is
by `.rds` extension plus the R serialization container magic:

- **gzip RDS** (`1f 8b`, probe `Likely`) - the `saveRDS()` default: a
  gzip-compressed XDR stream, decompressed and parsed by `rds2rust`.
- **uncompressed XDR RDS** (`X\n`, probe `Likely`) - the same XDR stream without
  the gzip wrapper.

The `.rds` extension alone cannot prove a file is an OpenSpecy object (it is a
generic R container), so the OpenSpecy shape is validated on read; a non-OpenSpecy
`.rds` returns an actionable error.

### Canonical object (OpenSpecy >= 1.0)

`as_OpenSpecy()` builds an S3-classed three-part list:

- `wavenumber` - numeric vector, the shared x-axis (cm^-1), length `W`.
- `spectra` - a `data.table`/`data.frame` with **one column per spectrum** and
  `W` rows; column names are spectrum identifiers.
- `metadata` - a `data.table`/`data.frame` with **one row per spectrum**
  (`spectrum_type` = `ftir`/`raman`, `spectrum_identity`, `sample_name`, ...).

## What nirs4all-formats extracts

- **Signals** - each spectra column becomes one record: a 1-D `intensity` signal
  over the shared wavenumber axis (`AxisKind::Wavenumber`, unit `cm-1`). The
  column name is carried as the signal `source`.
- **Axis** - the `wavenumber` vector, shared by every spectrum. Raman shift and
  IR wavenumber are both reported in cm^-1, matching OpenSpecy.
- **Targets** - the polymer / material identity (`spectrum_identity`,
  `material_class`) when present in the metadata.
- **Metadata & provenance** - FTIR/Raman `modality` (from `spectrum_type`), the
  R `class`, the spectrum column id, the full metadata row under `fields`,
  container type, and source file + SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Canonical `OpenSpecy` list (gzip RDS) | Supported | `wavenumber` + `spectra` + `metadata`; one record per spectrum. |
| Canonical `OpenSpecy` list (uncompressed XDR RDS) | Supported | Same mapping, no gzip wrapper. |
| Legacy single-spectrum `data.frame` (`wavenumber` + `intensity`) | Supported | Emitted as one record. |
| Attribute-stripped or metadata-free large library | Supported (degraded) | Parts located structurally by shape; spectra load, but metadata is only emitted if present in the RDS object. |
| `bzip2` / `xz`-compressed `.rds` | Not supported | Only gzip + uncompressed XDR are decoded. |

## Limitations & known gaps

- Only the default `saveRDS()` containers are decoded (gzip and uncompressed
  XDR). `bzip2`/`xz`-compressed `.rds` files are rejected with a parse error.
- Intensity semantics are reported as `unknown` unless the metadata names them
  via an `intensity_units` field (mapped to absorbance/transmittance/...).
- Some public `nobaseline.rds` exports contain a `NULL` `metadata` slot. The
  spectra and wavenumber axis still load via structural shape detection, but
  per-spectrum metadata (identity, modality) cannot be reconstructed from that
  RDS object alone.

## Reference readers

`OpenSpecy::read_spec()` / `readRDS()` in R. nirs4all-formats adds container
sniffing, structural fallback, axis/modality typing, target mapping and
provenance on top of the `rds2rust` decode.

## Samples & validation

A committed, network-free fixture under `samples/openspecy/`:
`synthetic_minilib.rds` - a minimal canonical `OpenSpecy` object with 3 spectra
(2 `ftir`, 1 `raman`) over 6 wavenumbers and a 4-column `metadata` table, written
with the `rds2rust` RDS writer (CC0). It is golden-backed in
`crates/nirs4all-formats/tests/goldens/` and exercised end-to-end (path + bytes)
in `crates/nirs4all-formats/tests/openspecy.rs`; the reader's structural
fallback and legacy-data.frame paths are unit-tested in the reader module. Real
OpenSpecy `nobaseline` libraries are large and license-restricted, so they are
validated out-of-tree rather than vendored.
