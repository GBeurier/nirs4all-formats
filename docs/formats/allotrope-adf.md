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

The reader now decodes a small, defensive subset of the ADF RDF triplestore in
`/data-description/dictionary` and `/data-description/quads`. When the observed
ADF mapping pattern is present, it promotes cube titles/labels, measure
component types, primary scale component types and secondary scale component
types into metadata. The local `adfsee` fixture maps:

- `AbsorbanceUnitValue` measures to an absorbance signal with `mAU` units;
- `SecondTimeValue` scales to a seconds axis, still typed as `index` because it
  is a time axis rather than a spectral wavelength/wavenumber axis;
- `NanometerValue` secondary scales to `nm` wavelength metadata for split 2-D
  records.

Unknown mappings fall back to conservative IDs and warnings. This is not an
ontology-complete resolver.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Notes |
|---|---:|---|---|---|
| `samples_local/allotrope_adf/adfsee_example.adf` | 4 | generated index or seconds scale, 18001 points; secondary `250/400 nm` for the 2-D UV spectrum | unknown double array or absorbance `mAU` | Local-only `adfsee` demo; 3 numeric cubes, one 2-column measure |

The local fixture is not committed because the ADF data package and ontologies
remain governed by Allotrope terms even though `adfsee` is an open inspection
tool. CI skips this test when `samples_local/` is absent.

## Dispatch Boundaries

This is not a full Allotrope implementation. Missing pieces before a `fait`
status:

- RDF dictionary and quad decoding for quantity names, units and dimensions;
- full ADF ontology mapping beyond the narrow component types above;
- SDK/reference-reader validation;
- vendor-specific ADF exports from Waters, Sciex, Agilent or similar systems;
- redistributable fixtures that can run in CI.

The JSON Simple Model path remains separate and is documented in
`formats/allotrope-asm.md`.
