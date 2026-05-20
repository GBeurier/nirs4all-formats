# ANDI/MS NetCDF

Status: detected and refused.

ANDI/MS is the ASTM E1947 chromatography and mass-spectrometry NetCDF profile.
It is not a NIRS / optical spectroscopy interchange format: its variables model
scan acquisition time, `m/z` values and ion intensities rather than a
wavelength-indexed molecular spectrum.

The native NetCDF reader now detects ANDI/MS containers by their standard
variables and returns a specific error directing users to
`pyteomics.openms.ANDIMS`, `PyMassSpec` or `pyOpenMS`.

## Covered Fixtures

| Fixture | Behavior | Detection markers |
|---|---|---|
| `samples/andi_ms/gc01_0812_066.cdf` | refused | `scan_acquisition_time`, `total_intensity`, `mass_values`, `intensity_values`, `point_count` |

## Decision

Do not coerce chromatography/MS scans into `SpectralRecord`. If ANDI/MS support
is needed later, it should live behind an explicit adjacent MS/chromatography
model or an adapter that converts a deliberate user-selected signal into a
NIRS-compatible table.
