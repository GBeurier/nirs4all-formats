# Thermo Nicolet OMNIC `.spa` / `.spg` / `.srs` / `.srsx`

Proprietary, reverse-engineered. `.spa` = single spectrum, `.spg` = group/multi, `.srs`/`.srsx` = time series. Multiple undocumented variants per OMNIC release.

## Samples

| File | Type | Size | Source | License |
|---|---|---|---|---|
| `wodger.spg` | `.spg` group | 177 KB | [`spectrochempy/spectrochempy@master/docs/sources/userguide/importexport/wodger.spg`](https://github.com/spectrochempy/spectrochempy/blob/master/docs/sources/userguide/importexport/wodger.spg) | CeCILL-B | The reference test fixture used by SpectroChemPy's `read_omnic()` documentation and tests. |
| `2-BaSO4_0.SPA` | `.spa` single | 174 KB | [`spectrochempy/spectrochempy_data@master/testdata/irdata/carroucell_samp/2-BaSO4_0.SPA`](https://github.com/spectrochempy/spectrochempy_data/blob/master/testdata/irdata/carroucell_samp/2-BaSO4_0.SPA) | CeCILL-B | Real BaSO₄ FT-IR sample from the SpectroChemPy carroucell example dataset. |
| `11-Z25-CP_0.SPA` | `.spa` single | 179 KB | SpectroChemPy data | CeCILL-B | Additional SPA regression fixture. |
| `CO_at_Mo_Al2O3.SPG` | `.spg` group | 1.4 MB | SpectroChemPy data | CeCILL-B | 19-record grouped spectrum fixture. |
| `nh4y-activation.spg` | `.spg` group | 8.4 MB | SpectroChemPy data | CeCILL-B | 55-record activation group fixture. |
| `TGAIR.srs` | `.srs` time series | 2.6 MB | [`spectrochempy/spectrochempy_data@master/testdata/irdata/subdir/TGAIR-unreadable.srs`](https://github.com/spectrochempy/spectrochempy_data/blob/master/testdata/irdata/subdir/TGAIR-unreadable.srs) | CeCILL-B | TGA-IR coupled time-series file. ⚠ The upstream filename is `TGAIR-unreadable.srs` — SpectroChemPy itself flags it as not fully decodable, so it's the canonical "hard case" fixture for `.srs` parsers. |

## Parser hints

- Magic: bytes 0-3 are typically `0a 5a a8 66` for `.spa` (varies across OMNIC versions).
- Reference readers:
  - Python: [`spectrochempy.read_omnic()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_omnic.html) (covers `.spa`/`.spg`); [`lerkoah/spa-on-python`](https://github.com/lerkoah/spa-on-python) (`.spa` only); see also OpenChrom community SPA reader.
- Multi-block `.spg` files should be exposed as a `SpectralCollection` (one record per sub-spectrum) rather than collapsed.
- `.srs` (time series) is highly variable across OMNIC versions — not all `.srs` are readable even with `spectrochempy`.
