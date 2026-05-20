# ASD FieldSpec

Status: Experimental. The primary-spectrum subset for revisions 1/6/7/8 is
golden-backed; undecoded embedded secondary/dependent/reference/calibration
blocks and separate ASD companion calibration files keep the wider ASD family
partial in `FORMAT_MATRIX.md`.

Implemented scope:

- file-version sniffing for `ASD`, `as6`, `as7` and `as8` magic prefixes;
- support for ASD files without `.asd` extension, such as `.000`;
- fixed header fields needed for primary loading: channel count, first
  wavelength, wavelength step, data type and data format;
- explicit `trailing_block_bytes` metadata for undecoded internal ASD blocks;
- primary spectrum payload in `float32`, `int32` and `float64` encodings;
- normalized output as one `SpectralRecord` with a wavelength axis in `nm`.

Covered fixtures:

| Fixture | Revision | Data format | Signal type |
|---|---:|---|---|
| `samples/asd/3L9257.000` | 1 | `float32` | reflectance |
| `samples/asd/v6sample00000.asd` | 6 | `float64` | raw counts |
| `samples/asd/v7_field_44231B009.asd` | 7 | `float64` | reflectance |
| `samples/asd/v7sample00000.asd` | 7 | `float64` | radiance |
| `samples/asd/soil.asd` | 8 | `float64` | raw counts |
| `samples/asd/v8sample00001.asd` | 8 | `float64` | raw counts |

Reference readers:

- `pyASDReader` for revision 6/7/8 metadata and spectrum checks;
- `prospectr::readASD()` fixture coverage for the legacy `.000` sample;
- `asdreader` and `spectrolab` remain reference candidates for deeper
  conformance once the R reference path is automated.

Known limitations:

- embedded secondary spectrum blocks are not decoded yet;
- embedded classifier/dependent variable blocks are not decoded yet;
- embedded reference spectrum blocks are not decoded yet;
- embedded calibration headers and calibration spectra are not decoded yet;
- audit log/signature blocks are not decoded yet;
- separate ASD `.ILL`, `.REF` and `.RAW` calibration companion files still have
  no open fixture.

The reader emits a `trailing_asd_blocks_not_decoded` warning when bytes remain
after the primary spectrum. That is expected for newer ASD revisions until the
secondary block parsers are implemented; the same byte count is exposed in
`metadata.asd.trailing_block_bytes` for downstream auditing.
