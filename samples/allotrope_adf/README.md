# Allotrope ADF `.adf` — local-only sample

The Allotrope Data Format (ADF) is a binary HDF5 file paired with an RDF
triplestore. Production ADF files are still pharma/vendor gated, but the
`adfsee` inspection tool ships one demo file that is useful for local
reverse-engineering:

- `samples_local/allotrope_adf/adfsee_example.adf`
- upstream: `allotrope-open-source/adfsee`, `src/main/resources/instances/example.adf`
- sha256: `5bfee7f64016187ad18dcbbf832c776881869f4ea26590930a48e09c1bb55c39`

The fixture is kept out of committed `samples/` because the data package and
ontologies are still governed by Allotrope terms. It can be placed in
`samples_local/` for local validation.

## Current parser scope

The native reader detects `.adf` HDF5 containers and validates the core ADF
groups (`/data-cubes`, `/data-description`, `/data-package`, `/named-graphs`).
It currently extracts numeric `/data-cubes/*/measures/*` payloads and uses a
matching `/scales/*` dataset as the x axis when available.

This is not a complete ADF implementation. RDF dictionary/quads, ontology
semantics, vendor method metadata and SDK conformance are still open work.
