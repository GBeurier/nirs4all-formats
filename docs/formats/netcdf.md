# Generic NetCDF NIRS Datasets

> **Status:** Supported (scoped) · **Vendor:** Vendor-neutral · **Extensions:** `.nc`, `.cdf` · **Feature flag:** `fmt-hdf5`

NetCDF is a self-describing scientific container (classic and HDF5-backed)
widely used for spectral and atmospheric datasets. This reader maps the common
NIRS `spectra + wavelengths` schema plus a few schema-specific
atmospheric / sun-photometer products, and recognises ANDI/MS chromatography
containers so it can refuse them with a useful pointer.

## Instruments & software

Vendor-neutral. The generic path targets NetCDF written by NIRS pipelines and
research datasets. Additional schema-specific paths cover Microtops MAN aerosol
series and (local-only) DOE ARM MFRSR and SURFSPECALB atmospheric products.
Non-NIRS NetCDF such as weather datasets and the PyrNet pyranometer fixture are
refused.

## File structure

Detected by the `.nc` / `.cdf` extension; the schema is validated on read. The
classic path decodes through the pure-Rust `netcdf-reader` crate, while
HDF5-backed NetCDF4 containers are decoded through `hdf5-reader` (both gated
behind `fmt-hdf5`). The reader also exposes the sidecar resolver
(`read_bytes_with_sidecars`): the only true companion file is an optional ARM
MFRSR `<stem>.yaml` QC sidecar, which is served by the resolver in the
in-memory path and read from disk by `open_path`; its absence is silent.

The generic NIRS schema is a 2-D `spectra` variable shaped `samples x wavelength`
with a 1-D `wavelengths` axis variable and optional 1-D target variables.

## What nirs4all-formats extracts

- **Signals** — the `spectra` variable for the generic schema; for the
  atmospheric paths, discovered channel sets (e.g. Microtops `aot_<wavelength>`
  series, ARM MFRSR hemispheric/diffuse/direct irradiance plus voltage and
  ratio channels, ARM SURFSPECALB `surface_albedo`).
- **Axis** — the wavelength axis variable (or, for Microtops, a wavelength axis
  assembled by sorting the `aot_<wavelength>` variable names).
- **Targets** — 1-D variables matching the sample dimension.
- **Metadata** — global attributes under `metadata.global_attributes` when the
  pure-Rust stack can decode them; per-signal QC arrays for the ARM paths.
- **Provenance & warnings** — source file + SHA-256; the ARM QC YAML is added as
  a `qc_sidecar` source, mapping suspect/incorrect time ranges to per-record
  `arm_mfrsr_sidecar_*` quality flags.

The reader emits one `SpectralRecord` per sample row, or per non-missing time
row for the derived time-series products.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Generic `spectra` + `wavelengths` schema | Supported | One record per sample row. |
| Microtops MAN AOT NetCDF4/HDF5 series | Supported (scoped) | `aot` typed as aerosol optical thickness; `aot_std` as uncertainty. |
| ARM MFRSR b1 7-channel time series | Experimental (local-only) | Irradiance/voltage/ratio channels + optional QC YAML sidecar. |
| ARM SURFSPECALB 6-filter surface albedo | Experimental (local-only) | Reflectance-like `surface_albedo`; all-missing rows dropped. |
| ANDI/MS chromatography NetCDF | Detected / refused | Recognised by ANDI variable markers; refused as non-NIRS. |
| Real generic NIRS NetCDF schemas | Planned | Broader real-world schemas and QC handling still wanted. |

## Limitations & known gaps

- ANDI/MS containers are detected via the strict NetCDF path and refused with a
  message pointing to `pyteomics`, PyMassSpec or `pyOpenMS`. A NetCDF4-classic
  file wrapping an HDF5 ANDI container is not re-checked in the HDF5 fallback, so
  it surfaces as an unsupported-schema error rather than the canonical pointer.
- For some NetCDF4 layouts, `hdf5-reader` 0.5 mis-decodes the shared attribute
  heap; the reader then falls back to a generic contiguous-layout decoder keyed
  on standard HDF5 metadata (scanning fractal-heap hard-link records, resolving
  each candidate dataset's contiguous layout, dataspace and datatype), emitting
  `microtops_man_netcdf_contiguous_layout_fallback`. That path recovers only
  fixed-length ASCII string attributes; variable-length and numeric scalar
  attributes are skipped, signalled by `microtops_man_netcdf_global_attributes_byte_scan`.
- The ARM MFRSR and SURFSPECALB paths are validated locally only (ARM Data Use
  Policy); a redistributable MFR-7/MFRSR `.OUT` dump and broader ARM mapping are
  still wanted.
- Generic NIRS NetCDF support needs real-world schemas with QC and multi-signal
  groups to harden it.

## Reference readers

`netcdf-reader`, `xarray`, the netCDF library and the ARM `act` toolkit read the
same files; ANDI/MS belongs to `pyteomics` / `pyOpenMS`. nirs4all-formats adds the
NIRS schema validation, signal typing and provenance.

## Samples & validation

Fixtures: `samples/netcdf/synthetic_nirs.nc` (50 records, `nm` axis,
`absorbance`/`protein`) and `samples/microtops/microtops_arc_msm114_2.nc`
(PANGAEA MSM114/2, CC-BY-4.0; 5 AOT channels + `*_std`), both golden-backed; the
PyrNet fixture (`samples/netcdf/pyrnet_to_l1a_output.nc`) is a locked non-NIRS
refusal. Local-only fixtures under `samples_local/` cover the ARM MFRSR b1 file
(4,320 observations x 7 filters, with QC YAML sidecar) and the ARM SURFSPECALB
product (986 useful rows x 6 filters); ARM AOSMET remains a refusal. Probe
reports `netcdf-nirs` at `Confidence::Likely`, or `andi-ms-netcdf` at
`Confidence::Definite` for refused chromatography containers.
