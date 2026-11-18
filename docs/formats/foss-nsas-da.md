# FOSS NSAS / Rapitec DA and CN

> **Status:** Experimental, observed layouts · **Vendor:** FOSS NIRSystems /
> Perstorp / Rapitec · **Extensions:** `.DA` spectra, optional `.CN` references

The `foss_nsas_da` Rust reader decodes legacy NSAS spectral records and optional
same-basename constituent/reference files. It is separate from modern
[WinISI / ISIscan `.cal` / `.nir`](foss-winisi.md). Detection combines known binary
geometry, instrument words, sample identifiers and acquisition fields; it has
`Likely` confidence and does not depend on the extension.

## Supported geometries and signal semantics

| Points | Header start / end | Stored axis, nm | Record bytes | Padding |
|---:|---|---|---:|---:|
| 700 | 1100 / 2500 | 1100–2498, step 2 | 2944 | 16 |
| 1050 | 400 / 2500 | 400–2498, step 2 | 4352 | 24 |
| 1700 | 800 / 1650 | 800–1649.5, step 0.5 | 7040 | 112 |
| 4200 | 400 / 2500 | 400–2499.5, step 0.5 | 17024 | 96 |

The original Beer Analyzer signature retains the `absorbance` signal and
`Absorbance` type, established by the paired SPC export. Other accepted headers
emit a `spectrum` signal with `Unknown` type, no inferred signal unit and an
explicit semantics warning. Neither an unfamiliar instrument word nor a math
code is converted into a guessed physical signal type. Known instrument words
are 0, 5000 and 6500; zero is retained numerically without inventing a model name.
Models and acquisition settings may vary between records. Geometry and the
number of constituents must remain constant within a file.

Other point counts, wavelength ranges and acquisition codes remain unsupported.
Vision has a special axis rule for 3101 points, but no such sample was recovered;
this reader deliberately refuses that unqualified geometry. Accepted sizes use
`(end-start)/N`, with the end excluded. See the
[binary research](foss-nsas-binary-research.md) for evidence and limits.

## DA record layout

There is no separate global header in the observed files. Each record consists
of a 128-byte header, N little-endian IEEE float32 values and alignment padding:

```text
record_bytes = ceil((128 + 4*N) / 128) * 128
```

Offsets are relative to the start of each record; numeric fields are little-endian.

| Offset | Width/type | Interpretation |
|---|---|---|
| `0x00` | u16 | Instrument word; 0, 5000 or 6500 in the corpus. |
| `0x02` | u16 | Vendor record number. Repeats, skips and resets; never a unique key. |
| `0x04` | 10 bytes | ASCII sample ID, padded by spaces or terminal NULs. |
| `0x0e` | u16 | Number of spectral points. |
| `0x10`, `0x12` | two u16 | Wavelength start and upper boundary for the supported geometries. |
| `0x14`, `0x16` | two u16 | Preserved acquisition/math words. The latter is not a general wavelength step. |
| `0x18–0x1d` | 6 bytes | Opaque, preserved. |
| `0x1e` | u16 | Number of constituents used by the CN importer; supported range 0–32. |
| `0x20–0x33` | 20 bytes | Opaque, preserved. |
| `0x34–0x3d` | five u16 | Acquisition words; observed combinations constrain detection, without invented enum labels. |
| `0x3e–0x3f` | u16 | Opaque acquisition word, preserved without constraining detection. |
| `0x40–0x4b` | six u16 | Local day, month, year, hour, minute, second. |
| `0x4c` | u16 | Scan count. |
| `0x4e` | u16 | Additional year-like word; precise purpose unknown. |
| `0x50–0x5b` | three f32 | Raw transducer fields. Original Beer Analyzer labels are `TRANSDUCER1–3`; units unknown. |
| `0x5c–0x7f` | 36 bytes | Opaque, preserved. |
| `0x80` onward | N f32 | Stored spectral values, promoted losslessly to f64. |
| after payload | alignment padding | Preserved byte-for-byte, without interpreting it as references. |

Records stay in physical order, without averaging, deduplication or preprocessing.
The reader preserves `da_header_hex`, `da_trailer_hex`, all six date/time words,
raw instrument code, stored record number, scans, constituent count and geometry.
Valid acquisition clocks become naive local strings; invalid clocks retain the
raw words and a warning. No timezone is invented.

Non-finite spectra are rejected. The original Beer Analyzer signature also
rejects non-finite sensors. Some vendor records contain non-finite sensor
fields: those metadata entries become null with a warning, while the full
header retains their exact bits. No automatic Vision-specific sensor division
or calibration correction is applied.

## Optional CN references

Filesystem reads automatically locate a same-basename CN, with DOS-style case
matching of the actual filename. In-memory and WASM callers supply the companion
through the existing sidecar API. `sidecar_requirements()` declares its optional
role and names, so callers do not need a separate vendor parser.

```python
from nirs4all_formats import open_records, open_with_sidecars

records = open_records("references700.da")  # discovers references700.cn next to it
records = open_with_sidecars("references700.da", da_bytes, {"references700.cn": cn_bytes})
```

Pure `open_bytes` never reads the filesystem, even when given a real path. Missing
CN files do not prevent spectral recovery. A positive constituent count without
CN produces an explicit missing-reference warning and empty targets. A supplied
but malformed/ambiguous CN is an error, rather than a silent spectra-only fallback.
No CA, OPR or RAM file is required to decode these spectra and references.

The observed CN layout is:

| Region | Layout |
|---|---|
| Header | 384 bytes; active constituent names occupy 10-byte slots starting at zero. |
| Data | One 128-byte row per DA record, containing 32 float32 slots. |
| Active values | The first `DA.constituent_count` slots, joined by physical ordinal. |

Only active slots become canonical `targets`. **Zero is a valid value**; no
missing-value sentinel or concentration unit is guessed. Non-finite active values
are rejected. Inactive slots are opaque and may contain arbitrary bits. A zero
constituent count never produces targets, even if a supplied CN contains nonzero
bytes. Counts above 32, changing counts, partial rows and row-count mismatches
are rejected.

ASCII names become normalized target keys, such as `water` and `oil`. Blank or
undecoded names receive indexed fallback keys; duplicate/normalization collisions
receive deterministic suffixes without overwriting any value. `cn_constituents`
records each original slot index, decoded name when available, target key and raw
name bytes. No OEM codepage is inferred for non-ASCII names. `cn_header_hex` and
`cn_record_hex` preserve the complete CN, including unused slots. Provenance
includes both DA and CN paths and SHA-256 hashes.

The real vendor examples validate zero or one active constituent. Multi-slot
support follows the inspected importer and is tested with invented data,
including 32 slots; it is not presented as a multi-target vendor reference
comparison. No sample ID is stored in the observed CN rows, so equal row counts
cannot prove that a reordered or unrelated companion belongs to a given DA.

## Validation and corpus

The first supplied DA contains **75 spectra, 15 distinct IDs and 78750 values**.
Its paired Galactic SPC independently establishes the 400–2498 nm wavelength
axis, absorbance interpretation, scans and first seven transducer/time mappings.
All 75 corresponding Y payloads are byte-identical. The SPC incorrectly declares
only 7 subfiles although 75 physical blocks precede its log at byte 317912:

```text
512 + 75 * (32 + 1050 * 4) = 317912
```

Only a temporary reference copy's declared count is repaired for conformance;
original files are never modified. Neither the generic SPC reader's behavior
nor the physical acquisition order is changed.

| Private Beer Analyzer file | Records | Values |
|---|---:|---:|
| `0LC1B44.DA` | 75 | 78750 |
| `0LC1B70.DA` | 60 | 63000 |
| `0LC1B83.DA` | 56 | 58800 |

All **191 spectra / 200550 values**, axes and local timestamps exactly match the
independent pinned aquap2 DA implementation in an isolated GPL reference
subprocess. Every DA reconstructs exactly. Their constituent count is zero;
no matching Beer Analyzer CN or original calibration equation was supplied.

An official Vision distribution supplied another **30 DA files / 382 spectra /
317250 values** and **28 CN files / 372 rows**. All are readable and reconstruct
exactly. Eleven CN files expose **180 active reference values**; the remaining
17 have zero declared constituents. These proprietary artifacts remain local.

Public CC0 fixtures cover the original DA/SPC pair, 700-point DA/CN with two
invented targets, and 1700/4200-point records. Acceptance tests cover the registry,
path/bytes/sidecar parity, metadata and hashes, exact reconstruction, corrupt
headers, truncation, ambiguous companions, duplicate names and invalid values.
Python, R and WASM bindings call the same Rust implementation.

See [calibration workflow](foss-nsas-workflow.md) and
[binary research](foss-nsas-binary-research.md). The CA block map remains an
unvalidated research result; no CA/OPR/RAM decoder, fitted coefficients or prediction
engine is implemented.
