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
- `matched_overlap_corrected` quality flag for matched-overlap-corrected
  exports;
- `declared_bad_fixture` quality flag and warning for intentionally bad
  fixture filenames.

## Fixture Coverage

| Fixture | Variant | Coverage |
|---|---|---|
| `BNL13001_000_laptop.sig` | laptop firmware | Semantic test: 1024 points, reference/target/reflectance, no quality flags |
| `BNL13001_000_moc.sig` | matched overlap corrected | Semantic test for overlap quality flag |
| `ACPL_D2_P1_B_1_001.sig` | Acer PDA clean fixture | Semantic test: 1024 points, no warnings or quality flags |
| `ACPL_D2_P1_B_1_000_BAD.sig`, `3_6_PANVI_2_T_1_001_BAD.sig` | declared bad fixtures | Accepted but flagged as bad fixtures |
| `serbinsh_gr070214_003.sig` | GER 3700 PDA | Golden and real-sample tests |
| `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | HR-1024i field resampled export | Golden and real-sample tests |

## Known Gaps

- Firmware-specific GPS/date/unit fields are preserved as raw vendor metadata
  but are not yet promoted into canonical metadata.
- HR-1024i firmware >=3.0 variants need more independent samples.
- Resampled exports are not distinguished from raw acquisitions beyond the
  vendor comment text.
- Conformance reports against `spectrolab` / `specdal` still need to be wired
  into the reverse-engineering lab.
