# BinaryBuilder recipe for nirs4all_formats_capi_jll, the C ABI of nirs4all-formats.
#
# Copy this directory to N/nirs4all_formats_capi/ in a fork of
# https://github.com/JuliaPackaging/Yggdrasil and open a pull request; the
# Yggdrasil CI builds every platform and registers the JLL in General. For a new
# release, update `version` and the commit hash of the matching v<version> tag.
# See docs/bindings/julia.md in nirs4all-formats.

using BinaryBuilder, Pkg

name = "nirs4all_formats_capi"
version = v"0.2.20"

sources = [
    # Commit of the v0.2.20 tag of https://github.com/GBeurier/nirs4all-formats.
    GitSource("https://github.com/GBeurier/nirs4all-formats.git",
              "0000000000000000000000000000000000000000"),
]

script = raw"""
cd ${WORKSPACE}/srcdir/nirs4all-formats
cargo build --release --locked -p nirs4all-formats-capi
if [[ "${target}" == *-mingw* ]]; then
    install -Dvm 755 "target/${rust_target}/release/nirs4all_formats_capi.${dlext}" "${libdir}/nirs4all_formats_capi.${dlext}"
else
    install -Dvm 755 "target/${rust_target}/release/libnirs4all_formats_capi.${dlext}" "${libdir}/libnirs4all_formats_capi.${dlext}"
fi
install -Dvm 644 crates/nirs4all-formats-capi/include/nirs4all_formats.h "${includedir}/nirs4all_formats.h"
install_license LICENSE LICENSING.md LICENSES/CeCILL-2.1.txt LICENSES/AGPL-3.0-or-later.txt
"""

# The platforms the Java and .NET packages ship; extend after checking the
# Yggdrasil CI (musl, FreeBSD, 32-bit and RISC-V are not tested upstream).
platforms = [
    Platform("x86_64", "linux"; libc="glibc"),
    Platform("aarch64", "linux"; libc="glibc"),
    Platform("x86_64", "macos"),
    Platform("aarch64", "macos"),
    Platform("x86_64", "windows"),
]

products = [
    LibraryProduct(["libnirs4all_formats_capi", "nirs4all_formats_capi"], :libnirs4all_formats_capi),
]

dependencies = Dependency[]

# The crates declare rust-version 1.88.
build_tarballs(ARGS, name, version, sources, script, platforms, products, dependencies;
               compilers=[:c, :rust], julia_compat="1.6", preferred_rust_version=v"1.88")
