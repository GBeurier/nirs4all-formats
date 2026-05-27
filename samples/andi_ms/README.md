# ANDI MS / `.cdf` (NetCDF chromatography)

ASTM E1947 ANDI MS — the chromatography-MS standard. Listed in `FORMATS.md` §3 as the **chromatography**, not NIR/FTIR standard. Included here so the NIRS loader can detect and refuse cleanly.

## Samples

| File | Source | License |
|---|---|---|
| `gc01_0812_066.cdf` | [`PyMassSpec/PyMassSpec@master/tests/data/gc01_0812_066.cdf`](https://github.com/PyMassSpec/PyMassSpec/blob/master/tests/data/gc01_0812_066.cdf) | **GPLv2** (fixture data; the GPL applies to the *reader code*, so freely usable as a fixture but **do not vendor `PyMassSpec` source code** into this repo). | Real Agilent 5975C MSD + 7890A GC export via ChemStation → ANDI. 7.6 MB. |

## Parser hints

- File is **NetCDF-3 classic** (CDF magic `CDF\x01`). Open with `scipy.io.netcdf_file` or `netCDF4`.
- ANDI MS-specific variables: `scan_acquisition_time`, `total_intensity`, `mass_values`, `intensity_values`, `point_count`.
- `nirs4all-formats` detects these names and refuses the container as chromatography/MS data instead of coercing `m/z` arrays into the NIRS `SpectralRecord` model.
- Reference readers: [`pyteomics.openms.ANDIMS`](https://pyteomics.readthedocs.io/), `pymzml`, [`PyMassSpec`](https://github.com/PyMassSpec/PyMassSpec).
