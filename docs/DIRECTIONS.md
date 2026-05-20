# Architecture Decision: Rust Core From Day One

> Status: accepted.
> Date: 2026-05-20.

## Context

The previous project notes described a Python-first loader with a later Rust
hot path. That direction made sense for a quick Python package, but it does not
match the intended product:

- a low-level, durable I/O library;
- high performance over large fixture campaigns;
- bindings for several languages;
- a future C ABI surface comparable in spirit to `pls4all`;
- repeatable reverse engineering and validation across ecosystems.

## Decision

`nirs4all-io` uses Rust as the reference implementation from the start.
Python and R are first-class distribution targets, not implementation owners.

The core is split into:

- `nirs4all-io-core`: normalized record model and shared contracts;
- `nirs4all-io`: reader registry and native readers;
- `nirs4all-io-capi`: additive C ABI for bindings and external consumers;
- `nirs4all-io-cli`: inspection, probing and conversion tools.

Bindings translate Rust records into language-native objects. They must not
maintain their own parsers.

## Consequences

Positive:

- one canonical parser per format;
- easier cross-language parity;
- stronger memory-safety posture for binary formats;
- performance work happens in the same core used by every binding;
- future C ABI and WASM/JNI surfaces are planned rather than retrofitted.

Costs:

- the first implementation phase is heavier than a pure Python package;
- packaging Python/R requires native build work;
- reverse-engineered parsers need stricter API discipline because several
  bindings will consume them.

## Validation Rule

A format reader is not accepted because it parses one file. It is accepted when
the sniffer, normalized output, metadata, provenance, warnings, adversarial
behavior and reference-loader comparison are all documented and tested.

## Relationship To Existing Libraries

Existing R/Python/JS/C++ readers are reference sources for conformance and
reverse engineering. They are not runtime dependencies of the MIT core unless
their license and architecture make that explicitly acceptable.
