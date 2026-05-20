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

## Missing Behavior

This is not yet a production FGI implementation. Remaining work:

- validate against at least one real FGI HDF5/XML pair;
- map the real XML schema beyond simple `<Metadata>` scalar fields;
- document whether multiple HDF5 measurements can be referenced by one XML
  sidecar;
- compare extracted arrays against `h5py`/`lxml` reference scripts.
