# JASCO JWS

Status: experimental.

The JASCO `.jws` reader covers OLE2 compound-document files that expose the
reverse-engineered stream pair seen in the committed fixtures:

- `DataInfo` stores the channel count, point count and spectral axis endpoints;
- `Y-Data` stores float32 ordinate values;
- `BaseInfo`, when present, contributes the original source path as metadata.

Single-channel files are emitted as one `signal` array. Multi-channel files are
emitted as `channel_1`, `channel_2`, ... with a shared spectral axis. The
current reader does not yet infer JASCO-specific channel semantics such as CD,
HT, absorbance or fluorescence from measurement metadata; those labels remain a
future compatibility task against open JWS readers.

## Supported Fixtures

| Fixture | Records | Axis | Signals | Notes |
|---|---:|---|---|---|
| `samples/jasco/243.jws` | 1 | wavenumber, `cm-1`, 7729 points | `signal` | OLE2/DataInfo/Y-Data single channel |
| `samples/jasco/sample_fluorescence.jws` | 1 | wavelength, `nm`, 301 points | `signal` | OLE2/DataInfo/Y-Data single channel |
| `samples/jasco/sample_CD_HT_Abs.jws` | 1 | wavelength, `nm`, 1501 points | `channel_1`, `channel_2`, `channel_3` | OLE2/DataInfo/Y-Data multi-channel |

## Dispatch Boundaries

The reader requires both a `.jws` extension and an OLE2 compound-document
header. Text exports from JASCO remain covered by `row-spectral-table`.

Other public JWS reverse-engineering projects describe variants with streams
such as `Data`, `Header` or `XdataValue`. Those layouts remain pending until
fixtures are available.
