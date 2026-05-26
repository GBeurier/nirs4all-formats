# Allotrope ASM JSON

> **Status:** Supported (scoped) · **Vendor:** Allotrope Foundation / Benchling · **Extensions:** `.json`

ASM (Allotrope Simple Model) is the JSON expression of Allotrope analytical
documents — a lighter alternative to the HDF5-based ADF. This reader covers the
plate-reader ASM path: spectral data cubes and single-wavelength endpoint
readings. It does not attempt full ASM schema coverage.

## Instruments & software

Plate-reader software and conversion tools that emit ASM, with Benchling's
`allotropy` converter as the canonical producer. Committed fixtures come from
Benchling spectral/endpoint examples; broader vendor conversions are still
wanted.

## File structure

Plain JSON, dispatched on a `.json` extension by sniffing for both
`"$asm.manifest"` and `"plate reader aggregate document"` (probe confidence
`Definite`). No feature flag and no companion files: ASM decodes in-memory
through `open_bytes`. The reader walks the plate reader aggregate document, its
plate documents and their measurement documents, emitting one record per
`measurement document`.

## What nirs4all-io extracts

- **Signals** — for cube measurements, the measure series under
  `/data/measures/0`; for endpoint measurements, a single value at the detector
  wavelength. The signal type is inferred from the measure concept / cube label
  (`absorbance`, `fluorescence`, …).
- **Axis** — for cubes, the dimension series under `/data/dimensions/0` with the
  declared unit (default `nm`) and a kind from the dimension concept; for
  endpoints, a single-point wavelength axis.
- **Metadata** — sample identifier, well/location, detection type, measurement
  time, manifest URL, converter metadata, the ASM cube/endpoint key and selected
  device-control settings.
- **Provenance & warnings** — source file + SHA-256; when a cube declares an
  `absorbance` measure concept but its key/label identify fluorescence emission,
  the original concept is kept in `metadata.asm_measure_concept`, a warning is
  emitted and the signal is named `fluorescence`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Plate-reader spectral data cube | Supported | One record per measurement document; wavelength axis + signal. |
| Single-wavelength endpoint reading | Supported | One-point wavelength axis. |
| Absorbance-concept fluorescence emission | Supported | Relabelled to `fluorescence` with a provenance warning. |
| Non-plate-reader / general ASM | Out of scope | Only plate-reader spectral documents are mapped. |

## Limitations & known gaps

- A narrow ASM bridge: it does not attempt full ASM schema coverage, ADF/HDF5
  parsing (see [`allotrope-adf`](allotrope-adf.md)) or vendor conversion.
- Coverage is dominated by Benchling examples; diverse industrial conversions
  (and spectral cases beyond plate readers) are still needed to broaden it.

## Reference readers

When original vendor files must be converted to ASM first, the reference tool is
`Benchling-Open-Source/allotropy`.

## Samples & validation

Fixtures under `samples/allotrope_asm/`:
`ACSINS_absorbance_spectrum.json` (360 records, 51-point `nm` axis, `absorbance`
`mAU`), `spectrum_emission_data.json` (1 record, 3-point axis, `fluorescence`
`mAU`) and `MD_SMP_absorbance_example.json` (192 single-point absorbance
records). The probe reports `allotrope-asm-json` at `Confidence::Definite`.
