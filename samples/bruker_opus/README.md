# Bruker OPUS — native binary

Proprietary, reverse-engineered, block-based format. Files have **no fixed extension** — OPUS appends a numeric counter (`.0`, `.0000`, `.001`, …) corresponding to the measurement number. Each file mixes parameter blocks (instrument settings, sample metadata) with one or more spectral data blocks (single-beam, absorbance, transmittance, interferogram, etc.).

## Samples

### From [`spectral-cockpit/opusreader2@master/inst/extdata`](https://github.com/spectral-cockpit/opusreader2/tree/master/inst/extdata) (GPL-3, R package)

| File | Size | Notes |
|---|---|---|
| `test_spectra.0` | 70 KB | Generic test fixture used by `opusreader2` regression tests. |
| `617262_1TP_C-1_A5.0` | 284 KB | Soil sample, OSSL/`opusreader2` test. Multiple data blocks present. |
| `MMP_2107_Test1.001` | 187 KB | `.001` extension — confirms OPUS files are not always `.0`. |
| `BF_lo_01_soil_cal.1` | 36 KB | Calibration spectrum. |
| `issue82_Opus_test.0` | 63 KB | Regression fixture from issue 82 of opusreader2 (unusual block layout). |

### From [`pierreroudier/opusreader@main/inst/extdata`](https://github.com/pierreroudier/opusreader/tree/main/inst/extdata) (GPL-3, R package)

| File | Size | Notes |
|---|---|---|
| `opusreader_test_spectra.0` | 70 KB | Second independent OPUS fixture (Bruker Vertex FTIR via pierreroudier's `opusreader`). Different acquisition setup than the spectral-cockpit one — exercises the cross-reader compatibility path. |

### From [`joshduran/brukeropus@master/examples`](https://github.com/joshduran/brukeropus/tree/master/examples) (MIT)

| File | Size | Notes |
|---|---|---|
| `brukeropus_file.0` | 240 KB | Bruker OPUS example shipped with the MIT-licensed `brukeropus` Python reader. Multiple parameter + data blocks; useful for cross-implementation parity tests. |

### From [`cran/soil.spec@master/inst`](https://github.com/cran/soil.spec/tree/master/inst) (GPL-2/3, CRAN R package — AfSIS soils project)

| File | Size | Notes |
|---|---|---|
| `icr_087266_B2.0` | 164 KB | African soil spectroscopy sample acquired on a Bruker MPA / Tensor for the **AfSIS** project (World Agroforestry Centre). Confirms OPUS coverage extends to MPA-class instruments commonly used in agro-NIRS. |
| `icr_087273_G3.0` | 164 KB | Sibling soil sample (different horizon). Together with the file above, exercises the multi-file directory-batch ingestion path. |

## Parser hints

- Detect by header magic: bytes 0-3 are `0a 0a fe fe` (the OPUS magic for newer files); some older files start with `0a 0a 1a 1a`. Never route on the extension alone.
- Coverage now spans **OPUS 7.x / 8.x** (spectral-cockpit, opusreader, brukeropus) and **Bruker MPA/Tensor soils workflow** (cran/soil.spec, AfSIS). OPUS 5.x / 6.x legacy archives remain the only documented gap.
- Reference readers:
  - R: [`opusreader2`](https://github.com/spectral-cockpit/opusreader2) (production-quality, actively maintained)
  - Python: [`brukeropusreader`](https://github.com/qedsoftware/brukeropusreader), [`brukeropus`](https://github.com/joshduran/brukeropus), [`opusFC`](https://stuart-cls.github.io/python-opusfc-dist/), [`spectrochempy.read_opus()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_opus.html). Coverage and completeness vary across readers.
- Multi-block files: a single OPUS file commonly contains AB, SB, RF, IFG, etc. The loader should expose them as a `signals` dict, not collapse to a single intensity array.
