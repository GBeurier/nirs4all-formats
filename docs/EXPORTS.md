# Exports

Every reader feeds every writer through `SpectralRecord`. There is no converter
per pair of formats. Exports never resample, reorder, rescale or convert units:
what a target cannot carry is **refused** or listed as a **loss** in the result.
[Conversions](#conversions) are planned here and computed by nirs4all-methods
before the export.

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
  zeros read as missing; both are declared. Constituents are declared on an
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

## Conversions

Formats decides what a target needs; nirs4all-methods computes every converted
value. `export_targets()` lists the `requirements` of each format: accepted
signal types and axis units, whether the axis must increase or be evenly spaced
(`even_axis`: `not_required`, `segments` up to `max_segments`, `known_grid`),
the known `grids` (WinISI 700 and 1050 points; NSAS 700, 1050, 1700 and 4200),
how other grids are treated, `max_points`, `missing_values` and whether the
conversions are written into the file.

`plan_conversion(records, target, options)` returns a plan without computing
anything:

| Field | Content |
|---|---|
| `status` | `ready`, `needs_answers`, `refused` or `unchanged` (written as it is) |
| `source_signal`, `records` | Signal converted and records holding it |
| `steps` | In order: `step`, `method_id` (none for selections), `params`, `inputs` (reference signals, kept point ranges, axis), signal type, percent flag and axis unit after the step, `description`, `notes` |
| `questions` | Missing information: `id` (the option that answers it), `prompt`, `choices`, `why` |
| `refusals` | `code`, plain-English `reason` and `fix` |
| `candidates` | Every signal, whether it can be converted and why |
| `output` | Suggested name (for example `absorbance (from reflectance)`), type, unit, axis unit |
| `measured_range`, `grid` | Measured axis limits in the output unit and the chosen grid |
| `notes` | Limits that apply, including untested vendor imports |

| Step | Method | When |
|---|---|---|
| `reflectance_from_references` | `preprocessing.signal_conversion.reflectance_from_references` | Raw counts, single beam or radiance with white (and dark) references of the same record and axis; R = (S - D) / (W - D) |
| `splice_correction` | `preprocessing.signal_conversion.splice_correction` | After references, when overlapping detectors or the `splices` option tell where detectors meet |
| `declare_signal_type` | none | An unknown signal type answered through `assume_signal_type` |
| `signal_conversion` | `to_absorbance`, `from_absorbance`, `kubelka_munk`, `from_kubelka_munk`, `percent_to_fraction` | The target or `signal_type` needs another signal type |
| `select_points` | none | Overlapping detectors (SVC): points that do not extend the axis are dropped |
| `axis_unit_conversion` | `preprocessing.resampling.axis_unit_conversion` | Another axis unit (nm, um, cm-1), or a decreasing axis to put in order |
| `resample` | `preprocessing.resampling.resampler` | The target needs a known or even grid; `extrapolate` is false and `bounds_error` true |

Vendor-computed reflectance or transmittance is preferred over a ratio of raw
counts; choose `signal` to recompute it. Options: `signal`, `signal_type`,
`axis_unit`, `grid` (`measured`, `even` or a grid id), `grid_step`, `grid_min`,
`grid_max`, `interpolation` (`linear`, `nearest`, `cubic`), `source_scale`
(`percent`, `fraction`), `assume_signal_type`, `absorbance_origin`, `splices`
and `axis_tolerance`.

A ratio to references is transmittance when the measurement mode, the
requested output or the vendor signals say so, otherwise reflectance with the
assumption stated. Percent or fraction comes from units that state a scale
(`%`, `1`, `fraction`); units that only name the signal (`TRANSMITTANCE`) or no
unit lead to the `source_scale` question, which shows the observed value range.

Refused: raw counts without references, ratios of raw counts measured with
different integration times (normalized counts and radiance are accepted),
derivatives and preprocessed signals, interferograms, references themselves,
multidimensional signals, peak tables on any grid, cm-1 to wavelength for Raman
instruments (technique, reader or laser metadata) and for signals not
established as absorption or reflection spectra, grids outside the measured
range and NSAS spectra that cover none of its axes.

Limits: log 1/R is apparent absorbance; percent or fraction must be known (a
wrong answer shifts absorbance by 2); nothing is extrapolated; interpolation
smooths and down-sampling high-resolution FTIR can alias; axes convert as
vacuum wavelengths (nm = 10^7 / cm-1); the white reference is taken as 100 %;
vendor splice corrections may differ; files written for vendor software are not
tested in that software yet. Range checks of the planner convert only the two
limits of the measured axis.

Hosts record each applied step under the record metadata key `conversions`:
a list of `{signal, step, method_id, methods_version, params, notes}`. Writers
return the steps of the exported signals as `conversions`, write them as
`##$CONVERSION n` labels in JCAMP-DX and as `CONVERSIONn` lines in the SPC log,
keep them in `records-json`, and declare `conversions_not_written` elsewhere.

```bash
nirs4all-formats plan whiteref.ProcSpec --to winisi
nirs4all-formats plan scan.0 --to winisi --options '{"grid": "winisi-700"}'
```

```js
const plan = planConversion(records, "winisi", { source_scale: "percent" });
```

The [demo](https://formats.nirs4all.org/) runs plans with `@nirs4all/methods`,
shows the converted spectra before, after or next to the originals and exports
them.

## Checks

`crates/nirs4all-formats/tests/writers.rs` writes every decodable public sample
and reads it back. `crates/nirs4all-formats/tests/conversion_plans.rs` checks
plans on public samples and that grids never leave the measured range. `tests/conformance/test_writers.py` compares exported files
with independent readers (`jcamp`, `spc_spectra`, `openpyxl`, `wai.spectralio`).
NSAS sources are rewritten byte for byte.
