# PP Systems UniSpec SC/DC

Status: experimental partial.

PP Systems UniSpec SC (`.SPT`) and UniSpec DC (`.SPU`) exports are handled
through the row-oriented spectral table reader when they expose an axis-first
ASCII table. The current committed fixtures are synthetic, so this reader is
kept partial until a raw field acquisition can validate real headers, units and
instrument metadata.

## Supported Fixtures

| Fixture | Instrument class | Records | Axis | Signals |
|---|---|---:|---|---|
| `samples/pp_systems/synthetic_unispec.SPT` | UniSpec SC | 1 | 200 wavelengths, `1100..2500 nm` | `dn_white`, `dn_target`, `reflectance` |
| `samples/pp_systems/synthetic_unispec_dc.SPU` | UniSpec DC | 1 | 200 wavelengths, `1100..2500 nm` | `channel_a_dn`, `channel_b_dn`, `reflectance` |

`DN` columns are emitted as raw-count signals. `Reflectance` columns are
emitted as reflectance signals. Header key/value lines such as `File`, `Date`
and `Notes` are preserved under `metadata.vendor`.

## Dispatch Boundaries

The parser requires an explicit wavelength axis column followed by numeric
signal columns. It does not claim arbitrary PP Systems reports by extension
alone.

The local Arctic LTER UniSpec-DC CSV/XLSX files are vegetation-index products
(`NDVI`, `EVI`, `PRI`, etc.), not raw `.SPT/.SPU` spectra. They remain expected
refusals and do not change the raw UniSpec coverage status.

## Remaining Gaps

- real redistributable UniSpec SC `.SPT` field export;
- real redistributable UniSpec DC `.SPU` field export with two radiometer
  channels;
- typed normalization of PP Systems acquisition metadata;
- comparison against SPECCHIO or another trusted reference import path if one
  becomes available.
