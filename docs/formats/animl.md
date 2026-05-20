# AnIML XML

Status: experimental.

The AnIML reader covers XML documents that expose a spectral `SeriesSet` with
an axis series named or identified as wavelength or wavenumber, plus one or
more same-length signal series.

## Supported Fixtures

| Fixture | Records | Axis | Signals | Targets |
|---|---:|---|---|---|
| `samples/animl/synthetic_nirs.animl` | 1 | wavelength, `nm` | `absorbance` | `protein` |

The reader preserves `sample_id` and `sample_name` metadata from `Sample`
attributes. Numeric `Parameter` values inside `SampleSet` are emitted as
record targets.

## Dispatch Boundaries

AnIML is a broad analytical container, not only a spectroscopy format. The
committed `samples/animl/Example3.animl` fixture contains non-spectral NMR/DLS
result parameters and is intentionally refused because it has no supported
spectral axis series.

Supported value blocks are currently explicit `<F>` and `<D>` values. Uniform
`AutoIncrementedValueSet` axis reconstruction and broader schema validation are
reserved for the next AnIML hardening pass.
