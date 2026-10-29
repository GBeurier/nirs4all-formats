# netcdf-rust

[![hdf5-reader crates.io](https://img.shields.io/crates/v/hdf5-reader.svg)](https://crates.io/crates/hdf5-reader)
[![hdf5-reader docs.rs](https://docs.rs/hdf5-reader/badge.svg)](https://docs.rs/hdf5-reader)
[![hdf5-writer crates.io](https://img.shields.io/crates/v/hdf5-writer.svg)](https://crates.io/crates/hdf5-writer)
[![hdf5-writer docs.rs](https://docs.rs/hdf5-writer/badge.svg)](https://docs.rs/hdf5-writer)
[![netcdf-reader crates.io](https://img.shields.io/crates/v/netcdf-reader.svg)](https://crates.io/crates/netcdf-reader)
[![netcdf-reader docs.rs](https://docs.rs/netcdf-reader/badge.svg)](https://docs.rs/netcdf-reader)
[![netcdf-writer crates.io](https://img.shields.io/crates/v/netcdf-writer.svg)](https://crates.io/crates/netcdf-writer)
[![netcdf-writer docs.rs](https://docs.rs/netcdf-writer/badge.svg)](https://docs.rs/netcdf-writer)

Pure-Rust readers and writers for HDF5 and NetCDF. The published library dependency graph has no C libraries or build scripts; internal `unsafe` is limited to read-only memory mapping and performance-critical decoding/copy paths.

Reference tests, benchmarks, fixture generators, and fuzz targets may use native
tooling such as NetCDF-C/HDF5 through dev-only dependencies. Those paths are not
part of the library runtime dependency graph. Writer output is validated against
the reference C libraries (libhdf5 via h5py, netcdf-c via netCDF4-python) in
addition to round-tripping through the sibling reader crates.

## Crates

| Crate | Description |
|---|---|
| `hdf5-core` | Shared HDF5 format model and checksums (Jenkins lookup3, Fletcher-32) |
| `hdf5-reader` | Low-level HDF5 decoder (superblock, object headers, B-trees, chunked I/O, filters) |
| `hdf5-writer` | HDF5 encoder (contiguous/compact/chunked layouts, groups, attributes, filters, datatypes) |
| `netcdf-core` | Shared NetCDF data model (types, attributes, dimensions, variables) |
| `netcdf-reader` | NetCDF reader supporting CDF-1/2/5 classic and NetCDF-4 (HDF5-backed) formats |
| `netcdf-writer` | NetCDF writer for CDF-1/2/5 classic and NetCDF-4 (HDF5-backed) formats |

## Feature support

| Feature | Read | Write |
|---|---|---|
| Classic CDF-1 / CDF-2 / CDF-5 | ✓ | ✓ |
| NetCDF-4 (HDF5-backed) | ✓ | ✓ |
| Chunked layouts (incl. unlimited dims) | ✓ | ✓ |
| Filters: deflate, shuffle, fletcher32 | ✓ | ✓ |
| Filters: lz4, nbit, scaleoffset | ✓ | — |
| Compound / enum / opaque / array / vlen / string datatypes | ✓ | ✓ |
| Dimension scales (`DIMENSION_LIST` + `REFERENCE_LIST`) | ✓ | ✓ |
| CF conventions (unpack, mask, calendars) | ✓ | n/a |
| szip filter, virtual datasets | — | — |

The minimum supported Rust version is 1.81. Reads are hardened against malformed
input: bounds-checked cursors, cycle detection in B-tree/heap/object-header
traversals, recursion-depth limits, and file-size-bounded allocation. The reader
crates are exercised by a corpus/property/fuzz suite in addition to
reference-oracle tests against the C libraries.

Not yet supported (contributions welcome): writing the szip / lz4 / nbit /
scaleoffset filters; typed compound-value writing; appending to or updating an
existing file in place; HDF5 virtual datasets; and object-header messages larger
than 64 KiB.

## Usage

```rust
use netcdf_reader::{NcFile, NcSliceInfo, NcSliceInfoElem};

let file = NcFile::open("era5.nc")?;
println!("format: {:?}", file.format());

for var in file.variables()? {
    println!("  var: {} {:?}", var.name(), var.shape());
}

// Read typed data (works for both classic and NetCDF-4)
let temp: ndarray::ArrayD<f32> = file.read_variable("temperature")?;

// Type-promoting read (any numeric type → f64)
let data = file.read_variable_as_f64("temperature")?;

// String variables (classic char arrays and NetCDF-4 NC_STRING)
let names = file.read_variable_as_strings("station_name")?;

// NetCDF-4 user-defined variables (enum, opaque, compound, array, vlen)
let quality = file.read_variable_user_defined("quality")?;

// CF conventions: unpack packed integer data (scale_factor + add_offset)
let unpacked = file.read_variable_unpacked("temperature")?;

// CF conventions: mask fill values + unpack in one call
let clean = file.read_variable_unpacked_masked("temperature")?;

// Hyperslab: read a single time step from a 4D variable
let sel = NcSliceInfo {
    selections: vec![
        NcSliceInfoElem::Index(0),                                        // time=0
        NcSliceInfoElem::Slice { start: 0, end: u64::MAX, step: 1 },     // all levels
        NcSliceInfoElem::Slice { start: 0, end: u64::MAX, step: 1 },     // all lat
        NcSliceInfoElem::Slice { start: 0, end: u64::MAX, step: 1 },     // all lon
    ],
};
let step: ndarray::ArrayD<f32> = file.read_variable_slice("temperature", &sel)?;

// Lazy iteration over time steps
for slice in file.iter_slices::<f32>("temperature", 0)? {
    let data = slice?;
    println!("  step shape: {:?}", data.shape());
}

// In-memory open with custom NC4 cache/filter options
let bytes = std::fs::read("era5.nc")?;
let file = NcFile::from_bytes_with_options(&bytes, netcdf_reader::NcOpenOptions {
    chunk_cache_bytes: 8 * 1024 * 1024,
    chunk_cache_slots: 257,
    metadata_mode: netcdf_reader::NcMetadataMode::Strict,
    #[cfg(feature = "netcdf4")]
    filter_registry: None,
})?;
```

Using `hdf5-reader` directly:

```rust
use hdf5_reader::Hdf5File;

let file = Hdf5File::open("data.h5")?;
let ds = file.dataset("/group1/temperature")?;
let data: ndarray::ArrayD<f64> = ds.read_array()?;

// Hyperslab selection
use hdf5_reader::{SliceInfo, SliceInfoElem};
let sel = SliceInfo {
    selections: vec![
        SliceInfoElem::Slice { start: 0, end: 10, step: 1 },
        SliceInfoElem::Index(5),
    ],
};
let slice: ndarray::ArrayD<f64> = ds.read_slice(&sel)?;

// String datasets
let labels = file.dataset("/labels")?.read_strings()?;
```

### Writing

`netcdf-writer` builds a file from dimensions, variables, and attributes and
serializes it to any classic or NetCDF-4 format:

```rust
use netcdf_writer::{NcAttrValue, NcFileBuilder, NcWriteOptions};

let mut builder = NcFileBuilder::new();
let time = builder.add_unlimited_dimension("time")?;
let x = builder.add_dimension("x", 3)?;

builder.add_attribute("title", NcAttrValue::Chars("example".into()))?;

let temp = builder.add_variable::<f32>("temp", &[time, x])?;
builder.add_variable_attribute(temp, "units", NcAttrValue::Chars("K".into()))?;
builder.write_variable(temp, &[280.0_f32, 281.0, 282.0, 283.0, 284.0, 285.0])?;

// AutoClassic picks CDF-1/2/5 from the schema; other options force a format.
let (_format, bytes) = builder.to_vec(NcWriteOptions::default())?;
std::fs::write("out.nc", bytes)?;

// NetCDF-4 (HDF5-backed) output:
use netcdf_writer::NcWriteFormat;
let (_format, nc4_bytes) =
    builder.to_vec(NcWriteOptions { format: NcWriteFormat::Nc4 })?;
```

`hdf5-writer` offers the same for raw HDF5 via `Hdf5Builder` / `DatasetBuilder`
/ `AttributeBuilder`.

## Features

**HDF5**
- Superblock v0-v3 and object header v1/v2 with checksum verification
- Compact, contiguous, and chunked layouts
- All chunk index types: v1/v2 B-tree, single-chunk, implicit, Fixed Array, Extensible Array
- Deflate, shuffle, Fletcher-32, N-Bit, ScaleOffset, and optional LZ4 filters
- Custom filters via `FilterRegistry`
- Fixed-length strings, HDF5 variable-length strings, and byte-vlen string datasets
- Dense-link resolution, soft-link resolution, optional external-link resolution, committed datatypes, global heap strings, and object references
- SOHM shared-message lookup, fractal heap managed/tiny/huge objects, and external raw data files
- Parallel chunk decoding, chunk caching, and object-header caching
- Range-backed opens via `Storage` backends (`BytesStorage`, `FileStorage`, `MmapStorage`)

**NetCDF**
- CDF-1, CDF-2, CDF-5, and NetCDF-4
- Automatic format detection
- Unified typed reads across formats
- Unified string reads for classic char arrays and NetCDF-4 string variables
- NetCDF-4 user-defined reads for enum, opaque, compound, fixed-size array,
  and non-string vlen variables, including custom borrowed decoders
- Type promotion to `f64`, unpacking, masking, and combined CF helpers
- Coordinate-variable lookup plus CF axis/time discovery when `cf` is enabled
- Exact CF time decoding for standard, proleptic Gregorian, noleap, all_leap,
  360_day, and Julian calendars when `cf` is enabled
- Slice reads, lazy slice iteration, and parallel NC4 slice reads
- Cache and filter configuration through `NcOpenOptions`, including in-memory and storage-backed opens

## Parallel-I/O Compatibility

This project reads files. It does not provide distributed parallel I/O APIs.

- PnetCDF-produced CDF-1, CDF-2, and CDF-5 files are supported as ordinary
  classic-format NetCDF files. PnetCDF's MPI-IO API surface is not implemented.
- NetCDF-C files created with `nc_create_par` or Parallel HDF5 are supported
  when the final file is a normal NetCDF-4/HDF5 file that uses HDF5 features
  supported by `hdf5-reader`. Parallel access mode is an open-time API concern,
  not a persistent file property.
- The Rayon APIs in this crate parallelize local decoding and independent byte
  range reads inside one process. They are not equivalent to MPI-IO collective
  or independent access modes.
- PnetCDF subfiling is out of scope for now because it is not an ordinary
  single-file CDF-1/2/5 dataset.

## Feature flags

Minimum supported Rust version: 1.81.

```toml
[dependencies]
netcdf-reader = "0.9.3"           # CDF-1/2/5 + NetCDF-4 (default)
netcdf-reader = { version = "0.9.3", default-features = false }  # CDF-1/2/5 only
netcdf-writer = "0.9.3"           # Classic + NetCDF-4 writer (default)
hdf5-writer = "0.9.3"             # Raw HDF5 writer
```

| Flag | Default | Description |
|---|---|---|
| `netcdf4` | yes | NetCDF-4 support via `hdf5-reader` |
| `rayon` | yes | Parallel chunk reading |
| `lz4` | yes | LZ4 filter support (hdf5-reader) |
| `cf` | no | CF Conventions helpers (axis identification, time decoding, CRS extraction, bounds) |

## External Raw Data Files

HDF5 external raw data files are not resolved by default. To allow them for
trusted files, opt in with a resolver rooted at the directory that should
contain the external data. The filesystem resolver rejects absolute paths and
`..` components. On Unix, it opens paths relative to the resolver root with
`openat` and `O_NOFOLLOW`, so symlinks are rejected rather than followed. The
same confinement is applied by `FilesystemExternalLinkResolver` when external
links are enabled. On non-Unix platforms, the filesystem resolvers fall back to
canonicalize-then-open and attacker-writable resolver roots are out of scope.

```rust
use std::path::Path;
use std::sync::Arc;

use hdf5_reader::{FilesystemExternalFileResolver, Hdf5File, OpenOptions};

let path = Path::new("data.h5");
let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
let file = Hdf5File::open_with_options(path, OpenOptions {
    external_file_resolver: Some(Arc::new(FilesystemExternalFileResolver::new(base_dir))),
    ..Default::default()
})?;
```

## Custom filters

Register filters before opening files:

```rust
use hdf5_reader::{Hdf5File, OpenOptions};
use hdf5_reader::filters::FilterRegistry;

let mut registry = FilterRegistry::new();
registry.register(32001, Box::new(|_filter, data, _elem_size| {
    // Custom decompression logic
    Ok(data.to_vec())
}));

let file = Hdf5File::open_with_options("data.h5", OpenOptions {
    filter_registry: Some(registry),
    ..Default::default()
})?;
```

## Testing

```sh
# Unit tests (no external dependencies)
cargo test --workspace

# Integration tests with generated fixtures
scripts/generate-fixtures.sh
cargo test --workspace
```

Small compatibility fixtures under `testdata/pnetcdf` and `testdata/parallel`
exercise standard CDF-1/2/5 and NetCDF-4 files matching layouts produced by
PnetCDF and parallel netCDF-C/HDF5 workflows. Reference C generators that use
those external parallel libraries live under `testdata/external`; they are not
library dependencies.

For reference comparisons and current benchmark results against
`georust/netcdf`, see [docs/benchmark-report.md](docs/benchmark-report.md).

## Releasing

See [RELEASING.md](RELEASING.md) for the release checklist and the required
publish order for all six crates.

## Known limitations

- SZIP is not built in (register via `FilterRegistry` if needed)
- ScaleOffset floating-point E-scale mode is not supported by the HDF5 decoder path

## License

MIT OR Apache-2.0
