# Exports

Every reader feeds every writer through `SpectralRecord`. There is no converter
per pair of formats. Exports never resample, reorder, rescale or convert units:
what a target cannot carry is **refused** or listed as a **loss** in the result.

## Formats

| Target | Extension | Content |
|---|---|---|
| `jcamp-dx` | `.jdx` | JCAMP-DX 4.24, one uncompressed spectrum per file. Evenly spaced axes use `XYDATA` with 32-bit integer ordinates and a power-of-ten `YFACTOR`; other axes use `XYPOINTS` with full decimals. |
| `galactic-spc` | `.spc` | GRAMS SPC `0x4B`, little-endian, 32-bit float ordinates. Evenly spaced axes keep 64-bit limits; explicit axes are 32-bit floats. `multi` groups spectra sharing an axis. |
| `csv` | `.csv` | One row per spectrum: identifier, targets, one column per axis value. |
| `xlsx` | `.xlsx` | Same matrix in sheet `spectra`; axis, signal and source in sheet `info`. |
| `xy-text` | `.txt` | Two columns, one spectrum per file, no header. |
| `bruker-dpt` | `.dpt` | OPUS data point table; wavenumber (cm-1) axes only. |
| `records-json` | `.json` | Complete normalized records. |

Every signal is written unless `signal` selects one. N-dimensional signals are
flattened into named spectra. Matrix targets write one file per signal and per
distinct axis. Missing values become empty cells in matrix targets and are
refused by the others.

## Software presets

A preset names a format the software documents as importable. `tested` stays
`false` until an import of our files into a named version is recorded: reading
our files back does not prove that a vendor accepts them. Opening a spectrum is
also not a license to predict with a vendor calibration built on another
instrument.

| Preset | Format | Also documented | Evidence |
|---|---|---|---|
| `opus` | `jcamp-dx` (`.dx`) | `galactic-spc`, `bruker-dpt` | OPUS 5 reference manual, 2.1 and 3.11 |
| `omnic` | `jcamp-dx` (`.jdx`) | `galactic-spc` | OMNIC Paradigm user guide |
| `perkinelmer-spectrum` | `jcamp-dx` (`.dx`) | `galactic-spc` | Spectrum user's guide |
| `nircal` | `jcamp-dx` (`.dx`) | `galactic-spc` | NIRCal 5.5 manual, 3.15.2 |
| `grams` | `galactic-spc` | `jcamp-dx` | native format |
| `unscrambler` | `xlsx` | `jcamp-dx`, `galactic-spc`, `csv` | The Unscrambler X 10.3 manual |
| `simca` | `xlsx` | `csv`, `jcamp-dx`, `galactic-spc` | Sartorius application note |
| `solo` | `xlsx` | `csv`, `galactic-spc`, `jcamp-dx` | PLS_Toolbox 7.5 documentation |
| `pirouette` | `galactic-spc` | `jcamp-dx` | Pirouette 5.0 manual, chapter 14 |
| `spectragryph` | `galactic-spc` | `jcamp-dx`, `xy-text` | Spectragryph feature list |
| `winisi` | `jcamp-dx` (`.dx`) | | user forum only |

WinISI users report that it expects 700 or 1050 points per spectrum. Native
FOSS `.nir`/`.cal` and NSAS `.DA`/`.CN` files are not written yet.

## Interfaces

```bash
nirs4all-formats targets
nirs4all-formats convert spectrum.0 --to omnic --out exported/
nirs4all-formats convert set.csv --to galactic-spc --multi --out exported/
```

```python
import nirs4all_formats as nio

nio.export_targets()
nio.convert_path("spectrum.0", "omnic", "exported")
output = nio.write_records(nio.open_records("set.csv"), "xlsx", signal="signal")
output["files"], output["losses"]
```

```r
nirs4allformats_export_targets()
nirs4allformats_convert("spectrum.0", "omnic", "exported")
```

```js
const { files, losses } = writeRecords(openBytes(name, bytes), "omnic", { base_name: "set" });
```

Options: `signal`, `base_name`, `extension`, `axis_tolerance` (largest absolute
axis deviation accepted to write an almost even axis as even; the deviation is
reported), `multi`, `delimiter` (`,`, `;`, tab) and `zip`. Existing files are
never replaced unless `overwrite` is set.

## Checks

`crates/nirs4all-formats/tests/writers.rs` writes every decodable public sample
and reads it back. `tests/conformance/test_writers.py` compares exported files
with independent readers (`jcamp`, `spc_spectra`, `openpyxl`).
