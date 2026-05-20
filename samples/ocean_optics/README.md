# Ocean Optics / Ocean Insight

Multiple sub-formats: SpectraSuite CSV (non-comma), OceanView text, OceanView `.ProcSpec` (XML wrapping a binary spectrum with checksum), and the Jaz handheld series binaries (`.jaz`, `.JazIrrad`).

All samples here are from [`ropensci/lightr`](https://github.com/ropensci/lightr/tree/main/inst/testdata) (GPL-2). `lightr` is the de-facto reference parser.

## Samples

### SpectraSuite & OceanView text exports

| File | Notes |
|---|---|
| `OOusb4000.txt` | SpectraSuite text export — variant separator + multi-line header. |
| `OceanView.txt` | OceanView text export (newer software). |
| `spec.csv` | Generic CSV export. |
| `CRAIC_export.txt` | UV-Vis microspec text export (CRAIC; bonus). |
| `FMNH6834.00000001.Master.Transmission` | "Master.Transmission" export from a museum collection workflow — exercises non-standard extensions. |

### OceanView `.ProcSpec` (proprietary binary container)

`.ProcSpec` is an OceanView container: an XML wrapper around a binary spectrum block with a CRC checksum that `lightr` validates. Layout drifts across OceanView versions and OS encoding.

| File | Notes |
|---|---|
| `OceanOptics_Windows.ProcSpec` | OceanView export from Windows OceanView (101 KB). |
| `OceanOptics_Linux.ProcSpec` | OceanView export from Linux OceanView — different binary layout. |
| `whiteref.ProcSpec` | A reference-spectrum `.ProcSpec` (white reference). |

### Ocean Optics `.spc` (OceanView flavour)

| File | Notes |
|---|---|
| `OceanOptics.spc` | OceanView export with Galactic SPC-compatible payload; routed through the Galactic SPC reader after header validation, not by extension alone. |

### Ocean Optics JCAMP-DX exports

| File | Notes |
|---|---|
| `OceanOptics_period.jdx` | JCAMP-DX export with period decimal separator (English locale). |

### Jaz handheld series (binary)

| File | Mode |
|---|---|
| `jazspec.jaz` | Standard `.jaz` binary spectrum |
| `irrad.JazIrrad` | Absolute irradiance `.JazIrrad` binary |

## Parser hints

- `.ProcSpec` is **zip-like internally** in many revisions — `lightr` first tries to unzip, then falls back to direct XML parsing. CRC mismatch is a soft warning, not a fatal error.
- The OceanOptics `.spc` fixture is intentionally disambiguated by header magic; do not route `.spc` files by extension alone.
- Reference reader: R [`lightr`](https://github.com/ropensci/lightr) (see [`lr_parse_procspec()`](https://docs.ropensci.org/lightr/reference/lr_parse_procspec.html)). No maintained Python port yet.
- Locale matters: SpectraSuite/OceanView use either `.` or `,` as decimal separator depending on Windows locale. See `lightr`'s non-English fixtures.
