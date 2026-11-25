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
| `foss-winisi-nir` | `.nir` | FOSS WinISI / ISIscan spectra: absorbance along nm, 32-bit floats, up to seven evenly spaced segments. |
| `foss-winisi-cal` | `.cal` | Same with up to 32 constituents (numeric targets). |
| `foss-nsas-da` | `.da` | FOSS NSAS spectra on one of the four known axes; constituents go to a `.cn` file of the same name. |
| `records-json` | `.json` | Complete normalized records. |

Every signal is written unless `signal` selects one. N-dimensional signals are
flattened into named spectra. Matrix targets write one file per signal and per
distinct axis. Missing values become empty cells in matrix targets and are
refused by the others. Interferograms are not written as JCAMP-DX, and signals
wider than an Excel sheet are left out of workbooks; both are declared.

Ordinates keep their stored scale. Percent values are labelled as percent;
other scaled units (`mAU`, `counts/s`) are written as arbitrary units with a
declared loss, never under the plain signal type. Spectra are named after the
sample identifier of the record, a title that tells records apart, or the source
file and position; generated names are declared. File names default to the
source file stem.

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
| `pirouette` | `galactic-spc` | `jcamp-dx`, `foss-nsas-da` | Pirouette 5.0 manual, chapter 14 |
| `spectragryph` | `galactic-spc` | `jcamp-dx`, `xy-text` | Spectragryph feature list |
| `winisi` | `foss-winisi-nir` | `foss-winisi-cal`, `jcamp-dx` | native format, see below |
| `winisi-cal` | `foss-winisi-cal` | `foss-winisi-nir`, `jcamp-dx` | native format, see below |
| `vision` | `foss-winisi-nir` | `foss-winisi-cal`, `foss-nsas-da`, `jcamp-dx`, `galactic-spc`, `csv` | Vision 4.1 manual, section 3.11 |

### FOSS files

FOSS publishes no specification. The layouts come from existing files and were
cross-checked with `wai.spectralio` (MIT) and the Vision importer; see
[WinISI](formats/foss-winisi.md) and [NSAS](formats/foss-nsas-da.md).

- Only absorbance (log 1/R) along nm is written. Other signals, units and axes
  are refused; unknown signal types are accepted with a declared assumption.
- Spectra of other instruments are written as WinISI writes them: model
  `Unknown`, instrument type 11. FOSS instruments keep model, type and serial.
- Sample numbers hold 12 characters (NSAS: 10) and constituent names 15
  (NSAS: 10). Longer ones are shortened and kept distinct. The WinISI sample ID
  field keeps up to 49 characters.
- WinISI uses 0 for missing constituents, so missing values become 0 and true
  zeros may read as missing; both are declared. Zeros of WinISI sources count as
  missing in FOSS exports. Constituents are declared on an
  "as received" basis.
- NSAS has no known missing value: a constituent some spectra lack is left out.
- Known WinISI files hold 700 or 1050 points per spectrum; other sizes are
  written with a warning and may not open. NSAS axes are limited to 700 points
  (1100-2498 nm by 2), 1050 (400-2498 by 2), 1700 (800-1649.5 by 0.5) and 4200
  (400-2499.5 by 0.5).
- Unknown dates are written as 1980-01-01 (WinISI) or left empty (NSAS).
- NSAS amplifier and acquisition words are those of an observed file of the same
  geometry, except for NSAS sources, whose bytes are kept apart from edited
  identifiers, dates and constituents.

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
with independent readers (`jcamp`, `spc_spectra`, `openpyxl`, `wai.spectralio`).
NSAS sources are rewritten byte for byte.
