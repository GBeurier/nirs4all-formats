# AnIML (Analytical Information Markup Language) `.animl`

IUPAC + ASTM XML standard for analytical data. Schema is open; production-grade Python tooling is limited (early-stage `animl-python`).

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `Example3.animl` | 4.6 KB | [`KE-UniLiv/animl-ontology@main/examples/example_AnIML_files/Example3.animl`](https://github.com/KE-UniLiv/animl-ontology/blob/main/examples/example_AnIML_files/Example3.animl) | (academic / AnIML.org "Example3" series) | The canonical "Example3" AnIML.org sample mirrored by the AnIML ontology project. |
| `synthetic_nirs.animl` | ~3 KB | Generated locally | CC-0 | Synthetic single-spectrum AnIML with one Sample, one ExperimentStep, paired wavelength/absorbance SeriesSet. Good for structural unit tests. |

## Parser hints

- AnIML uses the namespace `urn:org:astm:animl:schema:core:draft:0.90` (or later draft revision).
- Top-level structure: `<AnIML>` → `<SampleSet>` + `<ExperimentStepSet>` (each ExperimentStep contains `<Result>` blocks with `<SeriesSet>` data).
- Series data can be `<IndividualValueSet>` (explicit values) or `<AutoIncrementedValueSet>` (uniform grid).
- Reference readers: `animl-python` (early), generic `lxml.etree` parsing.
- Schema validators are useful for sanity-checking input.
