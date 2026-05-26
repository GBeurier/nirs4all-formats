# ANDI/MS NetCDF

> **Status:** Detected / refused · **Vendor:** ASTM / vendor-neutral · **Extensions:** `.cdf`, `.nc`

ANDI/MS is the ASTM E1947 chromatography and mass-spectrometry NetCDF profile.
nirs4all-io recognises it for disambiguation but **refuses it on read**: it is not
a NIRS / optical-spectroscopy interchange format. Its variables describe scan
acquisition time, `m/z` values and ion intensities rather than a
wavelength-indexed molecular spectrum.

## Instruments & software

Exported by GC-MS / LC-MS acquisition and processing software as a vendor-neutral
exchange container following the ASTM E1947 (ANDI/MS) convention, stored in
classic NetCDF (`CDF\x01`/`\x02`/`\x05`) or HDF5-backed NetCDF4.

## File structure

A NetCDF container whose standard ANDI/MS variables identify it: detection keys
are `scan_acquisition_time`, `total_intensity`, `mass_values`,
`intensity_values` and `point_count`. The shared NetCDF reader handles both the
classic-CDF and the HDF5-backed NetCDF4 encodings.

## Why it is refused / where to go instead

The NetCDF reader sniffs `.nc` / `.cdf` files, and when at least four ANDI/MS
marker variables are present it tags the candidate `andi-ms-netcdf` at
`Confidence::Definite` so dispatch routes here rather than to the generic NIRS
NetCDF path. On read it returns a specific error naming the detected variables and
pointing to a chromatography/MS toolkit:

- **`pyteomics.openms.ANDIMS`** — ANDI/MS reader.
- **`PyMassSpec`** — GC-MS analysis in Python.
- **`pyOpenMS`** — full OpenMS bindings.

The reader does **not** coerce chromatography/MS scans into `SpectralRecord`. If
ANDI/MS support is ever needed it should live behind an explicit adjacent
MS/chromatography model, or an adapter that converts a deliberate, user-selected
signal into a NIRS-compatible table.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Classic NetCDF ANDI/MS (`CDF` magic) | Detected / refused | Sniffed from head markers; refused on read. |
| HDF5-backed NetCDF4 ANDI/MS | Detected / refused | Root-group datasets walked; same refusal. |

## Limitations & known gaps

- No decoding of any kind — this is a deliberate refusal, kept distinct from the
  generic NIRS NetCDF reader so a chromatography file is never silently parsed as
  an optical spectrum.

## Reference readers

`pyteomics`, `PyMassSpec` and `pyOpenMS` are the recommended tools and the
references named in the refusal message.

## Samples & validation

The fixture under `samples/andi_ms/` asserts the refusal behaviour:

| Fixture | Behaviour | Detection markers |
|---|---|---|
| `samples/andi_ms/gc01_0812_066.cdf` | refused | `scan_acquisition_time`, `total_intensity`, `mass_values`, `intensity_values`, `point_count` |
