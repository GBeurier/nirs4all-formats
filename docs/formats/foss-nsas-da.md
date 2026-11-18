# FOSS NSAS / Rapitec DA

> **Status:** Experimental, observed layout only · **Vendor:** legacy Perstorp /
> Rapitec Beer Analyzer, FOSS NIRSystems technology · **Extension:** `.DA`

The `foss_nsas_da` Rust reader recovers the spectral records of the supplied
NIRSystems 6500 DA variant. This is a different container from modern
[WinISI / ISIscan `.cal` / `.nir`](foss-winisi.md). Detection uses a combination
of binary header fields, with `Likely` confidence, independently of the filename.
Other instruments, point counts and DA variants are not claimed as supported.
Partial records are rejected. Because this layout has no known global record
count, removal of one or more complete records cannot be detected without an
external expected count or source hash.

## Recovered data

- One record per stored spectrum, in physical file order, without averaging,
  deduplicating or preprocessing repeated sample identifiers.
- 1050 little-endian IEEE float32 absorbances, promoted losslessly to float64;
  wavelength axis 400, 402, …, 2498 nm, independently established by the paired
  SPC. The header's 2500 is consistent with an exclusive or nominal upper
  boundary; its precise field semantics remain unverified.
- Sample ID, vendor record number, instrument model, scan count, local date/time
  and three sensor values. The sensor units are unknown. Sensor labels and the
  signal/axis interpretation are independently supported by the paired SPC export.
- Entire 128-byte header and 24-byte trailer as hex, preserving unknown fields;
  per-source SHA-256 and explicit reverse-engineering/timezone warnings.
- No inferred chemical targets. Missing or invalid dates retain their six raw
  words and produce a warning; no UTC timezone is invented.

The reader supports filesystem and pure in-memory access, including builds with
no optional features and the filesystem-free WASM target. Bindings use the same
Rust registry and require no vendor-specific Python/R/WASM decoder.

## Observed byte layout

There is no separate file header in the supplied DA. Its 326400 bytes form
75 complete 4352-byte records. Offsets below are relative to each record;
all numeric fields are little-endian.

| Offset | Width/type | Observation / interpretation |
|---|---|---|
| `0x00` | u16 | 6500; instrument family supported by SPC `MODEL = NIRSYSTEMS 6500`. |
| `0x02` | u16 | Vendor record number; repeats, skips and resets. Preserved, never used as a unique identifier. |
| `0x04` | 10 ASCII bytes | Space-padded sample identifier; SPC `SUBFILE` confirms the first seven. |
| `0x0e` | u16 | 1050 spectral points. |
| `0x10` | u16 | 400, wavelength start. |
| `0x12` | u16 | 2500, consistent with an exclusive or nominal upper boundary; exact semantics unverified. The paired SPC establishes the last stored wavelength as 2498 nm. |
| `0x14`, `0x16` | two u16 | 1 and 2. The latter is consistent with wavelength spacing; broader field semantics unverified. |
| `0x18–0x33` | 28 bytes | Zero in all 75 records; preserved. |
| `0x34–0x3f` | six u16 | 1, 1026, 1, 1, 2, 0; semantics unknown. First five used conservatively in detection. |
| `0x40–0x4b` | six u16 | Day, month, year, hour, minute, second; first seven confirmed by SPC log. |
| `0x4c` | u16 | 32 scans, confirmed by SPC `SCANS` and subheaders. |
| `0x4e` | u16 | 1996 in all supplied records; matches acquisition year, precise purpose unknown. |
| `0x50–0x5b` | three f32 | SPC `TRANSDUCER1`, `TRANSDUCER2`, `TRANSDUCER3` values; units unknown. |
| `0x5c` | f32 | 1.0 throughout; purpose unknown. |
| `0x60–0x7f` | 32 bytes | Zero throughout; preserved. |
| `0x80–0x10e7` | 1050 f32 | Spectral values, identical to corresponding SPC Y payloads. |
| `0x10e8–0x10ff` | 24 bytes | Spaces throughout supplied corpus; preserved rather than treated as targets. |

## Findings on the supplied pair

The DA contains **75 spectra, 15 distinct identifiers and 78750 finite spectral
values**. Repeated IDs mostly have five records; one has four and one has six.
These are stored observations, not proof of a particular replicate policy.
The pair `(sample_id, vendor_record_number)` is also not unique: one identifier
has two distinct acquisitions with the same number. Physical order remains authoritative.
Acquisition timestamps have no encoded timezone.
Absorbance values range from approximately 0.36688733 to 1.65888441.

The companion SPC is Galactic/GRAMS new little-endian (`0x4b`). It independently
declares 1050 points, 400–2498 nm, X type 3 (nm), Y type 2 (absorbance), and stores
75 physical subfiles before its log at byte 317912:

```text
512 + 75 × (32-byte subheader + 1050 × 4-byte Y array) = 317912
```

However, the SPC `fnsub` field at byte 24 declares only **7** subfiles, and its log
describes only the first seven. Reading it conventionally therefore exposes seven
spectra. This inconsistency belongs to the supplied export. The DA reader returns
all 75 records without relying on the SPC. The generic SPC reader retains its
normal declared-count behavior.

Every corresponding DA/SPC Y payload is **byte-identical**, with no fitted scale,
offset, interpolation or tolerance. Full-array conformance compares the DA reader
with the independent MIT `spc-spectra` loader using a temporary SPC copy in which
only `fnsub` is repaired to the independently counted 75 physical blocks. The
original files are never changed. This is validation against a paired export,
not an independent DA reference implementation.
Conformance also reconstructs the entire DA from the canonical values plus the
retained header/trailer bytes and verifies exact equality with the original.

An independent binary review (`docs/reviews/NSAS_DA_2026_11_16.md`) additionally
validated the relationship between DA record numbers and SPC subheader indexes.
It records the unresolved header constants and telemetry observations separately
from established spectral semantics.

## Additional corpus received on 2026-11-18

Two more private DA files use the same observed 4352-byte record layout:

| File | Stored spectra | Distinct sample IDs in that file | Spectral values | Validation |
|---|---:|---:|---:|---|
| `0LC1B44.DA` | 75 | 15 | 78750 | Complete paired-SPC comparison and exact DA reconstruction. |
| `0LC1B70.DA` | 60 | 12 | 63000 | Structural checks, path/bytes parity and exact DA reconstruction. |
| `0LC1B83.DA` | 56 | 11 | 58800 | Structural checks, path/bytes parity and exact DA reconstruction. |

The corpus now contains **191 stored spectra and 200550 spectral values**. No
paired export was supplied for the two new files, so their successful lossless
recovery is not an independent spectral-reference or calibration comparison.
All observed detection constants, wavelength ranges and scan counts remain the
same; this broadens the corpus without qualifying another DA variant.

## Relation to the original calibration workflow

The received manuals now establish the logical role of `.DA`, `.CA`, `.OPR` and
`RAPITEC.RAM`; see the [page-cited workflow analysis](foss-nsas-workflow.md).
The runtime warning distinguishes that documented workflow from the still
unverified calibration-file byte layouts.

NSAS documents transducer inputs as possible regression terms, so the three
stored values must be preserved even though their units and their use by the
missing calibrations are unknown. NSAS also documents optional laboratory
information associated with data files. No constituent targets have been
decoded from these three examples; that must not be generalized to every DA.

The archive supplies manuals and two additional DA files, but no `.CA`, `.OPR`
or `RAPITEC.RAM` binary. There is therefore no new evidence assigning unknown DA
header words to processing codes or reconstructing original coefficients. The
workflow report identifies specific archive names, exports and partial files
that could support the next stage without requiring a complete matching set.

## Validation and licensing

Public CC0 fixtures and their generator live in `samples/foss_nsas_da/`; no
private measurements are copied into them. Focused tests cover sniffing without an
extension, normalized records, complete arrays, path/bytes parity, metadata,
unknown fields, repeated IDs/numbers, invalid dates, corruption, non-finite values
and truncation. A separate ignored Rust test and conditional Python conformance
case exercise the local-only original pair in `samples_local/foss_nsas_da/`.
A second ignored Rust test reconstructs all bytes of all three DA files and
checks 200550 values, sample counts and path/bytes parity.

```bash
cargo test -p nirs4all-formats --test foss_nsas_da
cargo test -p nirs4all-formats --test foss_nsas_da -- --ignored
python -m pytest -m conformance tests/conformance/test_foss_nsas_da.py
```

Original SHA-256 values:

- DA: `6e2b5651aa163e37b6720ffd8301cf1f16dc9303cf3f07f5064552d6242ca235`
- SPC: `d1efcab2d9e3228e83a9386b658030b46d4661dfc8b3acdebeb985015e31fc73`

Additional original DA SHA-256 values:

- `0LC1B70.DA`: `79310867fdc464cc54ddfb4d388f4d8c79a3df15634862c7e3006c04147b2339`
- `0LC1B83.DA`: `269fc5daf406bcafa2cd7842e7db3aefb97da335389335ac7638148de12df6ea`
