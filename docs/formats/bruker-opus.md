# Bruker OPUS Native

Status: experimental native reader.

## Scope Implemented

The Rust reader detects OPUS by binary magic (`0a 0a fe fe`) and never by
extension. OPUS files often use numeric extensions such as `.0`, `.1`, `.001`
or `.0000`.

Implemented:

- OPUS header and directory parsing;
- parameter block parsing for integer, float and string values;
- 1D data block parsing for float32 and int32 payloads;
- appairing data blocks with matching data-status parameter blocks;
- `CSF` scaling, `NPT` trimming, and generated X axes from `FXV`, `LXV`;
- duplicate data block names with stable suffixes such as `absorbance_2`;
- multi-signal records containing absorbance, reflectance, sample/reference
  spectra, sample/reference interferograms and phase blocks when present.

Not implemented yet:

- OPUS old magic (`0a 0a 1a 1a`);
- 3D/time-resolved data series;
- report/subreport tables as structured targets;
- image blocks and embedded visual data;
- full parameter label expansion and typed promotion of sample properties.

Unsupported or unpaired data blocks are preserved as provenance warnings.

## Record Mapping

Each OPUS file currently becomes one `SpectralRecord` with a `signals` map.
Signal names are semantic rather than OPUS abbreviations: for example
`absorbance`, `reflectance`, `sample_spectrum`, `reference_spectrum`,
`sample_interferogram`, `reference_interferogram`, `sample_phase`, `match` and
`match_2ch`.

Header and directory information is stored under `bruker_opus`. Per-signal
data-status parameters are stored under `bruker_opus_signal_params`. Other
parameter blocks are kept under `bruker_opus_params`.

## Fixtures and Reference Checks

Committed smoke and golden coverage currently includes:

| Fixture | Expected shape |
|---|---|
| `617262_1TP_C-1_A5.0` | 1 record, 5 signals, absorbance has 3578 points |
| `test_spectra.0` | 1 record, reflectance/sample/reference spectra |
| `BF_lo_01_soil_cal.1` | duplicate absorbance blocks become `absorbance` and `absorbance_2` |
| `icr_087266_B2.0`, `icr_087273_G3.0` | AfSIS Bruker MPA soil fixtures, absorbance plus sample/reference spectra and interferogram |

Reference controls were checked against local Python readers `brukeropus`,
`opusFC` and `brukeropusreader`:

- `617262_1TP_C-1_A5.0`: absorbance first X `7497.697861`, first Y
  `0.552472949`.
- `test_spectra.0`: reflectance first X `7498.291691`, first Y
  `0.524343193`.
- `BF_lo_01_soil_cal.1`: latest absorbance first Y `0.123978466`; older
  duplicate first Y `0.123221688`.

The readers disagree on some older or report-like blocks. `brukeropus` was
used as the naming/order reference for duplicate 1D blocks; `opusFC` was used
to confirm OPUS directory content and primary arrays.

## Next Work

- Add adversarial truncation tests for header, directory, parameter blocks and
  data/status mismatch cases.
- Add full-array external conformance scripts for the three reference readers.
- Decode 3D data series into either multiple records or a documented series
  representation.
- Promote report/subreport quantitative values into `targets` when they are
  clearly sample reference properties.
