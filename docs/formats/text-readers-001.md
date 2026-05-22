# Text Reader Batch 001

Date: 2026-05-20.

This batch establishes the first production path through the Rust registry:
`probe_path()` sniffs candidates, `open_path()` dispatches to the strongest
native reader and each reader returns normalized `SpectralRecord` values.

Implemented readers:

| Reader | Fixture | Assertion scope |
|---|---|---|
| `csv_like` | `samples/csv_tsv/synthetic_nirs.csv`, `.tsv`, `synthetic_nirs_semicolon.csv` | 50 records, 200-point wavelength axis, `protein` target, `sample_id` metadata; comma CSV, tab TSV and semicolon CSV variants are golden-backed. |
| `bruker_dpt` | `samples/bruker_dpt/synthetic.dpt`, `RS-1.dpt` | One absorbance record, `cm-1` axis, synthetic descending fixture plus real `lightr` fixture. |
| `avantes_ascii` | `samples/avantes/avantes_export.ttt` | Transmittance wave table. |
| `avantes_ascii` | `samples/avantes/irr_820_1941.IRR` | Two-column irradiance export. |
| `jcamp` | `samples/jcamp_dx/nist_water_ir.jdx` | Plain AFFN `XYDATA` with 3917 transmittance points. |
| `jcamp` | `samples/jcamp_dx/BRUKSQZ.DX`, `BRUKDIF.DX`, `SPECFILE.DX` | PAC/SQZ/DIF/DUP packed `XYDATA` ordinate decoding. |
| `jcamp` | `samples/jcamp_dx/BRUKNTUP.DX`, `TESTFID.DX` | NMR `NTUPLES` real/imaginary pages decoded into two signals. |
| `jcamp` | `samples/ocean_optics/OceanOptics_period.jdx` | `LINK`/`XYPOINTS` sample, dark and reference blocks plus computed transmittance. |
| `sed` | `samples/spectral_evolution/1566060_09506_working.sed` | 2151-point reflectance channel plus metadata. |
| `sed` | `samples/spectral_evolution/1566060_15025_not_working.sed` | Broken-but-valid DN-only fixture accepted with `missing_reflectance_signal` quality flag. |
| `svc_sig` | `samples/svc_ger/BNL13001_000_moc.sig` | Reference, target and reflectance channels plus overlap quality flag. |
| `svc_sig` | `samples/svc_ger/*_BAD.sig` | Deliberately malformed fixture filenames are accepted but flagged as `declared_bad_fixture`. |
| `usgs_aref` | `samples/envi_sli/usgs_liba_AREF.txt` | Single-column USGS AREF reflectance dump with generated index axis and explicit warning. |
| `spectral_table` | Si-Ware CSV, MODTRAN `.dat`, PP Systems `.SPT/.SPU`, ENVI/ECOSTRESS `.spectrum.txt`, Shimadzu TXT, USGS SPECPR ASCII, WiTec TXT | Row-oriented axis-first spectral tables, one normalized signal per numeric column after the axis. |
| `spectral_matrix` | Foss/WinISI text, Metrohm Vision Air CSV, VIAVI MicroNIR CSV | One spectrum per sample row, numeric headers or `Wavelengths:` block become the axis, property columns become targets. |
| `sun_photometer` | MFR `.OUT`, Microtops `.TXT` | Channel columns become a short wavelength axis, one record per observation row. |

Known limitations:

- CSV parsing is intentionally narrow and expects numeric spectral headers.
- Single-column text dumps without an embedded axis still need a sidecar axis,
  except the dedicated USGS AREF single-column path, which emits an index axis
  with a warning because no wavelength vector is present in the file.
- Target-only reports without spectra are not loaded into `SpectralRecord`.
- JCAMP `PEAK TABLE` / `PEAK ASSIGNMENTS` (sparse top-level), top-level
  multi-block `XYDATA`, and `##DATA TYPE=LINK` files are decoded:
  same-axis LINKs collapse into a single composite record while
  heterogeneous LINKs fan out (one record per child plus `link_*`
  metadata) since M3 (2026-05-23). See `docs/formats/jcamp-dx.md` for
  the full LINK contract.
- Python and R bindings ship a native PyO3 / extendr-api static library
  (M1 2026-05-22) with the legacy `nirs4all-io read-json` CLI subprocess
  retained as a fallback when the native module is unavailable.

Green gate:

```bash
. "$HOME/.cargo/env"
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python -m pip install -e tools/reverse-lab -e "bindings/python[numpy,pandas]"
python -m pytest tools/reverse-lab/tests bindings/python/tests
```

Golden summaries:

- stored in `crates/nirs4all-io/tests/goldens/`;
- checked by `cargo test --workspace`;
- intentionally cover stable normalized-output summaries before full
  reference-reader array comparisons are wired in.
