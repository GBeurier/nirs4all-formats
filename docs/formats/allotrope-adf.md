# Allotrope ADF

> **Status:** Experimental · **Vendor:** Allotrope Foundation · **Extensions:** `.adf` · **Feature flag:** `fmt-hdf5`

ADF (Allotrope Data Format) is the Allotrope Foundation's HDF5-based container
for analytical data cubes plus an RDF semantic description. This reader covers a
narrow data-cube subset and decodes a small, defensive slice of the RDF
triplestore. It is local-fixture only while redistributable and vendor ADF files
are sourced.

## Instruments & software

Produced by Allotrope-aware analytical software and conversion tools. Validation
currently relies on a single local `adfsee` demonstration file; vendor
instrumental ADF exports (Waters, Sciex, Agilent, Bruker, …) are not yet
available.

## File structure

Detected by an `.adf` extension plus the HDF5 magic (`\x89HDF\r\n\x1a\n`); the
ADF structure is validated on read. Decoding uses the pure-Rust `hdf5-reader`
crate (gated behind `fmt-hdf5`) through the shared HDF5 helper, which also routes
external-file references via the sidecar resolver. The reader requires the four
core ADF groups: `/data-cubes`, `/data-description`, `/data-package` and
`/named-graphs`.

For each cube under `/data-cubes`, numeric datasets under `measures` become
records. A 1-D dataset under `scales` whose length matches the primary measure
dimension is used as the x axis; otherwise a generated index axis is used.
Two-dimensional measures are split into one record per secondary column.

The reader additionally decodes the RDF triplestore in
`/data-description/dictionary` (a byte store + 13-byte key rows) and
`/data-description/quads` (a 5-column subject/predicate/object table indexed into
the dictionary), promoting cube titles/labels/descriptions, measure component
types and primary/secondary scale component types when the expected ADF mapping
pattern is present. Unknown mappings fall back to conservative IDs and warnings.

## What nirs4all-formats extracts

- **Signals** — numeric `measures` datasets. The `adfsee` fixture maps
  `AbsorbanceUnitValue` measures to an `absorbance` signal in `mAU`; other
  component types yield an `Unknown`-typed signal named from the measure ID.
- **Axis** — a matching `scales` dataset or a generated index axis.
  `SecondTimeValue` scales become a seconds axis typed `Time`; `NanometerValue`
  scales become an `nm` wavelength axis (used as the secondary axis for split
  2-D records).
- **Metadata** — cube ID, measure ID/shape, axis source/unit/kind, scale IDs,
  ADF component types, axis order, and decoded cube titles/labels/descriptions.
- **Provenance & warnings** — always
  `allotrope_adf_reverse_engineered_data_cube_subset`, plus
  `allotrope_adf_rdf_semantics_partially_mapped` /
  `..._not_resolved` (and a decode-failure warning if the RDF slice cannot be
  read).

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| 1-D numeric data cube | Experimental | One record per measure on its scale or a generated index axis. |
| 2-D data cube | Experimental | Split into one record per secondary column. |
| RDF semantics (titles, component types) | Experimental | Defensive subset of dictionary + quads; conservative fallback otherwise. |
| Vendor instrumental ADF | Planned | Waters/Sciex/Agilent/etc. exports not yet sourced. |

## Limitations & known gaps

- Not a full Allotrope implementation: only the narrow data-cube subset and a
  defensive RDF slice are decoded.
- The full ADF ontology, complete quantity/unit/dimension resolution and SDK /
  reference-reader validation are pending.
- No vendor instrumental ADF exports are available, and the only fixture is
  local-only (governed by Allotrope terms), so this cannot run in CI.
- The JSON Simple Model (ASM) path is separate — see
  [`allotrope-asm`](allotrope-asm.md).

## Reference readers

The Allotrope SDK and the `adfsee` inspection tool; full conformance awaits
redistributable fixtures and ontology resolution.

## Samples & validation

Fixture: `samples_local/allotrope_adf/adfsee_example.adf` (4 records from 3
numeric cubes, including one 2-column measure; generated-index or seconds scale
with a secondary `250/400 nm` axis for the 2-D UV spectrum; `mAU` absorbance).
The file is not committed because the ADF data package and ontologies remain
governed by Allotrope terms even though `adfsee` is an open inspection tool; CI
skips this test when `samples_local/` is absent. The probe reports
`allotrope-adf` at `Confidence::Likely`.
