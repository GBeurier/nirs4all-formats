# FGI HDF5 + XML

Status: experimental partial.

FGI exports are modelled as an HDF5 payload plus an XML metadata sidecar. The
current reader handles the committed synthetic pairing:

- XML root `<FGIMeasurement>`;
- `<DataReference path="...">` pointing to a sibling `.h5` file;
- `<Metadata>` child elements copied into `metadata.fgi_xml`;
- HDF5 payload decoded by the generic nested `spectra` + `wavelengths` mapper.

The emitted provenance format is `fgi-hdf5-xml`, with both the HDF5 primary
payload and XML sidecar listed in `provenance.sources`.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Metadata |
|---|---:|---|---|---|
| `samples/fgi/synthetic_fgi.xml` + `.h5` | 50 | wavelength, `nm`, 200 points | `absorbance` | XML `instrument`, `operator`, `date`; HDF5 group attributes |

## Sidecar contract (M1, 2026-05-22)

FGI is a sidecar-bearing format: the XML metadata file references its
HDF5 payload via `<DataReference path="...">`. Three entry points cover
decoding:

- `open_path(xml_path)` reads the XML plus the HDF5 sidecar from disk.
- `open_with_sidecars(name, xml_bytes, Arc<dyn SidecarResolver>)`
  decodes from in-memory bytes; pass the XML as the primary and have
  the resolver serve the HDF5 file referenced in `<DataReference>`.
- `open_bytes(name, xml_bytes)` returns `Error::UnsupportedSidecar`.

External HDF5 links inside the data file are routed through the same
resolver (M1 adds `Arc<dyn ExternalFileResolver>` /
`Arc<dyn ExternalLinkResolver>` adapters via
`crates/nirs4all-io/src/readers/hdf5_helpers.rs`).

## Missing Behavior

This is not yet a production FGI implementation. Remaining work:

- validate against at least one real FGI HDF5/XML pair;
- map the real XML schema beyond simple `<Metadata>` scalar fields;
- document whether multiple HDF5 measurements can be referenced by one
  XML sidecar;
- compare extracted arrays against `h5py`/`lxml` reference scripts.
