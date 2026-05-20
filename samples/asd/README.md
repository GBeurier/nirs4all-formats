# ASD `.asd` (FieldSpec / Malvern Panalytical)

Binary, multiple revisions. Reverse-engineered headers expose DN / white reference / radiance / reflectance + GPS + timestamps. The revision flag must be parsed before deciding payload offsets.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `soil.asd` | 34 KB | [`pierreroudier/asdreader`](https://github.com/pierreroudier/asdreader/blob/master/inst/extdata/soil.asd) | GPL-3 | Single soil reflectance spectrum, ASD FieldSpec. Used as the primary fixture by the R `asdreader::get_spectra()` tests. |
| `3L9257.000` | 9 KB | [`l-ramirez-lopez/prospectr`](https://github.com/l-ramirez-lopez/prospectr/blob/master/tests/testthat/testdata/3L9257.000) | MIT | Despite the `.000` extension this is an ASD-format file (confirmed by `file(1)` → "ASD archive data"). Demonstrates that ASD files do not always carry the `.asd` extension. |
| `v6sample00000.asd` | 34 KB | [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader/blob/main/tests/sample_data/v6sample/v6sample00000.asd) | MIT | **ASD file format revision 6** — used to test legacy-header parsing. |
| `v7sample00000.asd` | 85 KB | [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader/blob/main/tests/sample_data/v7sample/v7sample00000.asd) | MIT | **ASD file format revision 7**. |
| `v8sample00001.asd` | 36 KB | [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader/blob/main/tests/sample_data/v8sample/v8sample00001.asd) | MIT | **ASD file format revision 8** — newest revision; parser must dispatch on the version prefix. |
| `v7_field_44231B009.asd` | 52 KB | [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader/blob/main/tests/sample_data/v7sample_field_spectroscopy/44231B009-1-FW3R00000.asd) | MIT | Real field-spectroscopy v7 spectrum — populated app-data bytes, one internal absolute-calibration spectrum and an ASD footer marker. |

## Fixture-backed parser coverage

The Rust reader currently emits one primary `SpectralRecord` per ASD file and
stores additional fixture-backed diagnostics in `metadata.asd`.

| File | Internal block coverage |
|---|---|
| `3L9257.000` | Legacy revision-1 float32 primary spectrum; no trailing internal blocks. |
| `v6sample00000.asd` | Reference header/spectrum plus empty classifier block inventoried. |
| `v7sample00000.asd` | Reference block, empty classifier/dependent blocks, three calibration spectra (`BSE`, `LMP`, `RAW`) inventoried. |
| `v7_field_44231B009.asd` | Reference block, empty classifier/dependent blocks, one absolute calibration spectrum and footer marker inventoried. |
| `soil.asd` | Reference block, empty classifier/dependent/calibration/audit blocks, unsigned signature placeholder and padding inventoried. |
| `v8sample00001.asd` | Reference block, material-report classifier, dependent variables, audit event and signed signature inventoried. |

## Parser hints

- Header starts with the file-version string (`as` prefix). Read it first; offsets and payload semantics depend on the revision.
- Endianness: little-endian throughout.
- Reference readers:
  - R: [`asdreader`](https://github.com/pierreroudier/asdreader), [`prospectr::readASD()`](https://l-ramirez-lopez.github.io/prospectr/reference/readASD.html), [`spectrolab::read_spectra(format="asd")`](https://github.com/meireles/spectrolab)
  - Python: [`specdal`](https://github.com/EnSpec/SpecDAL), `pyASDReader`

## Calibration companions (NOT FOUND)

ASD `.ILL` / `.REF` / `.RAW` calibration companion files are referenced by the FieldSpec workflow when converting DN → radiance → reflectance. **No open-source sample is available for these** — they ship with the ASD instrument SDK and are not redistributed publicly. SPECCHIO has partial support behind login. Track this as an open gap: either generate a synthetic stand-in once the binary layout is reverse-engineered, or document the path as "vendor SDK only" and accept that a small share of ASD workflows will not round-trip.
