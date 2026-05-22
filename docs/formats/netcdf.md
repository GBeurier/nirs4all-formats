# NetCDF NIRS Datasets

Status: experimental.

The NetCDF reader uses the pure-Rust `netcdf-reader` crate. It currently maps
simple NIRS NetCDF datasets with:

- a 2-D `spectra` variable shaped `sample x wavelength`;
- a 1-D `wavelengths` axis variable matching the spectral dimension;
- optional 1-D target variables matching the sample dimension.

It also carries two schema-specific atmospheric/sun-photometer paths that are
not generic NetCDF NIRS datasets:

- Microtops MAN AOT series;
- local ARM MFRSR b1 7-channel irradiance/voltage/ratio time series.
- local ARM SURFSPECALB derived 6-filter surface-albedo time series.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/netcdf/synthetic_nirs.nc` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein` |
| `samples/microtops/microtops_arc_msm114_2.nc` | 378 | wavelength, `nm`, 5 AOT channels | `aot`, `aot_std` | none |
| `samples_local/mfr/arm_mfrsr_sgp_E11_20210329.nc` | 4,320 | wavelength, `nm`, 7 filters | hemispheric/diffuse/direct irradiance, alltime voltage, direct/diffuse ratio | none |
| `samples_local/netcdf/arm_nsa_surfspecalb_20160609.nc` | 986 | wavelength, `nm`, 6 filters | `surface_albedo` | none |

Global attributes are preserved under `metadata.global_attributes` when the
pure-Rust metadata stack can decode them. The reader emits one `SpectralRecord`
per sample row or per non-missing time row for derived time-series products.

The Microtops MAN NetCDF path is intentionally narrower than the generic NIRS
schema. The reader discovers root-level `aot_<wavelength>` variables and sorts
them into a wavelength axis before validating series lengths. The committed
PANGAEA MSM114/2 fixture is NetCDF4/HDF5 with contiguous `aot_380`, `aot_440`,
`aot_500`, `aot_675` and `aot_870` series plus matching `*_std` series. The
primary `aot` array and record carry the `aerosol_optical_thickness` signal
type, while `aot_std` carries the `uncertainty` signal type.

The current `hdf5-reader` 0.5 high-level API mis-decodes the shared attribute
heap on this particular NetCDF4 layout, which causes per-variable header
resolution to fail for any 1-D dataset that carries shared `standard_name` or
`coordinates` attributes (the five AOT channels, `lat`, `lon`, `cwv` and
`angstrom_exp`). When that happens, the reader falls back to a *generic*
contiguous-layout decoder that:

1. Scans the file bytes for fractal-heap hard-link records
   (`<name_len:u8><name:UTF-8><object_header_addr:u64_LE>`) whose 8-byte
   address points to a valid `OHDR` signature elsewhere in the file.
2. Resolves each candidate dataset through
   `Hdf5File::get_or_parse_header(addr)` to obtain `DataLayout::Contiguous {
   address, size }`, `Dataspace`, and `Datatype` messages.
3. Reads the contiguous payload through the file storage and decodes it
   according to the on-disk byte order. Only 1-D primitive datasets
   (`f64`/`i64`) with a `DataLayout::Contiguous` are accepted; chunked,
   compact, compound, VLEN, or non-numeric datasets are explicitly rejected.

The fallback is keyed on standard HDF5 metadata, not on file hashes or
byte offset tables. It emits `microtops_man_netcdf_contiguous_layout_fallback`
on every record whenever at least one variable went through it, alongside the
existing `microtops_man_netcdf_experimental` marker.

Global attributes are recovered from the same `Hdf5LayoutFallback` byte buffer
when `hdf5-reader`'s high-level attribute iterator fails on the root group.
The byte-scan path only recovers fixed-length ASCII string attributes encoded
as `<name>\0<class_word=0x13><size:u32_LE><scalar_dataspace=0x02><data>`;
variable-length string attributes (`authors`, `reference`) and numeric scalar
attributes are intentionally skipped. When that path is used the reader emits
`microtops_man_netcdf_global_attributes_byte_scan` so downstream consumers
know the attribute map came from a positional decoder.

The ARM MFRSR path is validated locally only. It maps filter variables
`*_filter1..7` onto a wavelength axis from `centroid_wavelength` attributes,
emits one record per `time` row, and preserves per-signal QC arrays in metadata.
When a sibling ARM QC YAML sidecar is present, the reader adds it as a
`qc_sidecar` provenance source and maps suspect/incorrect time ranges to
per-record `arm_mfrsr_sidecar_*` quality flags.
The ARM SURFSPECALB path is also local-only and adjacent: it maps the derived
`surface_albedo_mfr_narrowband_10m(time, filter)` product, drops rows where all
filters are missing (`-9999`), and emits a reflectance-like `surface_albedo`
signal.

## Dispatch Boundaries

NetCDF is a container. The reader probes NetCDF classic and HDF5-backed
containers, then validates the NIRS schema at read time. ANDI/MS
containers are detected via the strict `NcFile::from_bytes` path
(`netcdf.rs:1526`); the HDF5-backed fallback at
`read_netcdf4_hdf5_records` does not currently re-run the ANDI markers
check, so a NetCDF4-classic file that wraps an HDF5 container with the
canonical ANDI variable set would surface as "is not a supported NIRS
spectroscopy schema" rather than the canonical pyteomics/pyOpenMS
pointer. Other non-NIRS NetCDF files, such as weather datasets and the
committed PyrNet pyranometer fixture, are refused because they do not
contain a supported NIRS schema or known sun-photometer channel set.
Local ARM AOSMET is a weather product and remains a refusal case.

## Sidecar contract (M1, 2026-05-22)

NetCDF is decoded through `NcFile::from_bytes_with_options` in the
lossy fallback and `NcFile::from_bytes` in the strict path. The only
true companion file is the optional ARM MFRSR `<stem>.yaml` QC
sidecar:

- `open_path(path)` reads the NetCDF plus the optional YAML from disk.
- `open_with_sidecars(name, bytes, Arc<dyn SidecarResolver>)` decodes
  from in-memory bytes; the resolver may serve a `<stem>.yaml` or
  `<prefix>.yaml` (where the trailing `_YYYYMMDD` is stripped) entry.
  Absence of the YAML is silent — QC just doesn't fire.
- `open_bytes(name, bytes)` works when no QC sidecar is needed.

Robustness (F2, 2026-05-23): the QC YAML parser now treats tabs as
2-column tab stops and strips ` #`-prefixed inline comments before
parsing, so the canonical ARM YAML format plus tab-indented variants
both decode. QC range bounds are parsed as absolute UTC seconds since
the Unix epoch; the per-sample comparison reads the file's
`time:units` CF string (e.g. `seconds since 2021-03-29 00:00:00`),
converts the YAML bounds into the same epoch, and compares
absolutes. When `time:units` cannot be parsed the rule falls back to
the legacy seconds-within-day match — single-day b1 files still
produce the right flags, multi-day files just no-op rather than
silently mismatching.
