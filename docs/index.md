# nirs4all-formats

Rust-first, low-level readers for **NIRS and spectroscopy file formats**, with
matching Python / R / WebAssembly / C bindings and conformance checks against
reference loaders.

nirs4all-formats auto-detects each file by content, decodes it through a single Rust
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
:caption: Reference
:maxdepth: 2

DATA_MODEL
OUTPUTS
CLI
CONFORMANCE
INTEGRATION_NIRS4ALL
RELEASE
VERSIONING
SECURITY
LICENSE_MATRIX
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
formats/openspecy
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

## The nirs4all ecosystem

<!-- RTD slugs are assumed equal to the repo name; edit a :link: URL below if a slug differs at import. -->

::::{grid} 1 2 2 2
:gutter: 2

:::{grid-item-card} nirs4all
:link: https://nirs4all.readthedocs.io/en/latest/
Main Python modelling library — pipelines, SpectroDataset, predictions.
:::
:::{grid-item-card} nirs4all-methods
:link: https://nirs4all-methods.readthedocs.io/en/latest/
Portable C-ABI PLS/NIRS engine (libn4m) + bindings.
:::
:::{grid-item-card} nirs4all-io
:link: https://nirs4all-io.readthedocs.io/en/latest/
Dataset-assembly bridge → SpectroDataset.
:::
:::{grid-item-card} nirs4all-datasets
:link: https://nirs4all-datasets.readthedocs.io/en/latest/
Curated DOI-pinned NIRS dataset catalog (n4a-datasets).
:::
:::{grid-item-card} nirs4all-lite
:link: https://nirs4all-lite.readthedocs.io/en/latest/
Portable aggregate distribution (Rust + bindings).
:::
:::{grid-item-card} dag-ml
:link: https://dag-ml.readthedocs.io/en/latest/
Reproducible, OOF/leakage-safe ML coordinator.
:::
:::{grid-item-card} dag-ml-data
:link: https://dag-ml-data.readthedocs.io/en/latest/
Typed sample-aligned multi-source data contracts.
:::
::::
