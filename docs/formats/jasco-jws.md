# JASCO JWS

Status: experimental.

The JASCO `.jws` reader covers OLE2 compound-document files that expose the
reverse-engineered stream pair seen in the committed fixtures:

- `DataInfo` stores the channel count, point count and spectral axis endpoints;
- `Y-Data` stores float32 ordinate values;
- `BaseInfo`, when present, contributes the original source path as metadata;
- `ModuleInfo`, `SampleInfo`, `UserInfo` and `MeasParam` contribute instrument,
  sample and measurement hints used for conservative semantic channel labels.

The current committed fixtures are labelled semantically when the metadata is
specific enough:

- FT/IR single-channel spectra with percent-scale ordinates are emitted as
  `transmittance` with unit `%T`;
- fluorescence spectra from `FP-*` modules are emitted as `fluorescence`;
- CD multi-channel files from `CD-1500` / `J-1500` modules are emitted as
  `cd` (`mdeg`), `ht` (`V`) and `absorbance` (`dOD`).

Unknown layouts fall back to `signal` or `channel_N` names.

## Supported Fixtures

| Fixture | Records | Axis | Signals | Notes |
|---|---:|---|---|---|
| `samples/jasco/243.jws` | 1 | wavenumber, `cm-1`, 7729 points | `transmittance` | FT/IR-4100 percent-transmittance single channel |
| `samples/jasco/sample_fluorescence.jws` | 1 | wavelength, `nm`, 301 points | `fluorescence` | FP-8300 fluorescence single channel |
| `samples/jasco/sample_CD_HT_Abs.jws` | 1 | wavelength, `nm`, 1501 points | `cd`, `ht`, `absorbance` | CD-1500/J-1500 multi-channel CD/HT/Abs file |

## Dispatch Boundaries

The reader requires both a `.jws` extension and an OLE2 compound-document
header. Text exports from JASCO remain covered by `row-spectral-table`.

Other public JWS reverse-engineering projects describe variants with streams
such as `Data`, `Header` or `XdataValue`. Those layouts remain pending until
fixtures are available.
