# Data Model

The Rust core emits `SpectralRecord` values. Bindings may expose equivalent
language-native shapes, but the Rust model is canonical.

## SpectralAxis

One axis belongs to one signal. This avoids assuming that every channel in a
file shares the same x-axis.

Fields:

- `values`: native axis values as `f64`;
- `unit`: `nm`, `cm-1`, `um`, `thz`, `index`, etc.;
- `kind`: wavelength, wavenumber, frequency, energy, time or index;
- `order`: ascending, descending or non-monotonic.

## SpectralArray

One named signal channel.

Fields:

- `axis`;
- `values`;
- `dims`, containing exactly one `x` dimension;
- `signal_type`;
- optional physical `unit`;
- `role`, such as `raw_dn`, `white_ref`, `absorbance`, `reflectance`;
- `source`, usually `file` or `derived`.

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
