# Data Model

The Rust core emits `SpectralRecord` values. Bindings may expose equivalent
language-native shapes, but the Rust model is canonical.

## SpectralAxis

One axis belongs to one dimension of one signal. This avoids assuming that
every channel in a file shares the same x-axis.

Fields:

- `values`: native axis values as `f64` (must be finite);
- `unit`: `nm`, `cm-1`, `um`, `thz`, `index`, etc.;
- `kind`: wavelength, wavenumber, frequency, energy, time or index;
- `order`: ascending, descending or non-monotonic.

`SpectralAxis::index(n)` builds a 0-based ascending `index`-kind axis for an
uncalibrated dimension (e.g. a spatial pixel row).

## SpectralArray

One named signal channel. The canonical layout is N-dimensional and lossless:
`values` is a flat, **C-order (row-major)** buffer of `product(shape)`
elements. Exactly one dimension is the spectral axis (named `x`); its
coordinate is exposed directly as `axis` so a plain 1-D spectrum stays
ergonomic, while non-spectral dimensions keep their coordinate in `coords`.

Fields:

- `axis`: coordinate of the spectral (`x`) dimension;
- `values`: flat C-order buffer, `values.len() == product(shape)`;
- `shape`: per-dimension extent, `shape.len() == dims.len()`, all `> 0`;
- `dims`: dimension names — unique, non-empty, **exactly one is `x`**;
- `coords`: one `SpectralAxis` per non-`x` dimension, keyed by dim name
  (omitted from JSON when empty);
- `signal_type`;
- optional physical `unit`;
- `role`, such as `raw_dn`, `white_ref`, `absorbance`, `reflectance`;
- `source`, usually `file` or `derived`.

Construction:

- `SpectralArray::new(axis, values, dims, …)` — the **1-D** constructor;
  requires `dims == ["x"]` and `values.len() == axis.values.len()`.
- `SpectralArray::new_nd(shape, dims, axis, coords, values, …)` — the only
  path for multi-dimensional signals (e.g. an image cube slice
  `dims = ["y","x"]`, or a `[row, col, x]` hyperspectral cube). Enforces the
  invariants above plus `coords[d].values.len() == shape[index_of(d)]`.

A 1-D spectrum is just the trivial case: `shape == [n]`, `dims == ["x"]`,
`coords` empty.

> JSON note: `values` and axis coordinates are serialized as plain JSON
> numbers. Non-finite signal values (`NaN`/`Inf`, which real spectra may carry
> as gaps) survive the native PyO3 path but are not representable in strict
> JSON; use the native/binary transport when values may be non-finite. Axis
> coordinates are always required to be finite.

## SpectralRecord

One normalized sample or acquisition unit.

Fields:

- `signals`: named signal channels;
- `signal_type`: dominant signal type for convenience;
- `targets`: lab reference values for modelling;
- `metadata`: JSON-serializable acquisition/instrument/sample metadata;
- `provenance`: reader, format, source hashes and warnings;
- `quality_flags`: explicit caveats.

## Binding Exports

Python exports should include:

- raw record access;
- numpy matrix and axis helpers;
- pandas DataFrame conversion;
- sklearn dataset/provider classes;
- torch dataset adapters.

R exports should include:

- raw record access;
- matrix plus wavelength vector;
- data.frame/tibble conversion;
- target extraction helpers.
