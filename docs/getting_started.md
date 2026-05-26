# Getting started

`nirs4all-io` is a **Rust-first, low-level reader** for NIRS and spectroscopy
file formats. It reads ~58 format families, auto-detects each file by content,
and returns a canonical `SpectralRecord` model that the same code can project
into numpy / pandas / sklearn / torch (Python), matrices and data frames (R), or
typed arrays (WebAssembly).

It does **not** do chemometrics or modelling — it produces the clean,
provenance-tracked records that a modelling library such as
[`nirs4all`](https://github.com/GBeurier/nirs4all) then consumes.

The Rust core is the single source of truth. Every binding decodes through the
same registry and only converts the result, so a file reads identically from
Rust, Python, R, the CLI or the browser. See the
[supported-format catalogue](SUPPORTED_FORMATS.md) for what is covered.

## 30 seconds, three ways

**Command line**

```bash
# Which reader will handle this file, and why?
nirs4all-io probe samples/jcamp_dx/TESTSPEC.DX

# Decode it to normalized JSON records
nirs4all-io read-json samples/jcamp_dx/TESTSPEC.DX
```

**Python**

```python
import nirs4all_io as nio

# Lossless object model: every signal, axis, coord, metadata and provenance
records = nio.open_recordset("spectrum.sed")

# Or go straight to a modelling-ready matrix (X[n_samples, n_features], axis)
X, axis = records.to_numpy(signal="reflectance")
```

**R**

```r
library(nirs4allio)

dataset <- nirs4allio_open_dataset("spectrum.sed")
X       <- as.matrix(dataset)        # spectral matrix
df      <- as.data.frame(dataset)    # sample ids + targets + spectral columns
```

## What you get back

Every reader emits one or more `SpectralRecord`s. Each record carries:

- **signals** — named channels (e.g. `absorbance`, `reflectance`, `raw_counts`,
  `white_reference`), each with its own spectral axis;
- **axis** — values plus unit (`nm`, `cm-1`, `um`, …) and kind
  (wavelength / wavenumber / energy / time / index);
- **targets** — lab reference values for modelling, when the file carries them;
- **metadata** — instrument, acquisition and sample fields;
- **provenance** — source file, SHA-256, reader name/version and warnings;
- **quality flags** — explicit caveats (conversions, suspect axes, partial
  support).

Nothing is resampled, merged or silently dropped. See the
[data model](DATA_MODEL.md) for the full contract.

## Next steps

- [Installation](installation.md) — Rust, Python, R, WebAssembly and the C ABI.
- [Usage guide](usage.md) — probing, reading, sidecars, image cubes, scanning a
  folder, and projecting to numpy/pandas/sklearn/torch.
- [Supported formats](SUPPORTED_FORMATS.md) — the full catalogue.
- Bindings: [Python](bindings/python.md) · [R](bindings/r.md) ·
  [WebAssembly](bindings/wasm.md) · [C ABI](bindings/capi.md).
- Don't see your format, or hit a misread file? Open an
  [issue](https://github.com/GBeurier/nirs4all-io/issues/new/choose).
