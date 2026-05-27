# mzML / mzMLb

HUPO PSI standard for mass spectrometry. Not a NIRS format, but cited in `FORMATS.md` §3 as design inspiration for our internal schema (XML with `<spectrum>`/`<chromatogram>` elements, base64-encoded binary arrays).

## Samples

All from [`pymzml/pymzML@dev/tests/data`](https://github.com/pymzml/pymzML/tree/dev/tests/data) — MIT.

| File | Notes |
|---|---|
| `example.mzML` | Generic small mzML spectrum collection. |
| `mini.chrom.mzML` | Minimal chromatogram example. |
| `mini_numpress.chrom.mzML` | Chromatogram with **numpress compression** (Pi/SLOF) — useful for binary-decoding tests. |

## Parser hints

- mzML is XML; `<binaryDataArray>` elements carry base64-encoded float32/float64 with optional zlib + numpress compression.
- Reference readers: [`pymzml`](https://pymzml.readthedocs.io/), [`pyteomics`](https://pyteomics.readthedocs.io/).
- For NIRS the spec is interesting because it solves the same problem we have (uniform schema across instruments) — see `pyteomics.mzml.MzML` for an API design reference.
- `nirs4all-formats` detects these files and refuses them as mass-spectrometry data instead of coercing `m/z` spectra into the optical `SpectralRecord` model.
