# USGS SPECPR / PRISM

Historical binary format from USGS Spectroscopy Lab. Practical approach: convert to ENVI/ASCII once and ingest the converted form.

## Samples

| File | Source | License |
|---|---|---|
| `asphalt_gds366.27407.asc` | [`ns-bak/splib06.library@master/ASCII/A/asphalt_gds366.27407.asc`](https://github.com/ns-bak/splib06.library/blob/master/ASCII/A/asphalt_gds366.27407.asc) | U.S. Government public domain (USGS splib06a mirror) | One asphalt spectrum (`gds366`) from the USGS splib06a spectral library, converted to ASCII. |

## Parser hints

- The full binary `splib06a` (52 MB) is also in the same upstream repo; we don't ship it but you can fetch on demand if testing the binary path.
- Use the USGS `specpr` tools (or `xylib`) to convert binary SPECPR records to ASCII or ENVI SLI.
- ASCII export format: `(record_no, sample_name) wavelength reflectance` columns. See the file's leading lines for the actual header convention.
- For v1, **only support ASCII / ENVI converts**; don't reimplement the SPECPR binary reader unless there is a downstream user.
