# Allotrope ADF `.adf` — NOT FOUND

The Allotrope Data Format (ADF) is a binary HDF5 file paired with an RDF triplestore. Production-grade pharma-track standard.

## Status: no permissively-licensed open sample exists

After exhaustive search (Benchling-Open-Source/allotropy, AllotropeFoundation/*, allotrope-open-source/*, allotrope.org direct URLs, GitHub code search), **no public `.adf` binary fixture is available**. Real ADF files are gated behind the Allotrope Foundation membership and per-instrument vendor SDKs.

## What we have instead

See `allotrope_asm/` — Allotrope **Simple Model** (ASM JSON) is the practical Allotrope path used by [Benchling/allotropy](https://github.com/Benchling-Open-Source/allotropy) and ships many open fixtures.

## Recommendations for the parser

- **Detection**: ADF is HDF5 (magic `\x89HDF\r\n\x1a\n`) with specific Allotrope groups (`/Allotrope`, `/DataPackage`, etc.). Probe these groups, and refuse with a pointer to `allotropy` if found.
- **Stub for tests**: if a refusal-path test is needed, generate a synthetic HDF5 with an empty `/Allotrope` group — that's enough to validate the detection logic.
- **Real samples**: request from allotrope.org (membership-gated) or from a downstream pharma user. Update this README when a real fixture is added.
