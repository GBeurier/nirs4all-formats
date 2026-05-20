# Consumer Physics SCiO (handheld DLP-based NIR)

Consumer Physics's SCiO is a pocket molecular sensor (740-1070 nm, ~330 pixels) that
exports measurements as CSV through its developer-mode app. The CSV layout is
documented but the official SDK is credentialed; community fixtures come from the
reverse-engineering project [`kebasaa/SCIO-read`](https://github.com/kebasaa/SCIO-read).

## Samples

All from [`kebasaa/SCIO-read@master/01_rawdata`](https://github.com/kebasaa/SCIO-read/tree/master/01_rawdata), distributed under **GPL-3**.

| File | Size | Notes |
|---|---|---|
| `scio_app_scan.csv` | 7 KB | Single-scan export from the SCiO developer app. Three sections per the documented schema: the reflectance spectrum `R`, the raw sample signal `S`, and the raw calibration `C`. Wavelength axis is in the header. |
| `scio_calibration_plate_Polypen.csv` | 5 KB | A SCiO scan of a PolyPen calibration plate — useful as a white-reference paired-fixture for downstream reflectance recomputation. |
| `scio_scans_from_tech_support.csv` | 1.7 MB | Multi-scan dump shipped by Consumer Physics support, kept as the bulk fixture: ~100+ measurements with their metadata columns (timestamp, model, app version) followed by the three SCiO sections per scan. Confirms the loader handles the long-form "many scans appended" variant. |

## Parser hints

- Header rows include `key,value` metadata pairs (device id, user, app version, timestamp), then a row of section markers (`R`, `S`, `C`).
- Spectral block is **740-1070 nm** with ~330 channels (DLP-MEMS Hadamard scan). Resolution is ~10 nm.
- The native SCiO file format is hidden behind cloud API; the documented public path is the developer-mode CSV export, which is what we ship here.
- Cross-reference fixture: same SCiO instrument family is exercised at the Excel-export side via the UvA forensic Figshare dataset (`samples/excel/scio_forensic_P_avg.xlsx`) — the two share the 740-1070 nm wavelength grid.
