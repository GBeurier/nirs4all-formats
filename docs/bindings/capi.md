# C ABI

`nirs4all-formats-capi` is a small, additive C ABI over the Rust core. It is the
foundation other native bindings (MATLAB, Java/JNI, C#, Julia, Go, …) build on:
they link the C library and convert records, never reimplementing parsers.

The surface is intentionally minimal today and grows additively as reader entry
points are stabilised behind it.

## API

All symbols use the `n4fmt_` prefix and are declared in the generated header
`crates/nirs4all-formats-capi/include/nirs4all_formats.h`:

| Function | Signature | Notes |
|---|---|---|
| `n4fmt_abi_version` | `char *n4fmt_abi_version(void)` | C ABI version string. Caller owns the result — free it with `n4fmt_string_free`. |
| `n4fmt_string_free` | `void n4fmt_string_free(char *ptr)` | Frees a string returned by this ABI. `ptr` must be null or a pointer this ABI returned, freed at most once. |
| `n4fmt_core_is_available` | `bool n4fmt_core_is_available(void)` | Hook bindings use to confirm the native core is loaded. |

The ABI version is independent of the crate's semantic version.

## Build & link

```bash
cargo build -p nirs4all-formats-capi --release
```

The crate builds as a `cdylib`, `staticlib` and `rlib`, so you can link the
shared or static library. `build.rs` regenerates the header with cbindgen
(config: `cbindgen.toml`) whenever `src/lib.rs` or the cbindgen config changes; a
committed copy of the header is kept in `include/` so downstream packagers can
pin it without running cargo.

Tagged releases attach per-OS C ABI archives
(`nirs4all-formats-capi-<target>.{tar.gz,zip}`) bundling the library, the generated
header and the license — see [`RELEASE.md`](../RELEASE.md).

## Example

A C smoke test lives at `crates/nirs4all-formats-capi/examples/probe_version.c`:

```c
#include <stdio.h>
#include "nirs4all_formats.h"

int main(void) {
    char *version = n4fmt_abi_version();
    printf("nirs4all-formats C ABI %s, core available: %d\n",
           version, n4fmt_core_is_available());
    n4fmt_string_free(version);
    return 0;
}
```

## Memory & safety

- Any `char *` returned by the ABI is owned by the caller and must be released
  with `n4fmt_string_free` exactly once — do not call `free()` directly.
- The header is C and C++ friendly (`stdbool.h` / `stdint.h`).

## Roadmap

Decode entry points (probe, open-path, open-bytes returning a serialised record
buffer) are added here as they are stabilised, mirroring the CLI/Python surface.
Until then, language bindings that need full decoding can use the
`nirs4all-formats` CLI `read-json` transport, exactly as the Python and R bindings do
during bring-up.
