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
- `matched_overlap_corrected`, `overlap_removed` and `resampled_export`
  quality flags when the header or filename exposes those processing steps;
- `declared_bad_fixture` quality flag and warning for intentionally bad
  fixture filenames.

## Fixture Coverage

| Fixture | Variant | Coverage |
|---|---|---|
| `ACPL_D2_P1_*.sig`, `ACPL_F3_P2_B_1_000.sig` | Acer PDA clean fixtures | Golden-backed; all clean Acer fixtures now have semantic assertions for 1024 points, no warnings, no quality flags and reflectance anchors |
| `ACPL_D2_P1_T_1_WR_000.sig` | Acer PDA white reference | Golden-backed; semantic assertions cover reflectance near 100 percent plus reference/target anchors |
| `ACPL_D2_P1_B_1_000_BAD.sig`, `3_6_PANVI_2_T_1_001_BAD.sig` | declared bad fixtures | Golden-backed; accepted but flagged as bad fixtures |
| `BNL13001_000_laptop.sig`, `BNL13002_000_laptop.sig` | laptop firmware | Golden-backed; both laptop fixtures have semantic assertions for reference/target/reflectance and no quality flags |
| `BNL13001_000_moc.sig` | matched overlap corrected | Golden-backed with semantic test for overlap quality flag |
| `serbinsh_gr070214_003.sig` | GER 3700 PDA | Golden and real-sample tests |
| `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | HR-1024i field resampled export | Golden and real-sample tests for resampling, overlap removal and GPS metadata |

## Known Gaps

- HR-1024i firmware >=3.0 variants need more independent samples.
- Unit parsing still identifies the reference/target channels as radiance by
  label; physical radiance units are not provided by the observed SIG headers.
- Conformance reports against `spectrolab` / `specdal` still need to be wired
  into the reverse-engineering lab.
