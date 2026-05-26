# mzML

> **Status:** Detected / refused · **Vendor:** HUPO PSI / MS vendors · **Extensions:** `.mzML`, `.mzMLb`

mzML is the HUPO PSI XML standard for mass-spectrometry data. It is recognised by
nirs4all-io for disambiguation but is **not decoded into records**: it is not a
NIRS / optical-spectroscopy format, and its primary X axis is `m/z`, which is
outside the current `SpectralAxis` model.

## Instruments & software

Produced and consumed across the mass-spectrometry ecosystem (Thermo, Bruker,
SCIEX, Agilent and others) via the ProteoWizard / `msconvert` toolchain. mzML is a
useful design reference for nirs4all-io's data model, but its scope is MS, not
optical spectroscopy.

## File structure

XML containers (`.mzML`, optionally `indexedmzML`-wrapped) holding `<spectrum>`
and `<chromatogram>` elements whose binary data arrays carry `m/z`, intensity and
time vectors, frequently zlib-compressed and/or MS-Numpress encoded. `.mzMLb` is
the HDF5-backed binary variant.

## Why it is refused / where to go instead

The native registry sniffs XML `.mzML` / `.mzMLb` text containers (extension plus
a `<mzML` or `<indexedmzML` marker) at `Confidence::Definite`, then refuses on
read. The error counts the detected `<spectrum>` and `<chromatogram>` elements and
directs users to a dedicated MS toolkit:

- **`pyteomics`** — pure-Python mzML reader/iterator.
- **`pymzML`** — streaming mzML access with spectrum/chromatogram objects.
- **`pyOpenMS`** — full OpenMS bindings for MS data.

The reader does **not** decode spectra, chromatograms, zlib arrays or MS-Numpress
payloads. If mzML support is ever needed it should be introduced as an explicit
adjacent MS model or a separate adapter, not as a silent optical-spectrum import
path that coerces `m/z` arrays into NIRS `SpectralRecord`s.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `.mzML` (XML) | Detected / refused | Refused with a pointer to `pyteomics` / `pymzML` / `pyOpenMS`. |
| `indexedmzML` wrapper | Detected / refused | Same path; recognised by the `<indexedmzML` marker. |
| `.mzMLb` (HDF5-backed) | Documented only | Listed in scope; not separately exercised. |

## Limitations & known gaps

- No decoding of any kind — this is a deliberate refusal, not a partial reader.
- `.mzMLb` is documented but not separately covered by a fixture.

## Reference readers

`pyteomics`, `pymzML` and `pyOpenMS` are the recommended tools and the references
named in the refusal message.

## Samples & validation

Fixtures live under `samples/mzml/` and assert the refusal behaviour:

| Fixture | Behaviour | Notes |
|---|---|---|
| `samples/mzml/example.mzML` | refused | MS1 spectrum collection |
| `samples/mzml/mini.chrom.mzML` | refused | Chromatogram example |
| `samples/mzml/mini_numpress.chrom.mzML` | refused | Chromatogram with MS-Numpress compression |
