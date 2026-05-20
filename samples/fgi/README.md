# FGI HDF5 + XML

Standards-based container (HDF5 for arrays, XML for metadata), but the schema is FGI-specific → needs a dedicated mapper.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_fgi.h5` + `synthetic_fgi.xml` | Generated locally | CC-0 | Mock pairing — HDF5 group `Measurement1` with `spectra` and `wavelengths` datasets + an `instrument`/`operator`/`timestamp` attribute set; XML sidecar with `<FGIMeasurement>` metadata referencing the `.h5` path. |

## Parser hints

- The FGI schema places spectra inside groups (one group per measurement) with vendor-specific attribute names.
- Real FGI fixtures are not publicly available; reach out to the data owner for production samples.
- Reference readers: `h5py` for arrays, `lxml` for XML — no vendor SDK needed beyond the schema documentation.
