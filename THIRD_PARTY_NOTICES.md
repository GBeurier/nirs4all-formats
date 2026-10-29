# Third-Party Notices — nirs4all-formats

`nirs4all-formats` is distributed under `CeCILL-2.1 OR AGPL-3.0-or-later`. The two explicitly identified reader security backports under `vendor/` are bundled with their
MIT/Apache-2.0 licenses and SHA-256 provenance. Other dependencies are pulled from their
official distributions; their
licenses are acknowledged here as a courtesy and for compliance. Licenses are reported on a
best-effort basis; the authoritative text always ships with each upstream project.

It is built on the Rust crate ecosystem. The overwhelming majority of its dependencies are
published under permissive licenses — predominantly **MIT** and/or **Apache-2.0**, with a few
under **BSD** terms. Principal direct dependencies include:

| Component | License (SPDX) | Upstream |
|---|---|---|
| `serde`, `serde_json` | MIT OR Apache-2.0 | https://github.com/serde-rs |
| `anyhow`, `thiserror` | MIT OR Apache-2.0 | https://github.com/dtolnay |
| `clap` | MIT OR Apache-2.0 | https://github.com/clap-rs/clap |
| `sha2` (RustCrypto) | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |

> Note: GPL-licensed reference readers are used only for **conformance testing**, isolated behind
> subprocesses, and are **not** linked into the shipped library.

The developer-only fuzz seed `fuzz/corpus/registry_open_bytes/02-msa-minimum-metadata`
copies `samples/msa_iso22029/minimum_metadata.msa` (Git blob
`f0b8195d640f2ea64db4ac023847541f7956c273`). The [sample provenance](samples/msa_iso22029/README.md)
identifies RosettaSciIO and GPL-3.0. This fixture is not linked into the library.

For the exhaustive, version-pinned dependency tree and its licenses, run:

```
cargo tree
cargo deny check licenses   # if cargo-deny is configured
```

License-family texts are bundled under [`LICENSES/`](LICENSES/): MIT, Apache-2.0, BSD-3-Clause.

## Bundled reader security backports

`vendor/hdf5-reader` and `vendor/netcdf-reader` retain the original 0.9.3 sources and
licenses, with new package identities `nirs4all-hdf5-reader` and
`nirs4all-netcdf-reader` version `0.9.3-nirs4all.1`. Their `PROVENANCE.json` records
the upstream archive digest, revision and every unchanged source digest. The only
functional dependency change is HDF5's `lru` 0.16.4 → 0.18.5, fixing
RUSTSEC-2026-0253. No parser or numerical implementation is replaced.

These packages must be published in HDF5 → netCDF → Formats order before a public
Formats crate release. Normal versioned path dependencies make this requirement
visible to consumers; no root-only `[patch]` hides it. Local Python/R/WASM builds
use the bundled sources.
