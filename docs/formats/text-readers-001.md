# Text Reader Batch 001

Date: 2026-05-20.

This batch establishes the first production path through the Rust registry:
`probe_path()` sniffs candidates, `open_path()` dispatches to the strongest
native reader and each reader returns normalized `SpectralRecord` values.

Implemented readers:

| Reader | Fixture | Assertion scope |
|---|---|---|
| `csv_like` | `samples/csv_tsv/synthetic_nirs.csv` | 50 records, 200-point wavelength axis, `protein` target, `sample_id` metadata. |
| `bruker_dpt` | `samples/bruker_dpt/synthetic.dpt` | One absorbance record, 200-point descending `cm-1` axis. |
| `avantes_ascii` | `samples/avantes/avantes_export.ttt` | Transmittance wave table. |
| `avantes_ascii` | `samples/avantes/irr_820_1941.IRR` | Two-column irradiance export. |
| `jcamp` | `samples/jcamp_dx/nist_water_ir.jdx` | Plain AFFN `XYDATA` with 3917 transmittance points. |
| `jcamp` | `samples/jcamp_dx/BRUKSQZ.DX`, `BRUKDIF.DX`, `SPECFILE.DX` | PAC/SQZ/DIF/DUP packed `XYDATA` ordinate decoding. |
| `jcamp` | `samples/jcamp_dx/BRUKNTUP.DX`, `TESTFID.DX` | NMR `NTUPLES` real/imaginary pages decoded into two signals. |
| `sed` | `samples/spectral_evolution/1566060_09506_working.sed` | 2151-point reflectance channel plus metadata. |
| `svc_sig` | `samples/svc_ger/BNL13001_000_moc.sig` | Reference, target and reflectance channels plus overlap quality flag. |

Known limitations:

- CSV parsing is intentionally narrow and expects numeric spectral headers.
- IDL/ENVI transposed text exports are not parsed yet.
- JCAMP `XYPOINTS`, `PEAK TABLE` and multi-block `LINK` files are not decoded
  yet.
- The Python bridge uses `nirs4all-io read-json` as temporary transport; native
  PyO3/C ABI transport is still planned.

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
