# Thermo Nicolet OMNIC

> **Status:** Supported (scoped) · **Vendor:** Thermo Nicolet · **Extensions:** `.spa`, `.spg`, `.srs` (`.srsx` pending)

OMNIC is the native file format written by Thermo Nicolet's OMNIC software for
FT-IR / FT-NIR / Raman acquisitions. nirs4all-io reads single-spectrum `.spa`
files, grouped `.spg` files and the TGA/GC and rapid-scan `.srs` series layouts
through a reverse-engineered key-table parser modelled on SpectroChemPy.

## Instruments & software

Produced by Thermo Scientific OMNIC across the Nicolet FT-IR / FT-NIR and Raman
ranges, including hyphenated TGA-GC-IR workflows. The committed corpus is sourced
from SpectroChemPy documentation fixtures; rapid-scan series come from local-only
SpectroChemPy fixtures.

The **Thermo Antaris II** FT-NIR analyzer (RESULT / TQ Analyst software on the
OMNIC engine) writes the same `.spa` / `.spg` containers and is therefore read
here; its RESULT `.csv` / `.xlsx` exports route to the generic table / Excel
readers and `.spc` interchange to the [Galactic SPC reader](galactic-spc.md). A
branded Antaris fixture is still to be sourced.

## File structure

- **`.spa` / `.spg`** — detected by the ASCII magic `Spectral Data File`. The
  layout is a fixed header followed by a key table (count at offset 294, entries
  from offset 304, 16 bytes each). Each entry carries a key byte, a payload
  offset and a payload length. Key `02` points to the spectral header, key `03`
  to the float32 intensity block and key `6B` (107) to group spectrum titles and
  OMNIC timestamps. A `.spg` is recognised either by extension or by carrying
  more than one header key.
- **`.srs`** — detected by the magic `Spectral Exte File`. The TGA/GC layout is
  located from three `02 00 00 00 18 00 00 00 00 00` anchors, which fix the data
  header, background header and spectral-matrix offsets; the y/time axis length
  and bounds come from the data header.

## What nirs4all-io extracts

- **Signals** — `.spa` emits one `SpectralRecord`; `.spg` emits one record per
  sub-spectrum. The signal type is decoded from the header signal key:
  `absorbance`, `transmittance` (`%`), `reflectance` (`%`), `log(1/R)`,
  Kubelka-Munk, interferogram detector signal (`V`), and labelled Raman /
  photoacoustic intensities. `.srs` emits one 2D record with
  `dims = ["y", "x"]`.
- **Axis** — values are generated from the header `first_x` / `last_x` bounds and
  point count. The axis kind follows the header axis key: wavenumber (`cm-1`),
  wavelength (`nm` / `um`) or index. Wavenumber axes are emitted in their native
  descending order.
- **Metadata** — OMNIC title and timestamp, scan counts, zero-path difference,
  reference frequency, optical velocity, key-table offsets and (for series) the
  `series_variant`, `series_name` and `series_y_*` fields. The series y/time axis
  is preserved both as a first-class `["y", "x"]` coordinate (`min`,
  `AxisKind::Time`) and as `series_y_len` / `series_y_first_min` /
  `series_y_last_min` / `series_y_step_min` metadata.
- **Provenance & warnings** — every record carries a reverse-engineering warning
  (`nicolet_omnic_reverse_engineered_key_table` or the matching series warning)
  plus source file and SHA-256.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.spa` single spectrum | Supported | One record; semantic signal type from the header key. |
| `.spg` grouped spectra | Supported | One record per sub-spectrum, with per-spectrum titles. |
| `.srs` TGA/GC series | Supported | One 2D `["y", "x"]` record; minute/time y-axis. |
| `.srs` rapid-scan (raw / reprocessed) | Supported | Raw interferograms use a generated index axis until the model grows an interferogram-domain axis. |
| Other `.srs` series anchors | Detected / refused | Series magic without exactly three TGA/GC anchors is refused as an unsupported variant. |
| `.srsx` | Planned | No redistributable fixture yet. |

## Limitations & known gaps

- `.srs` support is intentionally limited to the layout fixed by the three
  TGA/GC anchors; other anchor patterns are refused explicitly rather than
  guessed.
- `.srsx` and additional high-speed / rapid-scan variants remain pending until a
  real fixture and reference export are available.
- Raw rapid-scan interferograms fall back to a generated index x-axis because the
  shared data model does not yet carry a richer interferogram-domain axis.

## Reference readers

The implementation follows the public reverse-engineering model used by
SpectroChemPy and `spa-on-python`; SpectroChemPy is the practical cross-check for
the `.spa`, `.spg` and `.srs` paths.

## Samples & validation

SPA/SPG/SRS fixtures live under `samples/nicolet_omnic/` and are golden-backed
with direct semantic tests, including the 2D matrices, offsets and `series_y_*`
metadata. Control fixtures include `2-BaSO4_0.SPA` (absorbance, `cm-1`, 11098
points), `wodger.spg` (2 records, 5549 points), `GC_Demo.srs` (1738 x-points x
788 y rows, transmittance) and `TGAIR.srs` (1868 x-points x 335 y rows). Three
local-only SpectroChemPy `.srs` files cover the `tg_gc`, `rapid_scan_raw` and
`rapid_scan_reprocessed` variants. The probe reports `nicolet-omnic` at
`Confidence::Definite` for standard extensions, and the series probe at
`Confidence::Possible` so dispatch can route to the read-time layout detector.
