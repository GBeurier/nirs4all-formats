# Allotrope ADF

Status: experimental, local-fixture only.

The Allotrope ADF reader covers a narrow HDF5 data-cube subset. It detects
`.adf` files with HDF5 magic and validates the core ADF groups:

- `/data-cubes`;
- `/data-description`;
- `/data-package`;
- `/named-graphs`.

For each cube under `/data-cubes`, the reader emits numeric datasets under
`measures` as `SpectralRecord`s. A matching 1-D dataset under `scales` is used
as the x axis when its length matches the primary measure dimension; otherwise
a generated index axis is used. Two-dimensional measures are split into one
record per secondary column.

The reader deliberately does not resolve the ADF RDF triplestore yet. Units,
semantic quantity labels and vendor method metadata are therefore preserved only
as conservative IDs and warnings.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples_local/allotrope_adf/adfsee_example.adf` | 4 | generated index or scale dataset, 18001 points | unknown data-cube measure | Local-only `adfsee` demo; 3 numeric cubes, one 2-column measure |

The local fixture is not committed because the ADF data package and ontologies
remain governed by Allotrope terms even though `adfsee` is an open inspection
tool. CI skips this test when `samples_local/` is absent.

## Dispatch Boundaries

This is not a full Allotrope implementation. Missing pieces before a `fait`
status:

- RDF dictionary and quad decoding for quantity names, units and dimensions;
- ADF ontology mapping and SDK/reference-reader validation;
- vendor-specific ADF exports from Waters, Sciex, Agilent or similar systems;
- redistributable fixtures that can run in CI.

The JSON Simple Model path remains separate and is documented in
`formats/allotrope-asm.md`.
