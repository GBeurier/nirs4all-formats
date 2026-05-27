# Princeton TriVista TVF

> **Status:** Supported · **Vendor:** Princeton Instruments · **Extensions:** `.tvf`

TVF is the file format written for Princeton Instruments TriVista triple-stage
Raman spectrometers. Despite the historical "binary spectroscopy file" wording,
the observed files are XML documents with ASCII frame payloads. nirs4all-formats emits
one record per frame, covering single spectra, time series, line scans, maps and
Step-and-Glue acquisitions. The format is Raman, adjacent to the core NIRS
point-spectrum scope.

## Instruments & software

Produced by TriVista software for Princeton Instruments TriVista
multi-spectrometer (triple-stage) Raman systems, including single/multi-frame
acquisitions, time series, line scans, XY maps and Step-and-Glue stitching.

## File structure

The reader sniffs `.tvf` by content (`<XmlMain` plus `TriVista-File`). The
document is an `XmlMain` / `Document` container:

- spectral values live in semicolon-separated `Frame` text nodes;
- the spectral axis is a pipe-separated `xDim/Calibration@ValueArray` whose first
  field declares the point count (non-uniform axes supported);
- `xDim@Length` is checked against the calibration array, and `Frame@xDim`
  against the resolved axis length;
- the escaped `InfoSerialized` attribute carries `Experiment`, `Detector`,
  `Calibration`, `X-Axis`, `Y-Axis` and numbered `Spectrometer` groups.

## What nirs4all-formats extracts

- **Signals** — one `SpectralRecord` per frame; signal units are inferred only
  when `DataLabel` clearly denotes counts.
- **Axis** — values from `xDim/Calibration`; the axis kind follows
  `Calibration@Unit` (not assumed wavelength-only). Axis metadata keeps the
  calibration label, normalised unit, display unit, calibration type and laser
  wavelength when present.
- **Metadata** — `InfoSerialized` X/Y navigation mapped to `spatial_x`,
  `spatial_y` and frame indices (units preserved when present, otherwise reported
  as `unknown`); detector and one-or-many spectrometer groups promoted to
  metadata (serial/model/stage/focal length/groove density/order arrays for
  triple-stage setups); `detector_temperature_c`; and Windows FILETIME frame
  timestamps mapped to `time_filetime_100ns` and `elapsed_time_seconds`.
- **Step-and-Glue** — emits the primary glued spectrum plus each child document
  as separate records, preserving the source windows.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Single / multi-frame, averaged or not | Supported | One record per frame. |
| Time series | Supported | Elapsed time derived from frame timestamps. |
| Line scan / XY map | Supported | X/Y navigation in metadata. |
| Multi-spectrometer (triple-stage) | Supported | Numbered spectrometer groups promoted. |
| Step-and-Glue | Supported | Glued primary plus child windows as separate records. |

## Limitations & known gaps

- Objective metadata and unsupported hardware-specific `InfoSerialized` branches
  are left to a later conformance pass.
- Spatial units absent from the current fixtures are reported as `unknown`
  rather than invented.
- Variants outside the committed corpus have no explicit scope decision yet, and
  automated full-array conformance against `rsciio.trivista` is still pending.

## Reference readers

Layout and fixture behaviour cross-checked against `rsciio.trivista` 0.13.0.
RosettaSciIO defaults to the glued spectrum for Step-and-Glue; nirs4all-formats also
emits child documents so low-level consumers can inspect the source windows.

## Samples & validation

The RosettaSciIO `.tvf` corpus under `samples/raman_trivista/` is fully
golden-backed: single/multi-frame averages, two-accumulation variants,
multi-spectrometer metadata, a 20-record Step-and-Glue file (glued primary plus
19 child windows), a timestamp-derived time series, a 21-point line scan and a
9-by-9 (81-record) XY map. The remaining items are conformance and metadata
work, not sample blockers.
