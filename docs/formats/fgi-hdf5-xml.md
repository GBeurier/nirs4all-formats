# FGI HDF5 + XML

> **Status:** Experimental · **Vendor:** FGI · **Extensions:** `.xml` (primary), `.h5`, `.hdf5` (payload) · **Feature flag:** `fmt-hdf5`

FGI exports pair an XML metadata file with an HDF5 spectral payload. The XML is
the entry point: it carries the measurement metadata and references its HDF5
data file. This is a narrow, sidecar-bearing reader scoped to the committed
synthetic pairing while a real FGI dataset is sourced.

## Instruments & software

FGI (a niche producer). No redistributable real-world FGI pairing is available
yet, so the current implementation is validated only against a committed
synthetic XML+HDF5 fixture.

## File structure

The reader is dispatched on the `.xml` primary by sniffing for both
`<FGIMeasurement` and `<DataReference` markers (probe confidence `Definite`).
The XML provides:

- root `<FGIMeasurement>`;
- a `<DataReference path="...">` pointing to a sibling `.h5` file;
- `<Metadata>` child elements copied into `metadata.fgi_xml`.

The referenced HDF5 payload is decoded by the generic nested
`spectra` + `wavelengths` mapper (see [`hdf5`](hdf5.md)), so this reader inherits
the `fmt-hdf5` feature gate. Because the HDF5 lives in a separate file, FGI is
resolved through the sidecar resolver: `open_path(xml)` reads both files from
disk, `open_with_sidecars(name, xml_bytes, resolver)` decodes from in-memory
bytes with the resolver serving the referenced HDF5, and plain `open_bytes`
returns `Error::UnsupportedSidecar`. External HDF5 file/link references inside
the data file are routed through the same resolver adapters.

## What nirs4all-io extracts

- **Signals & axis** — from the HDF5 payload, via the generic HDF5 mapper
  (here, an `absorbance` signal on a wavelength axis).
- **Metadata** — XML `<Metadata>` scalars under `metadata.fgi_xml` (e.g.
  `instrument`, `operator`, `date`), the data-reference path, plus the HDF5
  group attributes.
- **Provenance** — the format is rewritten to `fgi-hdf5-xml`, with both the HDF5
  primary payload and the XML metadata sidecar listed in `provenance.sources`.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Synthetic FGI XML + HDF5 pair | Experimental | Maps `<DataReference>` + `<Metadata>` scalars onto the HDF5 mapper. |
| Real FGI XML schema | Planned | Beyond simple `<Metadata>` scalar fields, still unmapped. |
| Multiple HDF5 measurements per XML | Planned | One-to-one assumed; many-to-one not yet specified. |

## Limitations & known gaps

This is not yet a production FGI implementation. Remaining work:

- validate against at least one real FGI HDF5/XML pairing;
- map the real XML schema beyond simple `<Metadata>` scalar fields;
- determine whether one XML sidecar may reference several HDF5 measurements;
- add reference-script comparison of extracted arrays.

## Reference readers

`h5py`, `hdf5r` / `rhdf5` for the HDF5 payload and `lxml` for the XML metadata.
A real pairing is needed before reference comparison is meaningful.

## Samples & validation

Fixture: `samples/fgi/synthetic_fgi.xml` + `.h5` (50 records, 200-point `nm`
axis, `absorbance`; XML `instrument`/`operator`/`date` plus HDF5 group
attributes), covered by golden summaries in
`crates/nirs4all-io/tests/goldens/`. The probe reports `fgi-hdf5-xml` at
`Confidence::Definite`.
