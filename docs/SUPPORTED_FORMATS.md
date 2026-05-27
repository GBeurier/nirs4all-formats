# Supported formats

nirs4all-formats reads NIRS and spectroscopy files through a registry of native Rust
readers. You never pick a reader by hand: `open_path` / `open_bytes` sniff the
file by content (magic bytes, container schema or text shape) and route it to
the best match. This page is the public catalogue of what is covered today.

For the internal, variant-by-variant tracking (including counts and the exact
files still being sourced), see [`FORMAT_MATRIX.md`](FORMAT_MATRIX.md) and
[`IMPLEMENTATION_DASHBOARD.md`](IMPLEMENTATION_DASHBOARD.md). Each format below
links to a detailed page.

## Status legend

| Status | Meaning |
|---|---|
| **Supported** | Main active variants are validated with samples, tests and docs — safe to rely on. |
| **Supported (scoped)** | Reliable within a documented subset; some variants are still pending. |
| **Partial** | Reads real files but is knowingly incomplete — read the page's *Limitations* before relying on it. |
| **Experimental** | A narrow or local-only path that may still change. |
| **Detected / refused** | Recognised, but intentionally **not** decoded (out of NIRS scope) — the reader points you to the right tool. |
| **Blocked / Planned** | Not readable yet; we need sample files or a specification. [Send files →](https://github.com/GBeurier/nirs4all-formats/issues/new/choose) |

Feature flags in brackets (`fmt-hdf5`, `fmt-matlab`, `fmt-parquet`) mark readers
that are compiled in by default but can be turned off (and are off on the
`wasm32` target unless noted).

## Generic & tabular

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| Delimited spectral tables | Generic / instrument exports | `.csv`, `.tsv`, `.txt` | Supported | [text-readers-001](formats/text-readers-001.md) |
| Row-oriented spectral tables | Generic / instrument exports | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | Supported | [row-spectral-table](formats/row-spectral-table.md) |
| Spectral matrix exports | Generic / Foss / Metrohm / VIAVI | `.csv`, `.txt` | Supported | [spectral-matrix](formats/spectral-matrix.md) |
| Excel workbooks | Generic / lab | `.xlsx`, `.xlsm` | Supported (scoped) | [excel](formats/excel.md) |
| NumPy | Python / NumPy | `.npy`, `.npz` | Supported | [numpy](formats/numpy.md) |
| Parquet | Apache / generic | `.parquet` | Supported (`fmt-parquet`) | [parquet](formats/parquet.md) |
| Consumer Physics SCiO | Consumer Physics | `.csv` | Supported | [scio-csv](formats/scio-csv.md) |

## FT-NIR / FT-IR / UV-Vis vendors

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| Bruker OPUS | Bruker | `.0`, `.1`, `.001`, `.0000`, `.dpt` | Supported (scoped) | [bruker-opus](formats/bruker-opus.md) |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs` | Supported (scoped) | [nicolet-omnic](formats/nicolet-omnic.md) |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc` | Supported (scoped) | [galactic-spc](formats/galactic-spc.md) |
| PerkinElmer Spectrum / IR | PerkinElmer | `.sp` | Supported (scoped) | [perkin-elmer](formats/perkin-elmer.md) |
| BUCHI NIRCal | BUCHI / Bühler | `.nir` | Supported (scoped) | [buchi-nircal](formats/buchi-nircal.md) |
| JASCO JWS | JASCO | `.jws` | Supported (scoped) | [jasco-jws](formats/jasco-jws.md) |
| Ocean Optics / Ocean Insight | Ocean Optics | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec` | Supported (scoped) | [ocean-optics](formats/ocean-optics.md) |
| Foss / WinISI exports | Foss | `.txt`, `.csv` | Supported (scoped) | [foss-winisi](formats/foss-winisi.md) |
| Metrohm Vision / Vision Air export | Metrohm | `.csv` | Supported (scoped) | [metrohm-vision](formats/metrohm-vision.md) |
| VIAVI MicroNIR export | VIAVI / JDSU | `.csv`, `.xlsx` | Supported (scoped) | [viavi-micronir](formats/viavi-micronir.md) |
| Shimadzu UVProbe export | Shimadzu | `.txt` | Supported (scoped) | [shimadzu-uvprobe](formats/shimadzu-uvprobe.md) |
| Si-Ware NeoSpectra export | Si-Ware | `.csv`, `.xlsx` | Supported (scoped) | [siware-neospectra](formats/siware-neospectra.md) |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | Partial | [siware-api](formats/siware-api.md) |

## Field spectroscopy

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | Supported (scoped) | [asd](formats/asd.md) |
| Spectral Evolution SED | Spectral Evolution | `.sed` | Supported (scoped) | [spectral-evolution-sed](formats/spectral-evolution-sed.md) |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | Supported (scoped) | [svc-ger-sig](formats/svc-ger-sig.md) |
| USGS / ECOSTRESS spectral text | USGS / JHU / ECOSTRESS | `.asc`, `.txt`, `.spectrum.txt` | Supported (scoped) | [usgs-speclib](formats/usgs-speclib.md) |
| Avantes AvaSoft (ASCII + binary) | Avantes | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.TRM`, `.ROH`, `.DRK`, `.REF`, `.Raw8`, `.IRR8` | Supported (scoped) | [avantes](formats/avantes.md) |
| PP Systems UniSpec SC / DC | PP Systems | `.SPT`, `.SPU` | Experimental | [pp-systems-unispec](formats/pp-systems-unispec.md) |
| Felix Instruments F-750 | Felix Instruments / CID Bio-Science | `.csv` | Supported (scoped) | [felix-f750](formats/felix-f750.md) |

## Hyperspectral cubes

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr` | Supported | [envi-sli](formats/envi-sli.md) |
| ENVI Standard cube | ENVI | `.img` / `.dat` + `.hdr` | Supported (scoped) | [envi-sli](formats/envi-sli.md) |
| AVIRIS / ERDAS LAN | AVIRIS / ERDAS | `.lan` + `.spc` + `.GIS` | Experimental | [erdas-lan](formats/erdas-lan.md) |

Cube readers expose pixel selection (rectangular ROI window or an ordered sparse
`(row, col)` mask) so you don't have to materialise an entire scene.

## Sun photometers

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| MFR / MFRSR | Solar Light / YES Inc. | `.OUT`, `.nc` | Supported (scoped) | [sun-photometers](formats/sun-photometers.md) |
| Microtops / MAN | Solar Light | `.TXT`, `.nc`, `.lev10/.lev15/.lev20` | Supported (scoped) | [sun-photometers](formats/sun-photometers.md) |

## HDF5, containers & standards

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| Generic HDF5 NIRS | Vendor-neutral | `.h5`, `.hdf5` | Supported (scoped, `fmt-hdf5`) | [hdf5](formats/hdf5.md) |
| Generic NetCDF NIRS | Vendor-neutral | `.nc`, `.cdf` | Supported (scoped, `fmt-hdf5`) | [netcdf](formats/netcdf.md) |
| FGI HDF5 + XML | FGI | `.h5` / `.hdf5` + `.xml` | Experimental (`fmt-hdf5`) | [fgi-hdf5-xml](formats/fgi-hdf5-xml.md) |
| MATLAB MAT / RData | MATLAB / R ecosystem | `.mat`, `.RData` | Supported (scoped, `fmt-matlab`) | [matlab](formats/matlab.md) |
| Allotrope ASM | Allotrope / Benchling | `.json` | Supported (scoped) | [allotrope-asm](formats/allotrope-asm.md) |
| Allotrope ADF | Allotrope Foundation | `.adf` | Experimental (`fmt-hdf5`) | [allotrope-adf](formats/allotrope-adf.md) |
| AnIML | IUPAC / ASTM | `.animl` | Experimental | [animl](formats/animl.md) |

## Exchange & standards-track

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| JCAMP-DX | IUPAC (vendor-neutral) | `.jdx`, `.dx`, `.jcm`, `.jcamp` | Supported (scoped) | [jcamp-dx](formats/jcamp-dx.md) |
| EMSA/MAS MSA | ISO 22029 | `.msa` | Supported (scoped) | [msa-iso22029](formats/msa-iso22029.md) |

## Raman & adjacent (decoded, adjacent to core NIRS)

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| Renishaw WDF | Renishaw | `.wdf` | Supported (scoped) | [renishaw-wdf](formats/renishaw-wdf.md) |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6m` | Partial | [horiba-labspec](formats/horiba-labspec.md) |
| WiTec WIP / WID | WiTec | `.wip`, `.wid` | Partial | [witec-wip](formats/witec-wip.md) |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | Supported | [trivista-tvf](formats/trivista-tvf.md) |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | Supported | [digitalsurf](formats/digitalsurf.md) |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | Experimental (adjacent) | [hamamatsu-img](formats/hamamatsu-img.md) |

## Detected & deliberately refused (not NIRS)

These are recognised so the walker can label them, but they are **not** decoded
into spectra — the reader points to the appropriate mass-spectrometry tool.

| Format | Vendor | Extensions | Status | Page |
|---|---|---|---|---|
| mzML | HUPO PSI | `.mzML`, `.mzMLb` | Detected / refused | [mzml](formats/mzml.md) |
| ANDI / NetCDF MS | ASTM | `.cdf`, `.nc` | Detected / refused | [andi-ms](formats/andi-ms.md) |

## Not yet supported — help us by sending samples

These formats are in scope but blocked until we can get real files or a usable
specification. If you can share any of them, you will directly unblock a reader.

| Format | Vendor | Extensions | Status |
|---|---|---|---|
| Foss NIRSystems / WinISI native | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | Blocked — native binary |
| Perten DA / Inframatic native | Perten / PerkinElmer | vendor binary | Blocked — no spectral sample |
| Bruker Tango / Matrix native | Bruker | OPUS | Planned — dedicated fixtures wanted |
| ASD calibration companions | ASD / Malvern | `.ILL`, `.REF`, `.RAW` | Blocked — companion files |
| fNIRS neuroscience | NIRx / SNIRF | `.snirf`, `.nirs` | Out of scope (physiology, not spectroscopy) |

## Don't see your format?

- **Request a format** or **send reference files** through the
  [issue templates](https://github.com/GBeurier/nirs4all-formats/issues/new/choose) —
  real files are the single biggest unblocker.
- If a supported file is **misread or refused**, open a *bug* issue with the
  output of `nirs4all-formats probe <file>`.

See [`CONTRIBUTING.md`](https://github.com/GBeurier/nirs4all-formats/blob/main/CONTRIBUTING.md)
for how sample files are licensed and added to the corpus.
