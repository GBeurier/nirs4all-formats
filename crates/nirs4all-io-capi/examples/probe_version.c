/*
 * Smoke test for the nirs4all-io C ABI archive.
 *
 * Build manually (after `cargo build -p nirs4all-io-capi --release`):
 *
 *   gcc -I crates/nirs4all-io-capi/include \
 *       -L target/release \
 *       -Wl,-rpath,target/release \
 *       crates/nirs4all-io-capi/examples/probe_version.c \
 *       -lnirs4all_io_capi -o probe_version
 *   ./probe_version
 *
 * Expected output:
 *
 *   nirs4all-io C ABI version: 0.1.0
 *   core_is_available: 1
 */

#include <stdio.h>
#include <stdlib.h>

#include "nirs4all_io.h"

int main(void) {
    char *version = n4io_abi_version();
    if (version == NULL) {
        fprintf(stderr, "n4io_abi_version returned NULL\n");
        return 1;
    }
    int available = n4io_core_is_available();
    printf("nirs4all-io C ABI version: %s\n", version);
    printf("core_is_available: %d\n", available);
    n4io_string_free(version);
    return available ? 0 : 1;
}
