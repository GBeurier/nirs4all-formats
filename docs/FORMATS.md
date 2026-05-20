# NIRS / Spectroscopy File Format Inventory

Inventory of file formats that `nirs4all-io` aims to ingest, with the current
state of open-source support and the parser strategy we plan to use. Sections
1–3 capture the formats provided in the original spec; sections 4–6 add formats
discovered during the web research phase (cf. `DIRECTIONS.md`, Annex A).

Formats that are listed but not decoded yet because of missing samples, unknown
binary layouts or deliberate scope decisions are tracked in
[`FORMAT_GAPS.md`](FORMAT_GAPS.md).

## Current Native Coverage

Experimental readers currently implemented in Rust and tested on committed
fixtures:

| Format family | Extensions / fixture class | Status | Notes |
|---|---|---|---|
| Plain delimited spectral tables | `.csv`, `.tsv`, numeric-header `.txt` | Experimental | One record per row, numeric header columns become the spectral axis, numeric non-spectral columns become targets. Covers real AuroraNIR, Foss XDS and OSSL NeoSpectra wide CSV fixtures. |
| Row-oriented spectral tables | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` with axis-first content | Experimental | One record per file, first numeric column is the spectral axis, following numeric columns become named signals. Covers committed Si-Ware, MODTRAN, PP Systems, ENVI/ECOSTRESS/IDL text, JASCO text export, Shimadzu text, USGS SPECPR ASCII and WiTec ASCII fixtures. |
| USGS AREF single-column text | `.txt`, `.asc` with `Record=... AREF` title | Experimental partial | Reflectance-only dumps without embedded wavelengths are loaded with a generated index axis and provenance warning. |
| Spectral matrix exports | `.csv`, `.txt` with one spectrum per sample row | Experimental | Numeric spectral headers or a `Wavelengths:` block become the axis. Covers committed Foss/WinISI text, Metrohm Vision Air CSV and VIAVI MicroNIR CSV fixtures. Target-only reports are detected as unsupported. |
| Sun photometer text / MAN exports | `.OUT`, `.TXT`, `.nc`, local `.lev10/.lev15/.lev20` | Experimental | MFR and Microtops channel columns become short wavelength axes, one observation per record; local ARM MFRSR b1 NetCDF emits 7-filter irradiance/voltage/ratio signals; the committed PANGAEA MAN NetCDF fixture emits AOT and AOT-STD signals; local AERONET MAN ASCII exports emit valid AOD channels and optional AOD-STD. |
| AnIML spectral XML | `.animl` | Experimental | Spectral `SeriesSet` documents with wavelength/wavenumber axis series and same-length signal series. Explicit numeric values and uniform `AutoIncrementedValueSet` axes are covered; non-spectral AnIML result documents are refused. |
| Allotrope ASM plate-reader JSON | `.json` with `$asm.manifest` | Experimental | Plate-reader ASM spectral data cubes and detector-wavelength endpoint readings. Covers committed Benchling allotropy fluorescence/absorbance ASM fixtures. |
| SiWare API JSON | `.json` with `measurement.wavelengths` / `measurement.absorbance` | Experimental | One-measurement NeoSpectra-style JSON payloads; predictions become targets. |
| Consumer Physics SCiO CSV | `.csv` | Experimental | Developer-app `band*` exports and grouped `spectrum_*` / `wr_raw_*` / `sample_raw_*` exports at 740-1070 nm. |
| NetCDF NIRS datasets | `.nc`, `.cdf` with `spectra` + `wavelengths` variables or known sun-photometer/channel schemas | Experimental | Pure-Rust NetCDF reader for simple sample-by-wavelength datasets, Microtops MAN, local ARM MFRSR b1 and local ARM SURFSPECALB derived albedo; ANDI/MS gets a dedicated non-NIRS refusal path and other adjacent NetCDF files are schema-refused. |
| Parquet NIRS tables | `.parquet` | Experimental | Arrow-backed reader for canonical NIRS tables whose spectral columns are numeric wavelength names; generic non-spectral Parquet files are schema-refused. |
| NumPy datasets | `.npy`, `.npz` | Experimental | NPY matrix reader with generated index axis and NPZ canonical `X`/`wavelengths`/`y`/`sample_ids` reader for ML datasets. |
| Bruker OPUS DPT export | `.dpt` | Experimental | Two-column ASCII, wavenumber axis in `cm-1`. |
| Bruker OPUS native | numeric extensions such as `.0`, `.1`, `.001`, `.0000` | Experimental | New OPUS magic, directory, parameter blocks and 1D data/status block pairs. Multi-signal files expose absorbance, reflectance, sample/reference spectra, interferograms and phase when present. |
| Avantes AvaSoft ASCII | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.txt` | Experimental | Wave tables, AvaSoft 8 text exports and two-column irradiance export. |
| Avantes AvaSoft binary | `.TRM`, `.ROH`, `.DRK`, `.REF`, `.ABS`, `.Raw8`, `.IRR8` | Experimental | Legacy AvaSoft 7 float32 headers and AVS82/AVS84 subfiles. `.ABS` and additional AVS8 modes are implemented by layout but still need fixtures. |
| ENVI Spectral Library / Standard cubes | `.sli` + `.hdr`, `.img`/`.dat` + `.hdr` | Experimental | Paired sidecar reader for `file type = ENVI Spectral Library`, one-band BSQ float32/float64 payloads, plus ENVI Standard BSQ/BIL/BIP cube expansion to one point spectrum per pixel. |
| ERDAS LAN / AVIRIS 92AV3C | `.lan` + `.spc` + optional `.GIS` | Experimental partial | Classic Indian Pines AVIRIS cube, 145 x 145 x 220 u16 BIL payload, expanded to one raw-count spectrum per pixel with ground-truth class labels when present. |
| Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.spc` | Experimental | SpectraSuite, OceanView, Jaz, CRAIC and two-column CSV text exports, plus OceanView `.ProcSpec` ZIP/XML archives with SHA-512 signature validation. The committed Ocean Optics `.spc` fixture is a Galactic/Thermo new-LSB explicit-X SPC file and is routed through that reader. |
| JCAMP-DX | `.jdx`, `.dx`, `.jcm` | Experimental partial | Single-block `XYDATA=(X++(Y..Y))` with plain AFFN plus PAC/SQZ/DIF/DUP ASDF ordinate decoding, NMR `NTUPLES` real/imaginary pages, and Ocean Optics `LINK`/`XYPOINTS` sample-dark-reference blocks. `PEAK TABLE` is explicitly refused until the shared model can represent sparse peak lists; broader `LINK` variants are pending. |
| EMSA/MAS MSA (ISO 22029) | `.msa` | Experimental | Standards-track single-spectrum text format. `XY` and `Y` payloads are supported with axis reconstruction from `OFFSET`, `XPERCHAN` and `CHOFFSET`. Reference reader: `rsciio.msa`. |
| Spectral Evolution SED | `.sed` | Experimental | Header key/value metadata plus wavelength and signal columns; DN-only files are flagged when no reflectance column exists. |
| SVC/GER SIG | `.sig` | Experimental | Spectra Vista SIG text fixtures with reference, target and reflectance channels; declared bad fixtures are flagged for validation reports. |
| ASD FieldSpec | `.asd`, ASD binary files with numeric extensions | Experimental | Revisions 1/6/7/8: fixed header, wavelength axis and primary spectrum. Reference/calibration/dependent blocks are not decoded yet. |
| Thermo / Galactic GRAMS SPC | `.spc`, `.SPC` | Experimental | New little-endian `0x4B` headers with generated X, explicit X, multi common-X, and `-XYXY` directory layouts; old little-endian `0x4D` is limited. New big-endian `0x4C` is detected but not decoded. |
| Thermo Nicolet OMNIC | `.SPA`, `.spg`, `.srs` | Experimental | Reverse-engineered key-table reader for single-spectrum `.SPA` and grouped `.SPG` files, plus TGA/GC `.srs` time series as 2D `y,x` records. |
| Perkin Elmer Spectrum / IR | `.sp` | Experimental | `PEPE` block reader for single-spectrum `.sp` files; `.fsm` imaging is detected but out of scope for v1. |
| BUCHI NIRCal | `.nir` | Experimental | `NIRCAL Project File` section reader for the committed 20-sample foliar-transfer fixture and a local 105-spectrum cannabis transfer fixture; property names are mapped to targets, with committed zero-valued properties emitted as nulls and local non-null `CBDA`/`THCA` targets preserved. |
| JASCO JWS | `.jws` | Experimental | OLE2 `DataInfo` + `Y-Data` reader for committed FT/IR transmittance, fluorescence and CD/HT/Abs multi-channel fixtures, with semantic labels inferred from JASCO metadata when available. |
| Horiba LabSpec / JobinYvon | `.xml`, `.txt` LabSpec exports | Experimental | LSX XML single spectra, range exports, linescans and maps plus LabSpec two-column, series-row and map-row text exports. Energy axes currently fall back to `Index` with a warning. |
| WiTec WIP / WID | `.wip`, `.wid`, `.txt` | Experimental partial | WiTec ASCII exports load through `row-spectral-table`; `WIT_PR06` TDGraph maps matching `Sa4.wip` decode to raw-count spectra with a wavelength axis. Legacy `WIT^` and unknown project layouts are refused explicitly. |
| Renishaw WDF | `.wdf` | Experimental | `WDF1` chunk reader for spectral payloads via `DATA`, `XLST` and `YLST`; `ORGN`/`WMAP` navigation metadata adds spatial X/Y/Z, FocusTrack Z, elapsed time, map dimensions and map indices. `WHTL` JPEG image metadata and `MAP ` PSET analysis-block inventory are exposed conservatively. |
| Princeton TriVista TVF | `.tvf` | Experimental | XML `Frame` payload reader for committed single spectra, multi-frame spectra, line scans, maps, time series and Step-and-Glue acquisitions. `InfoSerialized` X/Y axes become spatial metadata; Step-and-Glue emits the glued primary plus child windows. |
| DigitalSurf MountainsMap | `.sur`, `.pro` | Experimental | Fixed-header object reader for committed spectra, multi-spectrum profiles, hyperspectral maps and surface profiles, including `DSCOMPRESSED` zlib stream payloads. |
| Hamamatsu HPD-TA streak camera | `.img` | Experimental adjacent | `IM` header reader for committed focus, operate, photon-counting, shading and uncalibrated-X fixtures. Emits one 2D `y,x` raw-count signal with the secondary time/CCD axis in metadata. |

Promotion from Experimental to Beta/Done requires golden JSON conformance,
reference-loader comparison where available and adversarial tests.

Legend for **Open status**:

| Symbol | Meaning |
|--------|---------|
| ✅ | At least one production-quality open-source reader exists |
| 🟡 | Open reader exists but is partial / unmaintained / vendor-fragile |
| 🟠 | Only ASCII export is reliably loadable; binary is reverse-engineered |
| 🔴 | No open reader; vendor SDK required or full reverse engineering needed |

Legend for **Container**:

| Symbol | Meaning |
|--------|---------|
| `bin` | Proprietary binary (custom struct) |
| `ascii` | Text format with documented layout |
| `mixed` | Text wrapper around binary blocks |
| `tabular` | Spreadsheet container (`.xlsx` = ZIP/XML, `.xls` = OLE/CFB) |
| `hdf5` / `nc` / `xml` | Standardized container with vendor schema |

> **Disambiguation.** "NIRS" in this document means **near-infrared
> *molecular spectroscopy*** (instrument spectra). The acronym is also used
> for **functional NIRS (fNIRS) physiological time series** in neuroscience
> (`SNIRF` `.snirf`, NIRx `.nirs`, `.wl1/.wl2`, `.hdr`). Those are explicitly
> out of scope; users should look at [MNE-NIRS](https://github.com/mne-tools/mne-nirs)
> or the [SNIRF specification](https://github.com/fNIRS/snirf).

---

## 1. Field / portable spectroradiometers (user-supplied list)

| Vendor / instrument | Extensions | Container | Open status | Reference readers | Notes |
|---|---|---|---|---|---|
| ASD / Malvern Panalytical FieldSpec (Pro, FS3, FS4, HandHeld) | `.asd` (multiple binary revisions) | bin | ✅ | R: `asdreader`, `prospectr::readASD()`, `spectrolab::read_spectra(format="asd")`; Py: `specdal`, `pyASDReader` | Best-covered field format. Reverse-engineered headers expose DN / white reference / radiance / reflectance + GPS + timestamps. Revision flag must be parsed before deciding payload offsets. |
| ASD calibration files | `.ILL`, `.REF`, `.RAW` | bin | 🟡 | SPECCHIO; partial in `asdreader`/R | Companion calibration files of `.asd`. Often required for radiance → reflectance conversion. |
| Avantes AvaSoft 6/7 — legacy single-mode binaries | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | bin | 🟡 | R: [`lightr::lr_parse_avantes_trm()`](https://docs.ropensci.org/lightr/reference/lr_parse_avantes_trm.html); SPECCHIO (untested) | Binary one-spectrum-per-mode files. Apogee is *not* the same family despite a similar extension — leave Apogee out until we have fixtures. |
| Bruker FTIR / OPUS export | `.dpt` | ascii | ✅ | Any text loader (`pandas`, `read.table()`) | Two-column ASCII export from OPUS. Trivial. |
| Bruker OPUS native | no fixed ext. (often `.0`, `.0000`, …) | bin | ✅ | R: `opusreader2`, `hyperSpec.utils::read_opus()`; Py: `brukeropusreader`, `brukeropus`, `opusFC`, `spectrochempy.read_opus()` | Proprietary, reverse-engineered. Block-based file with parameter blocks + spectral blocks. Several Python readers diverge in completeness. |
| ENVI Spectral Library | `.sli` + `.hdr` (`.slb` listed by references, not fixture-backed here) | mixed (hdr ascii + sli bin) | ✅ | R: `RStoolbox::readSLI()`; Py: `spectral` (Spectral Python), `pysptools` | Well documented. Reference for our internal representation: paired metadata + binary block. |
| FGI HDF5 + XML | `.h5`, `.hdf5`, `.xml` | hdf5 + xml | 🟡 | R: `rhdf5`, `hdf5r`, `xml2`; Py: `h5py`, `lxml` | Generic HDF5 payload and the committed synthetic XML sidecar are covered; real FGI schema coverage remains pending. |
| Excel spectral tables | `.xls`, `.xlsx` | tabular | ✅ | `readxl`, `openpyxl`, `pandas.read_excel()`; Rust: `calamine` | Current native reader covers `.xlsx/.xlsm` single-sheet spectral tables, first-cell axis/data descriptors and canonical multi-sheet `spectra`/`metadata`/`references` layouts with numeric wavelength headers; legacy `.xls` remains pending. |
| MFR Sun Photometer | `.OUT`, local `.nc` | ascii / nc | 🟠 | Ad-hoc parser; SPECCHIO; xarray; ARM ACT | Regular fixed-width text; committed MFR fixture is covered by `sun_photometer`. Local ARM MFRSR b1 NetCDF is decoded into 7-filter multispectral irradiance/voltage/ratio records with QC metadata. |
| Ocean Optics SpectraSuite | `.csv` (non-comma) | ascii | ✅ | R: `lightr`, `pavo::getspec()`; Py: ad-hoc | Variant CSV with `;` or tab separator + multi-line header. |
| Ocean Optics OceanView | `.txt`, `.ProcSpec`, `.spc` (Ocean Optics flavour, not Galactic) | mixed | 🟡 | R: [`lightr::lr_parse_procspec()`](https://www.rdocumentation.org/packages/lightr/versions/1.9.0/topics/lr_parse_procspec) | `.ProcSpec` is a proprietary container (XML + binary payload, optionally archived) with a checksum that `lightr` validates. Layout drifts across OceanView versions. |
| PP Systems UniSpec SC | `.SPT` | ascii | 🟠 | Ad-hoc | Synthetic axis-first text export is covered by `spectral_table` with semantic tests for raw DN and reflectance channels; limited literature; SPECCHIO claims support. Local Arctic LTER indices are derived products and do not close the raw `.SPT` sample gap. |
| PP Systems UniSpec DC | `.SPU` | ascii | 🟠 | Ad-hoc | Synthetic axis-first text export is covered by `spectral_table` with semantic tests for dual raw DN channels and reflectance. Local Arctic LTER indices are derived products and do not close the raw `.SPU` sample gap. |
| Microtops Sun Photometer | `.TXT`, `.lev10/.lev15/.lev20`, `.nc` | ascii / nc | 🟠 | Ad-hoc; xarray | Text with rich metadata; committed AOT CSV fixture, committed MAN NetCDF fixture and local AERONET MAN ASCII `.lev*` samples are covered by `sun_photometer`. |
| GER 3700 / SVC | `.sig` | ascii (with variants) | ✅ | R: `spectrolab`; Py: `specdal` | Two header conventions (PDA vs. laptop). |
| SVC HR-1024 / HR-1024i | `.sig` | ascii (with variants) | ✅ | Same as above | Date/GPS/units differ across firmware. |
| Spectral Evolution / PSR | `.sed` | ascii | ✅ | R: `spectrolab::read_spectra(format="sed")`; Py: `specdal` | Best-documented field spectrometer ASCII format. |
| MODTRAN5 albedo | `.dat` | ascii | ✅ | Text loader | Not really an instrument format; committed albedo fixture is covered by `spectral_table`. |
| IDL / ENVI text output | `.txt` (whitespace-sep) | ascii | ✅ | Text loader | Axis-first ECOSTRESS/ENVI text spectra are covered by `spectral_table`; single-column sidecar-axis dumps remain pending. |
| USGS SPECPR / PRISM | `SPECPR` (no ext.) | bin (historical) | 🟠 | USGS free SW; convert to ENVI/ASCII | Practical approach: shell out to USGS converter once, then ingest ASCII/ENVI. |

---

## 2. Benchtop / industrial / FT-NIR (extension of the user list)

| Vendor / instrument | Extensions | Container | Open status | Reference readers | Notes |
|---|---|---|---|---|---|
| Bruker OPUS — Tango / MPA / Matrix series | OPUS native | bin | ✅ | Same as OPUS above | Production NIR analyzers also write `.0` style OPUS files. |
| Thermo / Galactic GRAMS | `.spc` | bin | ✅ | Py: [`spc-spectra`](https://github.com/nick-macro/spc-spectra), [`rohanisaac/spc`](https://github.com/rohanisaac/spc), `specio`, `spectrochempy`; xylib (C++); JS: [`cheminfo/spc-parser`](https://cheminfo.github.io/spc-parser/) | De-facto interchange format. Multiple binary variants: **old vs. new** header, **LSB vs. MSB** byte order, and several data layouts (`-XY`, `-XYY`, `-XYXY`) for single-spectrum, common-X multi-spectrum, or independent-X multi-spectrum files. Test fixtures must cover every combination. |
| Thermo Nicolet OMNIC | `.spa`, `.spg`, `.srs`, `.srsx` | bin | 🟡 | Py: [`spectrochempy.read_omnic()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_omnic.html), `lerkoah/spa-on-python` (.spa only) | Current native reader covers `.spa` single spectra, `.spg` grouped spectra, TGA/GC `.srs` time series and local-only rapid-scan `.srs` raw/reprocessed fixtures. `.srsx` remains pending. |
| Perkin Elmer Spectrum / IR | `.sp`, `.fsm` | bin | 🟡 | Py: `specio` | Current native reader covers `PEPE` single-spectrum `.sp` block files with f64 ordinates. `.fsm` is an imaging variant we detect and refuse as out-of-scope for v1. |
| Foss NIRSystems / WinISI | `.NIR`, `.DA`, `.cal`, `.eqa` | bin | 🔴 | None reliable | **⚠ `.NIR` extension is shared with BUCHI NIRCal — never route by extension alone, always sniff the header signature.** The committed WinISI text matrix export is covered by `spectral_matrix`; native binary remains reverse-engineering work. |
| Foss DA1650 / DS2500 / DS3 | CSV/report exports + optional `.NIR` spectrum export | mixed (export) | 🟡 via export | Standard text loader; `.NIR` export needs reverse engineering | DS3 manual ([p. 45](https://www.manualslib.com/manual/2155011/Foss-Nirs-Ds3.html?page=45)) confirms CSV report + optional binary `.NIR`. WinISI/Foss XDS text matrix exports are covered by `spectral_matrix`/`csv_like`; target-only DS3 reports remain unsupported as spectra. |
| Metrohm NIRS XDS / DS2500 / Vision / Vision Air | CSV/Excel exports; native [Vision](https://www.metrohm.com/cs_cz/service/software-center/vision.html) project DB | mixed | 🟡 via export | Standard text loader | Synthetic Vision Air CSV spectral matrix export is covered by `spectral_matrix` with semantic tests for axis, absorbance and property targets. Native Vision project DB has no open reader; CSV/Excel exports are the practical path for v1. |
| Bruker Tango (FT-NIR) | OPUS native | bin | ✅ | Same as OPUS | Same loader as benchtop OPUS. |
| BUCHI NIRFlex / NIRMaster (NIRCal) | `.nir`, JCAMP-DX export | bin / ascii | 🟡 / ✅ via export | R: [`prospectr::read_nircal()`](https://rdrr.io/cran/prospectr/man/read_nircal.html) | Current native reader covers the committed `NIRCAL Project File` section layout for spectra, wavenumbers and property targets. The committed target values are all zero and are emitted as nulls, matching `prospectr`'s missing-value treatment; a local cannabis `.nir` validates non-null `CBDA`/`THCA` targets. Distinct from FOSS `.NIR`. |
| Perten DA / Inframatic | vendor proprietary, CSV export | bin / ascii | 🔴 / ✅ via export | Same strategy | Field-feed analyzer; committed CSV fixture is target-only and intentionally unsupported as a `SpectralRecord` because it has no spectral axis. |
| Avantes AvaSoft v8 (modern) | `.RAW8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.IRR8`, `.RIR8`, `.RMN8`, `.RMD8` | bin | 🟡 | R: `lightr` (subset); MATLAB community tools; **no maintained Python reader** | Each suffix encodes the measurement mode (scope, dark-corrected scope, absorbance, transmittance, reflectance, absolute/relative irradiance, Raman). [AvaSoft 8 manual](https://www.avantes.com/content/uploads/2022/02/020379-AvaSoft-8-Manual.pdf) is the reference. |
| Avantes AvaSoft ASCII exports | `.ttt`, `.trt`, `.tit`, `.tat` | ascii | ✅ | Any text loader (`pandas`) | Cheap fallback when the binary parser is missing. Worth supporting before the binaries. |
| JASCO V-series / FT-IR | `.jws`, `.txt` export | bin / ascii | 🟡 | Py: [`jws2txt`](https://pypi.org/project/jws2txt/), `jwsProcessor`; conversion via OMNIC | Reverse-engineered; mostly UV-Vis but used in NIR mode too. Text export is covered by `spectral_table`; current native `.jws` support covers OLE2 `DataInfo` + `Y-Data` fixtures with metadata-driven `transmittance`, `fluorescence`, `cd`, `ht` and `absorbance` labels. |
| Shimadzu UVProbe | `.spc` (Shimadzu proprietary, **not** Galactic), `.txt` export | bin / ascii | 🟠 | Experimental Py readers ([`pyfasma-spc`](https://pypi.org/project/pyfasma-spc/) note); vendor [converter](https://www.an.shimadzu.co.jp/products/molecular-spectroscopy/uv-vis/semicustom/uv-13/index.html) | Same `.spc` extension as Galactic but different binary format. Synthetic TXT export is covered by `spectral_table` and semantic tests; native `.spc` waits for a licensed fixture and reference comparison. |
| VIAVI MicroNIR (handheld) | CSV/XLSX export (`.csv`, `.xlsx`) | ascii/tabular | ✅ | Any CSV/Excel loader | Committed spectral matrix CSV export and real MicroNIR 1700 XLSX exports are covered. Native ".pri" project files remain out of scope without a public fixture/spec. |
| Si-Ware NeoSpectra | CSV/XLSX export | ascii/tabular | ✅ | Any CSV/Excel loader | Handheld MEMS spectrometer; committed axis-first CSV, OSSL wide CSV slice and UvA forensic XLSX export are covered by generic tabular readers. |
| Spectro Inc. SiWare API | JSON/CSV | ascii | ✅ | Standard JSON/CSV | Recent cloud-attached spectrometers; JSON is covered by `siware_api`; CSV stream is covered by `spectral_table`. |

---

## 3. Standardized / vendor-neutral formats

| Format | Extensions | Container | Open status | Reference readers | Notes |
|---|---|---|---|---|---|
| JCAMP-DX | `.jdx`, `.dx`, `.jcm`, `.jcamp` | ascii | ✅ (with caveats) | Py: [`jcamp`](https://pypi.org/project/jcamp/), [`spectrochempy.read_jcamp()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_jcamp.html), `nmrglue`; R: `ChemoSpec`, `hyperSpec` | [IUPAC standard](https://iupac.org/what-we-do/digital-standards/jcamp-dx/). The `.dx`/`.jdx` payload can use `AFFN`, `XYDATA`, `DIF/DUP`, or `NTUPLES` encoding — each existing reader covers a subset. Test fixtures must exercise every encoding. v1 priority. |
| ANDI / NetCDF | `.cdf`, `.nc` | nc | ✅ | Py: `pyteomics.openms.ANDIMS`, `PyMassSpec`, `pyOpenMS`; Rust: `netcdf-reader` | [ASTM E1947](https://store.astm.org/e1947-98r22.html) is the **chromatography-MS** ANDI standard, *not* a NIR/FTIR standard. Current native reader detects standard ANDI/MS variables and refuses these containers as non-NIRS. |
| AnIML | `.animl` (xml) | xml | 🟡 | Py: `animl-python` (early), Schema validators | IUPAC + ASTM XML standard. Current native reader covers spectral `SeriesSet` fixtures with explicit numeric values or uniform `AutoIncrementedValueSet` axes, and refuses non-spectral AnIML results. |
| Allotrope ASM | `.json` | json | 🟡 | [`Benchling-Open-Source/allotropy`](https://github.com/Benchling-Open-Source/allotropy) | JSON Simple Model. Current native reader covers plate-reader spectral data cubes and detector-wavelength endpoints after vendor-to-ASM conversion. |
| Allotrope ADF | `.adf` | hdf5 + triplestore | 🟡 | Allotrope Foundation SDK, `adfsee` | Experimental local-only reader for numeric `/data-cubes`; minimal RDF component mapping covers cube titles, seconds axes, secondary nm scales and absorbance units, while full ontology mapping and vendor ADFs remain unresolved. |
| mzML / mzMLb | `.mzML`, `.mzMLb` | xml / hdf5 | ✅ | `pyteomics`, `pymzml` | MS-oriented but cited as design inspiration for our internal schema. Current registry detects XML mzML and refuses it as non-NIRS with a pointer to MS-specific libraries; `.mzMLb` is documented but not fixture-backed yet. |
| Plain CSV / TSV / TXT | `.csv`, `.tsv`, `.txt` | ascii | ✅ | `pandas`, `nirs4all.data.loaders.CSVLoader` | Already supported in `nirs4all`. We extend it with header heuristics. |
| Parquet | `.parquet` | columnar bin | ✅ | `pyarrow`, `fastparquet`, `nirs4all.data.loaders.ParquetLoader` | Current native reader covers canonical NIRS tables with numeric wavelength columns and refuses generic Parquet tables. Used as the internal cache format in `nirs4all`. |
| HDF5 (generic) | `.h5`, `.hdf5` | hdf5 | ✅ | `h5py`, `tables`; Rust: `hdf5-reader` | Current native reader covers root or nested `spectra` + `wavelengths` datasets and refuses non-spectral HDF5 containers. |

---

## 4. Hyperspectral imaging (partial, listed for completeness)

These are explicitly not the primary target (we focus on point spectra), but
their formats reuse many of the same containers and several
hyperspectral-imaging users may want to extract pixel spectra:

- ENVI image cubes (`.dat`/`.img` + `.hdr`) — supported by Spectral Python.
- ENVI Spectral Library (`.sli`) — already in section 1.
- Legacy AVIRIS/Indian Pines ERDAS LAN (`.lan` + `.spc` + `.GIS`) — a
  sample-backed subset is loaded experimentally.
- EHU Indian Pines MATLAB v5 cube (`indian_pines_corrected.mat` plus optional
  `_gt.mat`) — supported only from `samples_local/` because redistribution
  terms are academic-use without a clear SPDX-compatible license.
- Specim / HySpex / Headwall raw cubes (often ENVI-compatible).
- BIL/BIP/BSQ raw with sidecar header.
- HDF5-based imaging (NEON AOP, AVIRIS-NG).

Current policy: accept small ENVI Standard, AVIRIS LAN and local-only MATLAB
cube fixtures by expanding each pixel to a point spectrum when enough axis or
band metadata is present, while keeping broader imaging workflows partial. A future
`extract_point_spectra(cube, mask)` helper is still needed for ROI/mask
workflows and for larger NEON, Specim, HySpex, Headwall and AVIRIS-NG payloads
where whole-cube expansion is not practical.

---

## 5. Adjacent useful formats

| Format | Why it matters | Decision |
|---|---|---|
| `.mat` (MATLAB) / `.RData` | Many academic NIR datasets are shared as MATLAB or R workspace files | Current native reader covers simple MAT v5 and v7.3 `X` + `wavelengths` + optional `y`, committed Eigenvector Corn, NIR Shootout 2002, SpectroChemPy DSO and ALS2004 structured MAT fixtures, prospectr `NIRsoil.RData`, and the local-only Indian Pines MATLAB v5 hyperspectral cube with optional ground-truth sidecar. |
| `.npy` / `.npz` | Common in ML workflows | Current native reader covers bare numeric NPY matrices and canonical NPZ `X`/`wavelengths`/`y`/`sample_ids` datasets. |
| `.xlsx` | Many lab transfers happen via Excel | Current native reader covers simple `.xlsx/.xlsm` spectral tables and canonical multi-sheet lab templates joined by `sample_id`. |
| Raman / UV-Vis "look-alikes" (Renishaw WDF, Horiba LabSpec, TriVista TVF, DigitalSurf, Hamamatsu IMG, WiTec WIP, JASCO) | Same `.spc`/`.jws` or spectral-map/time-resolved workflows, often confused with NIR | Horiba LabSpec XML/text and one LabSpec6 `.l6m` binary map, Renishaw WDF spectral payloads, TriVista TVF XML, DigitalSurf `.sur/.pro`, Hamamatsu `.img` and JASCO JWS now load experimentally; WiTec WIP/WID has a real `WIT_PR06` fixture and is being kept experimental. Detect, report instrument type, refuse only if explicitly NIRS-only mode is requested. |

---

## 6. Coverage summary (target for v1)

| Tier | Formats | Rationale |
|---|---|---|
| **A — must-have** | `.asd`, `.sig`, `.sed`, OPUS native, `.spc` (Galactic, all sub-variants), JCAMP-DX (`AFFN`/`XYDATA`/`DIF/DUP`/`NTUPLES`), ENVI SLI, Avantes AvaSoft 6/7 (`.TRM`/`.ABS`/`.ROH`/`.DRK`/`.REF`) and AvaSoft 8 (`.RAW8`/`.RFL8`/`.ABS8`/`.TRM8`/`.IRR8`), Avantes ASCII exports, CSV/TSV variants, Excel | Cover the majority of academic and industrial NIR field/benchtop deployments. |
| **B — high value** | `.spa`/`.spg`/`.srs` (Nicolet OMNIC), `.sp` (PE), Foss/Metrohm/Buchi CSV/JCAMP exports, BUCHI NIRCal `.nir` (via `prospectr` port), ASD `.ILL`/`.REF`/`.RAW`, OceanView `.ProcSpec`, JASCO `.jws`, Shimadzu UVProbe `.spc` | High-impact but partial open support; budget reverse-engineering or R-port work. UVProbe `.txt` export is already covered by the row-oriented table path; native Shimadzu `.spc` remains separate from Galactic/Thermo SPC. |
| **C — opportunistic** | FGI XML sidecars, Foss `.NIR` native, Perten native, AnIML hardening, Allotrope ASM/ADF hardening, USGS SPECPR | Either niche, vendor-locked, or covered by export workflows. |

> **Performance / availability assumption (important).** A *must-have* tag
> means the format must work in v1, *not* that we promise native speed for
> it. For Avantes binaries in particular, the only field-tested open reader
> is in R (`lightr`); the Python path is either (a) calling out to R via
> `rpy2` as an optional extra, (b) porting `lightr`'s parser, or (c) reading
> the ASCII exports first. v0.1 will document which option ships.

---

## 7. Cross-format normalization concerns

For every format above, the loader has to align the following axes before
emitting the unified record:

1. **Spectral axis** — `x`, `x_unit`, `x_kind`, `x_order`. FT-NIR / FTIR
   instruments natively store wavenumbers in cm⁻¹ (typically *decreasing*),
   dispersive instruments store wavelengths in nm (typically increasing).
   **Do not silently convert** to a canonical nm-monotonic axis — that would
   invert the order and break native sampling. Store the native axis with
   units and direction, and provide explicit `to_wavelength()` /
   `to_wavenumber()` conversions plus a `resample(grid)` helper.
2. **Signal channels** — `signals: dict[str, SpectralArray]`. A single file
   commonly stores several co-registered channels: raw counts (DN), dark
   reference, white reference, instrument-corrected radiance, reflectance,
   absorbance, single-beam, interferogram, background. We expose them as a
   named-channel dict with per-channel role + unit + signal type + provenance,
   rather than a single `intensities` array with a side `reference`.
3. **Signal type per channel** — absorbance, reflectance (`R`, `%R`),
   transmittance, log(1/R), Kubelka-Munk, derivative-already-applied,
   preprocessed. Enum defined in `nirs_loader` (and re-exported by `nirs4all`
   to avoid the dependency cycle).
4. **Reference properties / targets** — `targets: dict[str, Any]`. BUCHI
   NIRCal, FOSS / WinISI, Metrohm Vision, and most agro datasets carry lab
   reference values (protein, moisture, fat, ash, …) inside the spectrum
   file. These are *not* metadata — they are training labels and must be
   preserved in a typed, separate field.
5. **Metadata** — `metadata: dict[str, Any]` with a typed subset: instrument,
   serial, firmware, integration time, GPS, timestamp, operator, sample ID,
   ambient T/RH.
6. **Multi-spectrum files** — `.spc` sub-files, OPUS multi-blocks, SLI
   libraries, archives. The reader always returns a `SpectralCollection`
   (length 1 for single spectra) to keep the API uniform.
7. **Extension collisions** — `.spc` (Galactic vs. OceanView vs. Shimadzu vs.
   Renishaw / WiTec), `.NIR` (Foss vs. BUCHI), `.dat` (ENVI cube vs. MODTRAN
   text), `.sig` (SVC PDA vs. SVC laptop variants), `.spa` (Nicolet vs.
   others). Format dispatch is by magic-byte sniffing first, extension only
   as a tie-breaker.

Sources used to compile this inventory are listed in `DIRECTIONS.md`,
Annex A.
