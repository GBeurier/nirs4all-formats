# syntax=docker/dockerfile:1
#
# nirs4all-formats command-line image. Build from the repository root:
#
#   docker build -f docker/Dockerfile -t nirs4all-formats .
#   docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" nirs4all-formats \
#       convert /data/spectrum.spc --to jcamp-dx --out /data/out
#
# No sample or user data is baked in; mount the files to read. See docs/DOCKER.md.

# The build stage runs on the build machine and cross-compiles for the target
# platform, so multi-platform builds do not emulate the Rust compiler.
FROM --platform=$BUILDPLATFORM rust:1-slim-bookworm AS build
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-gnu > /rust-target; gcc=gcc-x86-64-linux-gnu; libc=libc6-dev-amd64-cross ;; \
      arm64) echo aarch64-unknown-linux-gnu > /rust-target; gcc=gcc-aarch64-linux-gnu; libc=libc6-dev-arm64-cross ;; \
      *) echo "unsupported TARGETARCH $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && apt-get update \
    && apt-get install -y --no-install-recommends "$gcc" "$libc" \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add "$(cat /rust-target)"
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar \
    CARGO_PROFILE_RELEASE_STRIP=symbols
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY vendor vendor
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target,id=nirs4all-formats-target-$TARGETARCH \
    target="$(cat /rust-target)" \
    && cargo build --release --locked --target "$target" -p nirs4all-formats-cli \
    && install -Dm755 "target/$target/release/nirs4all-formats" /out/nirs4all-formats

FROM debian:bookworm-slim
ARG VERSION=dev
LABEL org.opencontainers.image.title="nirs4all-formats" \
      org.opencontainers.image.description="Read NIRS and spectroscopy vendor files and convert them to exchange formats." \
      org.opencontainers.image.source="https://github.com/GBeurier/nirs4all-formats" \
      org.opencontainers.image.url="https://github.com/GBeurier/nirs4all-formats" \
      org.opencontainers.image.licenses="CECILL-2.1 OR AGPL-3.0-or-later" \
      org.opencontainers.image.version="${VERSION}"
COPY --from=build /out/nirs4all-formats /usr/local/bin/nirs4all-formats
COPY LICENSE LICENSING.md /usr/share/licenses/nirs4all-formats/
COPY LICENSES /usr/share/licenses/nirs4all-formats/LICENSES
RUN useradd --uid 10001 --user-group --no-create-home --home-dir /data --shell /usr/sbin/nologin nirs4all
USER 10001:10001
WORKDIR /data
ENTRYPOINT ["nirs4all-formats"]
CMD ["--help"]
