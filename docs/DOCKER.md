# Docker

The `nirs4all-formats` image runs the [command-line tool](CLI.md) on Linux
(`linux/amd64`, `linux/arm64`). It contains the binary and the licenses on
`debian:bookworm-slim`, runs as the unprivileged user `10001` and uses `/data`
as its working directory. No sample or user data is included.

## Image

Once published by the release workflow:

```bash
docker pull ghcr.io/gbeurier/nirs4all-formats:0.2.20   # or :latest
```

Built from a checkout:

```bash
docker build -f docker/Dockerfile -t nirs4all-formats .
```

## Usage

Mount the directory holding your files at `/data`. Pass your user and group so
the produced files belong to you and the container can write to the directory:

```bash
mkdir -p out
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/data" ghcr.io/gbeurier/nirs4all-formats \
    convert /data/spectrum.spc --to jcamp-dx --out /data/out
```

Other commands:

```bash
docker run --rm -v "$PWD:/data:ro" ghcr.io/gbeurier/nirs4all-formats probe /data/spectrum.spc
docker run --rm -v "$PWD:/data:ro" ghcr.io/gbeurier/nirs4all-formats read-json /data/spectrum.spc
docker run --rm ghcr.io/gbeurier/nirs4all-formats targets
docker run --rm -v "$PWD:/data:ro" ghcr.io/gbeurier/nirs4all-formats scan /data --json
```

The output directory must exist, and existing files are kept unless
`--overwrite` is passed (see [Exports](EXPORTS.md)).

## Publishing

`.github/workflows/release-docker.yml` builds both platforms on every
`v<version>` tag (the build stage cross-compiles, so no compiler runs under
emulation) and pushes `ghcr.io/gbeurier/nirs4all-formats:<version>` and
`:latest` (pre-release versions get only `<version>`). It uses the workflow
`GITHUB_TOKEN` with `packages: write`; no secret is needed. The first push
creates a private package: make it public once under the package settings
(Packages, `nirs4all-formats`, Package settings, Change visibility). The
image's `org.opencontainers.image.source` label links it to this repository.
