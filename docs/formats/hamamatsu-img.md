# Hamamatsu HPD-TA IMG

> **Status:** Experimental · **Vendor:** Hamamatsu · **Extensions:** `.img`

Hamamatsu `.img` files from HPD-TA streak-camera systems are 2D time-resolved
images, not point-sample NIR spectra. They are decoded here mainly for
disambiguation and because the core record model can represent a `y,x` signal:
`x` is wavelength or pixel position, while `y` is a secondary time or
detector-position axis. This format sits outside the core NIRS point-spectrum
scope and is kept explicitly adjacent.

## Instruments & software

Written by Hamamatsu HPD-TA software for streak-camera acquisitions — focus,
operate, photon-counting and shading-reference modes, with calibrated or
uncalibrated axes.

## File structure

The reader sniffs by `.img` extension plus a 64-byte `IM` little-endian header
whose magic is followed by an `[Application]`/`HPD-TA` comment marker. The header
carries the comment length, image width/height, offsets, file type
(`0`=8-bit, `2`=16-bit, `3`=32-bit; `1`=compressed is refused), channel counts and
a timestamp. A UTF-8 HPD-TA comment block follows the header, then a
`width * height` unsigned payload; calibration tables referenced by `#offset,count`
live later in the file as float32 arrays.

## What nirs4all-formats extracts

- **Signals** — one record holding a single 2D signal with dimensions `y,x`,
  one record per file. 8-bit, 16-bit and 32-bit unsigned payloads are decoded.
- **Axis** — the X axis is reversed to ascending wavelength order (matching
  RosettaSciIO) when calibrated, otherwise kept as a pixel index (`px`). The
  secondary Y axis is calibrated time (typed `time`, e.g. `us`/`ns`/`ps`) when a
  time calibration is present, otherwise an uncalibrated detector-position
  `index`.
- **Metadata** — the secondary Y axis values are stored in metadata.
- **Provenance & warnings** — every file emits a warning identifying it as a
  streak-camera 2D signal: time-axis files use
  `hamamatsu_img_secondary_time_axis_in_metadata`, focus-style files use
  `hamamatsu_img_y_axis_is_detector_position`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Operate / photon-counting / shading (calibrated time Y) | Experimental | Y axis typed `time` (`us`/`ns`/`ps`). |
| Focus mode (vertical CCD position Y) | Experimental | Y axis is detector position in pixels. |
| Uncalibrated X and Y | Experimental | X kept as pixel index (`px`). |
| `file_type=1` compressed | Detected / refused | The unused compressed variant is refused. |

## Limitations & known gaps

- This is an adjacent 2D imaging format, not a NIRS point-spectrum reader; it is
  decoded only so the registry can recognise and represent these files.
- It stays explicitly adjacent until a point-sample spectral Hamamatsu export is
  targeted; the unused compressed `file_type=1` variant is not implemented.

## Reference readers

Fixture values and axes cross-checked against `rsciio.hamamatsu` 0.13.0.
RosettaSciIO is GPL-3.0 and is used only as an external conformance reference, not
as a runtime dependency.

## Samples & validation

Five fixtures under `samples/hamamatsu/` cover focus, operate, photon-counting,
shading-reference and uncalibrated-X modes (all single records, `512 x 672` or
`508 x 672`). Operate/photon-counting/shading files expose calibrated time Y axes
(`us`/`ns`/`ps`, typed `time`); focus and uncalibrated-X files keep detector or
pixel-index axes. The probe reports `hamamatsu-img` at `Confidence::Definite`.
