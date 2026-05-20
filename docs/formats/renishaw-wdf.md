# Renishaw WDF

Status: experimental partial.

The Renishaw WDF reader covers the minimal single-spectrum subset of WiRE
`.wdf` files:

- `WDF1` chunk header and block table validation;
- `DATA` float32 ordinate payload;
- `XLST` float32 spectral axis payload;
- `YLST` unit metadata when present;
- fixed-header metadata such as point count, scan type, measurement type,
  accumulation count, application version, laser wavenumber, user and title.

Maps, line scans, depth profiles, time series, StreamLine acquisitions,
white-light images and interrupted acquisitions are recognized by the same WDF
sniffer but refused by the current reader until `WMAP` and `ORGN` navigation
axes are decoded.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/raman_renishaw/renishaw_test_spectrum.wdf` | 1 | wavelength, `nm`, 36 points | RosettaSciIO single-point spectrum |
| `samples/raman_renishaw/wire_sp.wdf` | 1 | wavenumber, `cm-1`, 1015 points | SpectroChemPy real-world single spectrum |

Known negative fixtures include `renishaw_test_undefined.wdf` and
`interrupted_acquisition.wdf`; they fail with a clear single-spectrum subset
message rather than silently flattening map or partial-acquisition data.

## Binary Notes

All observed blocks start with:

```text
name[4]
block_uid: u32le
block_size: u64le
payload...
```

The first `WDF1` block is 512 bytes. The current reader uses these fixed
header offsets:

| Offset | Field |
|---:|---|
| `0x003c` | point count per spectrum |
| `0x0040` | capacity |
| `0x0048` | stored spectrum count |
| `0x0050` | accumulation count |
| `0x0054`, `0x0058` | Y and X sizes |
| `0x0080`, `0x0084` | scan type and measurement type |
| `0x0098`, `0x009c` | spectral unit code and laser wavenumber |

`XLST` payload starts with `data_type: u32le`, `unit: u32le`, followed by
`point_count` float32 axis values. `DATA` stores float32 ordinate values.

## Reference Readers

Planned reference checks:

- `rsciio.renishaw` for broad WDF coverage;
- `py-wdf-reader` / `renishawWiRE` for the single-spectrum path;
- SpectroChemPy `read_wire` for chunk and enum cross-checking.
