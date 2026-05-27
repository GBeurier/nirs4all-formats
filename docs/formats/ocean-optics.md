# Ocean Optics / Ocean Insight

> **Status:** Supported (scoped) · **Vendor:** Ocean Optics / Ocean Insight · **Extensions:** `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec` (also `.jdx`, `.spc` via sibling readers)

Ocean Optics / Ocean Insight spectrometers export through SpectraSuite,
OceanView, OOIBase32 and the Jaz firmware. nirs4all-io reads the family's ASCII
exports and the OceanView `.ProcSpec` ZIP archive, emitting one `SpectralRecord`
per file.

## Instruments & software

Produced by SpectraSuite, OceanView, OOIBase32 and Jaz across the Ocean Optics /
Ocean Insight range, plus CRAIC two-column exports. Committed fixtures span
SpectraSuite, OceanView, Jaz / JazIrrad, CRAIC, Master.Transmission, two-column
CSV and Linux/Windows `.ProcSpec` archives.

The **Flame-NIR / Flame-NIR+** (InGaAs, ~950-1650 nm) is part of this family. It
exports only through OceanView and produces the same ASCII / `.ProcSpec` / `.jdx`
layouts decoded here, so no dedicated reader is needed; a Flame-NIR fixture
covering the InGaAs axis (vs the current CCD-range fixtures) is still to be
sourced.

## File structure

- **ASCII exports** — text files keyed by header banners such as
  `SpectraSuite Data File`, `OOIBase32 Version`, `Jaz Data File` or
  `Jaz Absolute Irradiance File`, with a `>>>>>Begin … Data<<<<<` marker before
  the numeric block. CRAIC and headerless two-column CSV exports are recognised
  by their numeric-pair content.
- **`.ProcSpec`** — a ZIP archive (magic `PK`) containing `ps_*.xml` (the spectral
  arrays and acquisition metadata), `OOIVersion.txt` and, when present,
  `OOISignatures.xml`. The reader validates the SHA-512 signature of the XML
  member when the signature file is present.

## What nirs4all-io extracts

- **Signals** — two-column exports emit one signal (`processed`, `reflectance`,
  `transmittance` or `irradiance`) chosen from headers and file name. Jaz
  multichannel exports map `W/D/R/S/P` to wavelength axis, `dark_reference`,
  `white_reference`, `sample` and a processed signal (`irradiance` for absolute
  irradiance files, otherwise `processed`). `.ProcSpec` archives map
  `channelWavelengths`, source `pixelValues`, `darkSpectrum`,
  `referenceSpectrum` and `processedPixels` to the same signal set, with the
  processed type (`transmittance` / `reflectance` / `absorbance` / `processed`)
  taken from the OceanView core processor / `yUnits` class.
- **Axis** — wavelength in `nm` for all variants.
- **Metadata** — vendor key/value lines under `metadata.vendor` with normalized
  key names; the source file name is stored too, because some workflows encode
  the measurement type in the extension rather than the header. `.ProcSpec`
  archives additionally record integration time, boxcar width, averages,
  electrical-dark / non-linearity flags, serial numbers, spectrometer class and
  pixel count, timestamp, archive members and the SHA-512 `signature_status`.
- **Provenance & warnings** — source file, SHA-256 and signature warnings for
  `.ProcSpec` (missing / mismatched signature).

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| SpectraSuite / OceanView text | Supported | `>>>>>Begin … Data<<<<<` numeric block. |
| OOIBase32 `*.Master.Transmission` | Supported | Two-column transmittance. |
| Jaz `.jaz` / `.JazIrrad` | Supported | `W/D/R/S/P` columns; absolute-irradiance → `irradiance`. |
| CRAIC two-column text | Supported | Reflectance export. |
| Two-column Ocean-style CSV | Supported | Headerless numeric-pair CSV. |
| OceanView `.ProcSpec` ZIP | Supported | XML arrays + SHA-512 signature verification. |
| Ocean Optics JCAMP `LINK` (`.jdx`) | Supported (via JCAMP reader) | Keeps sample/dark/reference arrays and computes processed transmittance. |
| Ocean Optics `.spc` | Supported (via Galactic SPC reader) | Committed fixture is the Galactic new-LSB explicit-X layout. |
| QE Pro / Maya / Apex exports | Planned | Awaiting redistributable samples. |

## Limitations & known gaps

- The first tranche does not parse a distinct Ocean-specific `.spc` binary or any
  Ocean JCAMP beyond what the JCAMP reader already decodes. The committed `.spc`
  fixture is routed to the [Galactic SPC reader](galactic-spc.md) because its
  layout belongs to that family rather than a separate Ocean container, and Ocean
  JCAMP `LINK` exports are routed to the JCAMP-DX reader.
- QE Pro, Maya and Apex export variants, and any non-Galactic Ocean `.spc`,
  remain pending until redistributable samples exist.
- Semantic typing of generic text / Jaz `processed` spectra is limited when the
  export records the processing mode in metadata rather than column labels.

## Reference readers

`lightr` is the practical external reference for this family (with `pavo` for R);
both stay conformance-only because the Rust core is MIT.

## Samples & validation

All 12 committed Ocean Optics / Ocean Insight data fixtures under
`samples/ocean_optics/` are golden-backed, with direct semantic tests over the
text, CSV, Jaz, CRAIC, Master.Transmission and ProcSpec families. Control values
include `OOusb4000.txt` (3648 points, `processed`, `178.65 -> 888.37 nm`, last
`-12.792`), `CRAIC_export.txt` (3761 points, `reflectance`, first `13.3999`, last
`169.6574`), `jazspec.jaz` (2048 points, four channels, processed last
`13.679238`), `irrad.JazIrrad` (2048 points, `irradiance` last `3.643908`) and
`OceanOptics_Linux.ProcSpec` (3648 points, `transmittance` `0.0 -> 125.074331`).
Ocean JCAMP is validated through the JCAMP-DX reader and the committed `.spc`
through the Galactic SPC reader. Probe confidence is `Definite` for the
`.ProcSpec` archive and the banner-keyed ASCII exports, and `Likely` for CRAIC
and two-column CSV.
