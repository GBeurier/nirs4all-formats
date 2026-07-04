# Security policy

`nirs4all-formats` is a library of **file-format parsers** (Rust core + Python/R/WASM/C bindings) for
~58 NIRS/spectroscopy formats. Its primary security surface is **untrusted input**: it parses vendor
files that may be malformed or crafted.

Security-relevant properties and expectations:

- Parsers are written in **safe Rust**; `unsafe` is confined to the C ABI boundary and buffer views.
- A crafted file should fail with a clean error, never a memory-safety violation, unbounded allocation,
  or arbitrary code execution. Reports of parser panics/crashes on adversarial input are in scope.
- The WASM build runs in the browser sandbox; the C ABI hands back borrowed views (the caller must not
  free across the boundary — see the ABI docs).

The detailed parser runtime rules (bound all reads/decompression, reject archive path traversal /
absolute paths / symlinks, never execute vendor macros, treat GPS/operator/serial metadata as
sensitive) are in [`docs/SECURITY.md`](docs/SECURITY.md).

## Reporting a vulnerability

Please report security issues **privately** — do not open a public GitHub issue. Email
**nirs4all-admin@cirad.fr** with the affected version, a description, and (ideally) a minimal
reproducing input file. We aim to acknowledge within a few working days and coordinate a fix and
disclosure. Please do not attach live malware; a benign crafted reproducer is sufficient.
