# Registry fuzz target

`registry_open_bytes` sends bounded in-memory payloads through the public
`nirs4all_formats::open_bytes` registry entry point. The first input byte selects
one of nine static file names; the remaining payload is limited to 1 MiB. No
fuzz-controlled path is opened and the harness does not access the network.

The five small seeds are reproducibly copied from tracked `samples/` fixtures:

| Corpus seed | Selector | Source fixture | Provenance |
|---|---:|---|---|
| `00-jcamp-peak-assignments` | 0 | `samples/jcamp_dx/synthetic_peak_assignments.jdx` | Hand-crafted local regression fixture |
| `01-perkin-elmer-trailing` | 1 | `samples/perkin_elmer/synthetic_trailing.sp` | Generated locally, CC0 |
| `02-msa-minimum-metadata` | 2 | `samples/msa_iso22029/minimum_metadata.msa` | RosettaSciIO public fixture, GPL-3.0 |
| `03-viavi-micronir` | 3 | `samples/viavi_micronir/synthetic_micronir.sam` | Generated locally, CC0 |
| `04-animl-autoincrement` | 4 | `samples/animl/synthetic_nirs_autoincrement.animl` | Generated locally, CC0 |

Rebuild and verify the corpus without network access:

```bash
/usr/bin/python3.11 fuzz/scripts/build_seed_corpus.py
/usr/bin/python3.11 -m pytest -q tests/security/test_fuzz_scaffold.py
```

When a pre-provisioned nightly Rust toolchain and `cargo-fuzz` are available,
start a campaign explicitly from the repository root:

```bash
mkdir -p fuzz/campaign/registry_open_bytes
cp fuzz/corpus/registry_open_bytes/* fuzz/campaign/registry_open_bytes/
cargo +nightly fuzz run registry_open_bytes fuzz/campaign/registry_open_bytes -- \
  -dict=fuzz/dictionaries/registry_open_bytes.dict -max_len=1048577
```

The campaign writes only to the ignored scratch corpus, preserving the five
tracked provenance seeds. The `max_len` includes the one-byte selector. This
repository does not install toolchains, fetch dependencies, or start fuzz
campaigns automatically. The minimal harness disables optional format features;
separate full-feature and sidecar-aware targets remain future work.
