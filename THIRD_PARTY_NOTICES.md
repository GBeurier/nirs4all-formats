# Third-Party Notices — nirs4all-formats

`nirs4all-formats` is distributed under `CeCILL-2.1 OR AGPL-3.0-or-later`. It does **not**
vendor the components below — they are pulled from their official distributions — but their
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

For the exhaustive, version-pinned dependency tree and its licenses, run:

```
cargo tree
cargo deny check licenses   # if cargo-deny is configured
```

License-family texts are bundled under [`LICENSES/`](LICENSES/): MIT, Apache-2.0, BSD-3-Clause.
