# AnIML XML

> **Status:** Experimental · **Vendor:** IUPAC / ASTM · **Extensions:** `.animl`

AnIML (Analytical Information Markup Language) is a broad XML container for
analytical data, not only spectroscopy. This reader covers AnIML documents that
expose a spectral `SeriesSet`: an axis series identified as wavelength or
wavenumber, plus one or more same-length signal series. Coverage is scoped to
committed synthetic fixtures while real spectral AnIML files are sourced.

## Instruments & software

Vendor-neutral analytical instruments and software that emit AnIML. No real
redistributable spectral AnIML file is available yet, so committed spectral
fixtures are synthetic; a committed real-world AnIML file is non-spectral and is
deliberately refused.

## File structure

XML, dispatched on an `.animl` extension by sniffing for `<AnIML` or `:AnIML`
(probe confidence `Definite`). No feature flag and no companion files: AnIML
decodes in-memory through `open_bytes`. The reader streams the XML, locating the
axis series inside a `SeriesSet` and reading the signal series alongside it.

## What nirs4all-formats extracts

- **Signals** — every non-axis series whose length matches the axis, named from
  the series name with the signal type inferred from that name. When several
  signals are present, the record's dominant signal type is chosen by priority.
- **Axis** — the series identified as wavelength or wavenumber, with the kind and
  unit derived from its ID/name/unit (defaulting to `nm` for wavelength,
  `cm-1` for wavenumber).
- **Targets** — numeric `Parameter` values inside `SampleSet`.
- **Metadata** — `sample_id` and `sample_name` from `Sample` attributes.
- **Provenance** — source file + SHA-256, reader name and version.

Supported value blocks are explicit `<F>` and `<D>` numeric values plus uniform
`AutoIncrementedValueSet` grids defined by `StartValue` and `Increment`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Spectral `SeriesSet` with explicit values | Experimental | Axis + same-length signal series. |
| `AutoIncrementedValueSet` axis grid | Experimental | Uniform `StartValue` + `Increment`. |
| `SampleSet` numeric parameters | Experimental | Emitted as record targets. |
| Real vendor spectral AnIML | Planned | Synthetic fixtures only; real files + XSD conformance wanted. |

## Limitations & known gaps

- AnIML is a broad analytical container; documents without a supported spectral
  axis series are refused (the committed non-spectral NMR/DLS example is a
  locked refusal).
- Remaining gaps: real spectral AnIML fixtures, non-zero segmented value-set
  indices, schema/XSD validation and broader vendor-export conformance.

## Reference readers

`animl-python` and generic XML validators read the same documents; reference
comparison awaits real spectral fixtures.

## Samples & validation

Fixtures under `samples/animl/`: `synthetic_nirs.animl` and
`synthetic_nirs_autoincrement.animl` (each 1 record, `nm` axis, `absorbance`,
`protein` target; the second uses an `AutoIncrementedValueSet` axis).
`Example3.animl` is a real but non-spectral AnIML file kept as a locked refusal.
The probe reports `animl` at `Confidence::Definite`.
