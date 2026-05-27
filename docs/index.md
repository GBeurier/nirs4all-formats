# nirs4all-io

Rust-first, low-level readers for **NIRS and spectroscopy file formats**, with
stable Python / R / WebAssembly / C bindings and conformance checks against
reference loaders.

nirs4all-io auto-detects each file by content, decodes it through a single Rust
registry, and returns a canonical, provenance-tracked `SpectralRecord` model. It
does no modelling itself — it produces the records that
[`nirs4all`](https://github.com/GBeurier/nirs4all) consumes.

**Start here:** [Getting started](getting_started.md) ·
[Installation](installation.md) · [Usage guide](usage.md) ·
[Supported formats](SUPPORTED_FORMATS.md)

```{toctree}
:caption: Getting started
:maxdepth: 2

getting_started
installation
usage
SUPPORTED_FORMATS
```

```{toctree}
:caption: Bindings
:maxdepth: 2

bindings/python
bindings/r
bindings/wasm
bindings/capi
```

```{toctree}
:caption: Format reference
:maxdepth: 1

formats/allotrope-adf
formats/allotrope-asm
formats/andi-ms
formats/animl
formats/asd
formats/avantes
formats/bruker-opus
formats/buchi-nircal
formats/digitalsurf
formats/envi-sli
formats/erdas-lan
formats/excel
formats/felix-f750
formats/fgi-hdf5-xml
formats/foss-winisi
formats/galactic-spc
formats/hamamatsu-img
formats/hdf5
formats/horiba-labspec
formats/jasco-jws
formats/jcamp-dx
formats/matlab
formats/metrohm-vision
formats/msa-iso22029
formats/mzml
formats/netcdf
formats/nicolet-omnic
formats/numpy
formats/ocean-optics
formats/parquet
formats/perkin-elmer
formats/pp-systems-unispec
formats/renishaw-wdf
formats/row-spectral-table
formats/scio-csv
formats/shimadzu-uvprobe
formats/siware-api
formats/siware-neospectra
formats/spectral-evolution-sed
formats/spectral-matrix
formats/sun-photometers
formats/svc-ger-sig
formats/text-readers-001
formats/trivista-tvf
formats/usgs-speclib
formats/viavi-micronir
formats/witec-wip
```

```{toctree}
:caption: Reference
:maxdepth: 1

DATA_MODEL
CLI
CONFORMANCE
RELEASE
VERSIONING
SECURITY
INTEGRATION_NIRS4ALL
LICENSE_MATRIX
```

```{toctree}
:caption: Project & internals
:maxdepth: 1

PLAN
DIRECTIONS
ROADMAP
STATUS
FORMATS
FORMAT_MATRIX
IMPLEMENTATION_DASHBOARD
FORMAT_GAPS
MISSING_SAMPLES
Formats_extract
OUTPUTS
REVERSE_ENGINEERING
FIXTURE_GOVERNANCE
CI_MATRIX
```
