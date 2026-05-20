# Perkin Elmer Spectrum / IR `.sp` / `.fsm`

Proprietary binary. `.sp` = single spectrum, `.fsm` = imaging (Spotlight FT-IR microscope).

## Samples

| File | Type | Size | Source | License |
|---|---|---|---|---|
| `spectra.sp` | `.sp` single | 27 KB | [`paris-saclay-cds/specio@master/specio/datasets/data/spectra.sp`](https://github.com/paris-saclay-cds/specio/blob/master/specio/datasets/data/spectra.sp) | BSD-3-Clause | The reference test fixture used by `specio`'s PE reader tests. |

## Parser hints

- Reference reader: Python [`specio`](https://github.com/paris-saclay-cds/specio) (`specio.specread()` dispatches by header sniff).
- `.fsm` (imaging) is **out of scope for v1** — return a clear "imaging not supported, use `specio` directly" error if encountered.
- Header magic: PE `.sp` starts with bytes that include a block-length encoding; see `specio/plugins/perkin_elmer/_pe.py` for the reference parser.
