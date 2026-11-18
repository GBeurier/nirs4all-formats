# NSAS binary research, 2026-11-18

This follow-up investigates evidence beyond the supplied Beer Analyzer files and
manuals. It recovers additional binary structure from real DA/CN pairs and adds
an independent DA reference comparison. The supported DA geometries, decoded
metadata and optional CN references are now integrated in the Rust reader; see [runtime contract](foss-nsas-da.md). No CA/OPR/RAM
decoder is established. The initial observations below distinguish corpus-backed
structure from static importer hypotheses.

## Evidence and handling

The [official Metrohm Vision software page](https://www.metrohm.com/en/service/software-center/vision.html)
links a [Vision 4.1.1.394 Single-user installer](https://share.metrohm.com/s/qLTiPaC9cdpSdkG).
Its 955063426-byte ZIP has SHA-256
`0f3e7d9a9cb32e977fdfb8122de2608f7603645f90f6c9b448ebe45db1f37a94`.
Static extraction, including the internal sample archives, recovered **30 DA
files and 28 CN files**. No vendor executable was run. The installer, proprietary
sample files, licenses, extraction manifest, hashes and analysis outputs remain
under the ignored local research corpus; they are not redistributed.

Public evidence independently establishes the companion's purpose:

- [US6549861B1, data-format discussion](https://patents.google.com/patent/US6549861B1/en)
  explicitly identifies same-basename `.DA` spectra and `.CN` constituent values.
  Its subsequent ASCII-format description concerns a different format and must
  not be mistaken for the NSAS binary layout.
- [Infometrix Pirouette 5 manual](https://infometrix.com/wp-content/uploads/2023/04/pirouette.pdf),
  PDF page 422, describes NSAS binary DA input and optional CN dependent variables.
- [Metrohm Vision 4.1 manual](https://www.metrohm.com/content/dam/metrohm/shared/documents/manuals/81/81058050EN.pdf),
  section 6.7, printed pages 149–150, documents importing NSAS calibration equations.
  This confirms an existing implementation, rather than publishing its byte layout.

## DA record alignment

All 30 extracted DA files partition exactly with the candidate rule

```text
N = little-endian u16 at record offset 14
record_bytes = ceil((128 + 4*N) / 128) * 128
```

Each record retains its 128-byte header, N little-endian float32 values and
remaining padding. Geometry was checked at every record boundary. The same rule
also explains all three Beer Analyzer files. It accounts for the trailer length
without assigning chemical meaning to padding bytes.

Static analysis of Vision's DA helper independently confirms the 128-byte
alignment calculation and `(end-start)/N` axis spacing for these point counts.
It also reveals an exception for N = 3101, using `(end-start)/(N-1)`. No 3101-point
sample was recovered; the usual exclusive-end interpretation must therefore not
be generalized to every NSAS variant.

| Observed files | N | Stored start/end | Inferred spacing | Record bytes | Padding bytes | Records |
|---:|---:|---|---:|---:|---:|---:|
| 27 vendor files | 700 | 1100 / 2500 | 2 | 2944 | 16 | 354 |
| `RSS10308.da` | 4200 | 400 / 2500 | 0.5 | 17024 | 96 | 9 |
| `acdisol_mlt.da` | 1700 | 800 / 1650 | 0.5 | 7040 | 112 | 18 |
| `TSSZERO.da` | 1050 | 400 / 2500 | 2 | 4352 | 24 | 1 |
| 3 Beer Analyzer files | 1050 | 400 / 2500 | 2 | 4352 | 24 | 191 |

For the vendor corpus, **382 records / 317250 finite spectral values** were
extracted. Repacking every float32 payload and preserving all headers and padding
reproduced all 30 source files byte-for-byte. This is a structural recovery,
without an independent vendor export for their complete spectral semantics.
Instrument words include 0, 5000 and 6500, sometimes within a single file; math
and acquisition words also vary. The registered reader now accepts these four observed geometries with bounded
constituent counts and acquisition-code checks. Only the original Beer Analyzer
signature is labelled absorbance; other signals retain unknown physical semantics.

## CN reference-value blocks

For **every one of the 28 matched DA/CN pairs**:

```text
CN file size = 384 + 128 * number_of_DA_records
row i starts at 384 + 128*i
each row can be decoded as 32 little-endian IEEE float32 slots
```

The matched files contain **372 rows / 11904 float32 slots**. All values are
finite. Every CN was reconstructed exactly from its preserved 384-byte header
and repacked rows. Eleven files contain a readable first header label and
nonzero values in the first slot; all their other slots are zero:

| CN files | First label | Rows | Observed first-slot range |
|---|---|---:|---|
| `Avicl20`, `Avicl50`, `Avicl100`, `Avicl180` | `20` | 24 total | Constant 20, 50, 100 or 180, respectively. |
| `Lact110`, `Lact80`, `Lactfast`, `Lactger` | `WATER` | 22 total | Approximately 4.8–5.6. |
| `POLYCL` | `oh#` | 28 | Approximately 44.88–97.71. |
| `POLYVL` | `Hydroxyl #` | 7 | Approximately 44.94–84.77. |
| `slurry` | `Oil` | 99 | Approximately 0.31–6.458. |

For example, `Lactfast.cn` is 896 bytes: a 384-byte header beginning with `WATER`,
then four 128-byte rows. Its first float32 values are approximately 5.0, 5.4,
5.1 and 5.2. `Lactfast.da` contains exactly four 2944-byte spectra. The first
sample's candidate reference value is thus at byte 384, the next at 512, then
640 and 768; the remaining 31 slots of each row are zero.

The other 17 CN files have blank labels and zero-valued rows. Their existence
demonstrates why finding a CN file alone does not prove that useful laboratory
values are present. Zero versus missing-value semantics and measurement units
are not established by these examples.

The numerical rows contain no observed sample IDs. Pairing by basename and
physical ordinal is consistent with all counts and the documented companion
relationship, but has not been checked against a vendor application's display.
Static analysis resolves a field width that the examples alone cannot establish:
Vision reads 384 header bytes, then copies each name from `header + 10*i` with a
length of 10. For each subsequent 128-byte row, it reads float32 values from
`row + 4*i`. Thus names have **10-byte slots**, not the superficially plausible
12-byte slots obtained by dividing 384 by 32. Up to 32 float32 positions fit in
each observed row; remaining header bytes are not interpreted. The examples do
not validate multiple active constituents or a missing-value convention. The integrated CN reader is scoped to this fixed layout; unknown encodings,
missing-value conventions and other CN variants are not claimed.

The little-endian DA word at offset 30 is
1 throughout the 11 named/nonzero CN pairs and 0 throughout the 17 blank/zero
pairs. It is 0 in all three Beer Analyzer files. Static data flow confirms that
Vision reads this word from the 128-byte DA header, passes it back to the caller
and uses it as the bound of the CN name/value loops. It is therefore the
**constituent count consumed by this importer**, beyond a mere presence
correlation. Zero does not prove that references never existed for the Beer
Analyzer measurements or that a missing companion cannot contain useful material.

The local disassembly records these paths: DA helper `0xc79c00`, count load at
`0xc7a1f7`, caller at `0x8826e0` and CN loop at `0x885c6a` with name copies at
`0x885c94–0x885cac`. These are virtual addresses in the extracted Vision image,
not offsets within a DA/CN file.

## Candidate CA structure from the importer

This part uses **static program analysis, without any real CA sample**. The
extracted `VISION.exe` is PE32/i386, 20824576 bytes, SHA-256
`48fd9c4a8d1467785047a0751009621373e803266ed27976d4af721b0cfad138`,
preferred image base `0x400000`. The CA file filter leads to function `0xe17d60`.
The common read-like thunk is `0xf2d744`: its buffer/length arguments and returned
byte counts support that interpretation, but its imported MFC ordinal was not
resolved to a symbol name. Local evidence includes the full targeted
disassemblies, hashes and VA-to-file-offset mappings.

The following **file offsets are inferred** from successive full-length reads
after opening, without a hidden seek or other file mutation. They are useful
targets for a first real CA example, not a normative schema.

| Candidate offset | Read/field | Static observation |
|---:|---|---|
| 0 | 128 bytes | First header-like block, read at VA `0xe18410`. Words at 16 and 18 feed range-like parameters. |
| 128 | 256 bytes | Second block, read at `0xe184b9`; code adds a NUL at in-memory byte 160, without proving that all 256 bytes are text or that file byte 288 must be zero. |
| 384 | 128 bytes | Third block, read at `0xe18575`. |
| 394 | signed little-endian i16, `k` | Loaded with `MOVSX` at `0xe1860c`; signed comparison with 8 selects the model path. |
| 512 | MLR: 128 bytes | First parameter block, read at `0xe18986`; first scalar is an intercept candidate, followed by coefficient candidates. |
| 640 | MLR: 128 bytes | Position/term block, read at `0xe18a2f`; signed i16 pairs are used for up to `k` terms. |

For `k <= 8`, the constructor and MSVC RTTI identify `CNSASMLRfunction`; for
`k > 8`, they identify `CPLS1function`. `k` also bounds coefficient/position loops.
It must **not** be relabelled as a PLS latent-factor count. A negative `k` also
takes the MLR branch in this program; that is importer control flow, not evidence
that a valid calibration may have a negative term count.

In the MLR branch, the five sequential reads consume **768 bytes**. The first
32-bit scalar at 512 is copied to an internal parameter; words at `516+4*i`
are copied to a coefficient-like array. The signed pairs at `640+4*i` and
`642+4*i` feed coordinate-like parameters. The second member has special handling
for -1 and 0; other values activate a separate term kind. A first coordinate
at or beyond the end-like header word is transformed using division by 2.
These are concrete encoding/flow observations. Intercept, ratio and transducer
interpretations remain candidates until an evaluator or real calibration report
establishes their meaning. Neither an exact CA file length nor absence of trailing
records follows from the 768-byte consumption.

The PLS branch reads a larger first parameter block, alignment padding, then a
position-like block. With `align128(x) = 128*ceil(x/128)`, the observed consumption is

```text
512 + align128(4*(max(k,31)+1)) + 4*max(k,32)
```

For `k` from 9 through 31 this also consumes 768 bytes; at 32 it consumes 896.
The placement loop converts signed positions to an axis index and places
coefficient-like values into a generated spectrum, using a fixed 2.0 step in this
path. No training references, loading/weight matrices or model reconstruction
were demonstrated. Version coverage and malformed-input behavior remain unknown.

This is substantial progress from knowing only the file extensions, but it is
still an importer-derived **candidate map**. No CA parser, recovered Beer
Analyzer equation or prediction equivalence is claimed.

## Transducer scaling observation

Vision's DA helper divides the float32 values at header offsets 80, 84 and 88
by 2 before copying them to an internal object (`0xc79f95–0xc79fd8`). This is
an observed import step, without an end-to-end UI or prediction trace. It does
not identify units or establish the Rapitec convention. The supplied SPC log
matches the raw DA numbers, so the Formats reader continues to preserve them
without applying the Vision-specific conversion.

## Independent DA reader comparison

The GPL-3-or-later
[aquap2 reader](https://github.com/bpollner/aquap2/blob/57ee096fbecf28a321eed5884784e01b0bca036b/R/prep_importData_specific.r)
contains a standalone `getNIRData_Vision_da` implementation. The pinned source's
SHA-256 is `2d96e735d82a2753f63126ebbb8ff13092f1e2c50970783189b322b7a76222b4`.
It was invoked in an isolated R 4.1.2 subprocess, without importing or linking
GPL code into the Rust/Python runtime.

The comparison recovered **all 191 records and 200550 values** from the three
Beer Analyzer files. Every promoted float64 value, every wavelength coordinate
and all 191 local-clock strings matched exactly, without tolerance or fitting.
A fixed UTC display zone was used only to preserve the stored six clock fields;
the original acquisition timezone remains unknown. aquap2 alters repeated IDs
with `make.unique()` and omits stored record numbers, so those are not validated
against that implementation.

This extends independent spectral-reference coverage to the two files that have
no paired SPC. It does not independently establish transducer meanings, hidden
header fields, concentration values or any fitted calibration.

The pinned aquap2 implementation chooses fixed trailers of 24 bytes for 2 nm
and 96 bytes for 0.5 nm. It therefore cannot serve as a general reference for
the 700- and 1700-point vendor examples, which require 16 and 112 bytes. It was
not modified to force agreement.

`tests/conformance/test_foss_nsas_da_aquap2.py` reproduces the comparison for the
public synthetic fixture and the three conditional private files. Supply the
pinned source using `NIRS4ALL_AQUAP2_NSAS_SOURCE` and make `Rscript` available.
Tests check the source hash before executing it; they do not download or vendor
the reference. Missing optional prerequisites produce explicit skips.

## Remaining calibration research

The installer and its nested sample archives contain no `.CA`, `.OPR` or
`RAPITEC.RAM` example. Exact-name searches, public code searches and archive
searches also produced no qualified sample of those three formats. This is a
bounded search result, not evidence that no public copy exists.

Static strings in `VISION.exe` include the UTF-16 file filter
`NSAS Equation Files (*.CA)` and the RTTI class name `CNSASMLRfunction`.
They locate a real import path for further static analysis. A string or class
name alone does not identify offsets, numeric layouts or recovered coefficients.
The mathematical DLL's NSAS derivative entry points similarly identify an
implementation without supplying its input-file schema.

The most useful new request to the contributor is a same-basename `.CN` for any
Beer Analyzer DA, even without a CA/OPR/RAM set. One simple `.CA` plus its NSAS
equation display remains the next direct calibration-format test case.
