# SVC / GER SIG

Status: experimental.

The `.sig` reader targets Spectra Vista / GER ASCII exports whose header
contains `Spectra Vista SIG Data` and whose spectral block starts after a
literal `data=` line.

## Scope Implemented

Implemented:

- PDA and laptop-style SVC/GER headers as key/value metadata under `vendor`;
- whitespace-separated spectral rows with wavelength, reference, target and
  reflectance columns;
- normalized wavelength axis in `nm`;
- three signals per record: `reference`, `target` and `reflectance`;
- `%` unit on the reflectance channel;
- canonical `acquisition_start_*` / `acquisition_end_*` metadata from
  firmware `time` fields;
- canonical GPS latitude, longitude and GPS time metadata from SVC
  `ddmm.mmmmN` / `dddmm.mmmmW` fields when present;
- `source_signal_units` metadata from the vendor `units` header;
- canonical `instrument_model` and `instrument_serial` parsed from the SVC
  `HI: <serial> (<model>)` instrument header;
- canonical `foreoptic` array (two strings, reference and target) parsed
  from the SVC `optic=` header;
- per-detector / per-scan acquisition metadata parsed from `integration=`,
  `scan coadds=`, `temp=`, `battery=`, `error=` and `memory slot=`. The Si /
  InGaAs1 / InGaAs2 triplets are split between reference and target scans
  and surfaced as `integration_time_reference_ms`,
  `integration_time_target_ms`, `coadds_reference`, `coadds_target`,
  `detector_temperatures_reference_celsius`,
  `detector_temperatures_target_celsius`. Battery voltages, error codes and
  memory slots are surfaced as two-element arrays
  (`battery_voltages_volts`, `error_codes`, `memory_slots`);
- canonical `radiometric_factors` (three floats), `overlap_policy`
  (`preserve` / `remove`), `matching_type` and
  `overlap_break_wavelengths_nm` parsed from the SVC `factors=` bracket;
- `matched_overlap_corrected`, `overlap_removed`, `detector_overlap_preserved`
  and `resampled_export` quality flags reflecting the SVC overlap policy
  found in the `factors=` bracket (or in `comm=` / the filename for
  resampled exports);
- `white_reference` quality flag when the filename stem contains `_WR_`
  (the SVC white-reference acquisition convention used by `spectrolab`);
- `declared_bad_fixture` quality flag and warning for intentionally bad
  fixture filenames.

## Fixture Coverage

| Fixture | Variant | Coverage |
|---|---|---|
| `ACPL_D2_P1_*.sig`, `ACPL_F3_P2_B_1_000.sig` | Acer PDA clean fixtures | Golden-backed; all clean Acer fixtures now have semantic assertions for 1024 points, reflectance anchors, `detector_overlap_preserved` quality flag and the new acquisition metadata (instrument model/serial, foreoptic, factors, integration/coadds/temperatures, battery, error codes) |
| `ACPL_D2_P1_T_1_WR_000.sig` | Acer PDA white reference | Golden-backed; semantic assertions cover reflectance near 100 percent plus reference/target anchors and the `white_reference` quality flag |
| `ACPL_D2_P1_B_1_000_BAD.sig`, `3_6_PANVI_2_T_1_001_BAD.sig` | declared bad fixtures | Golden-backed; accepted but flagged as bad fixtures |
| `BNL13001_000_laptop.sig`, `BNL13002_000_laptop.sig` | laptop firmware | Golden-backed; both laptop fixtures have semantic assertions for reference/target/reflectance, `detector_overlap_preserved` flag and the new acquisition metadata |
| `BNL13001_000_moc.sig` | matched overlap corrected | Golden-backed with semantic tests for the `matched_overlap_corrected` / `overlap_removed` flags, parsed `overlap_break_wavelengths_nm = [970, 1901]` and `matching_type = "Radiance @ 976 - 1010 / NIR-SWIR On"` |
| `serbinsh_gr070214_003.sig` | GER 3700 PDA | Golden and real-sample tests, including parsed `radiometric_factors` and `overlap_break_wavelengths_nm = [985, 1906]` |
| `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | HR-1024i field resampled export | Golden and real-sample tests for resampling, overlap removal, the 350-2500 nm / 1 nm canonical axis, GPS metadata, parsed factor bracket and `LENS 4(1)` foreoptic |

## Known Gaps

- HR-1024i firmware >=3.0 variants need more independent samples (current
  fixtures only cover the spectrolab and R-FieldSpectra corpora).
- The reference / target channels report `signal_type=radiance` based on the
  `units=` header, but SVC firmware exposes uncalibrated DN-derived radiance
  with no physical unit; we deliberately leave `unit=null` on those signals.
  A redistributable calibrated `.sig` example would let us promote a real
  spectral radiance unit on top of the parsed `radiometric_factors` triplet.
- Conformance reports against `spectrolab` / `specdal` still need to be
  wired into the reverse-engineering lab. The metadata surface now mirrors
  what those R/Python libraries expose (instrument model/serial, foreoptic,
  integration time, coadds, detector temperatures, battery, error codes,
  factors and overlap policy), so a byte-level comparison can stay
  text-only.
- Per-detector spectral ranges (the Si / InGaAs1 / InGaAs2 split points)
  are not yet surfaced; they are implicit in the parsed
  `overlap_break_wavelengths_nm` when the file is overlap-removed but
  cannot currently be inferred from the raw PDA fixtures.
