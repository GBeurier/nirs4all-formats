# NIRS / Spectroscopy File Format Inventory

Inventory of file formats that `nirs4all-io` aims to ingest, with the current
state of open-source support and the parser strategy we plan to use. Sections
1–3 capture the formats provided in the original spec; sections 4–6 add formats
discovered during the web research phase (cf. `DIRECTIONS.md`, Annex A).

## Current Native Coverage

Experimental readers currently implemented in Rust and tested on committed
fixtures:

| Format family | Extensions / fixture class | Status | Notes |
|---|---|---|---|
| Plain delimited spectral tables | `.csv`, `.tsv`, numeric-header `.txt` | Experimental | One record per row, numeric header columns become the spectral axis, numeric non-spectral columns become targets. |
| Bruker OPUS DPT export | `.dpt` | Experimental | Two-column ASCII, wavenumber axis in `cm-1`. |
| Avantes AvaSoft ASCII | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR` | Experimental | Wave tables and two-column irradiance export. Binary AvaSoft variants remain pending. |
| JCAMP-DX | `.jdx`, `.dx`, `.jcm` | Experimental partial | Plain AFFN `XYDATA=(X++(Y..Y))` only. DIF/DUP, SQZ/PAC and NTUPLES are pending. |
| Spectral Evolution SED | `.sed` | Experimental | Header key/value metadata plus wavelength and signal columns. |
| SVC/GER SIG | `.sig` | Experimental | Spectra Vista SIG text fixtures with reference, target and reflectance channels. |

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
| ENVI Spectral Library | `.sli` + `.hdr` (sometimes `.slb`) | mixed (hdr ascii + sli bin) | ✅ | R: `RStoolbox::readSLI()`; Py: `spectral` (Spectral Python), `pysptools` | Well documented. Reference for our internal representation: paired metadata + binary block. |
| FGI HDF5 + XML | `.h5`, `.hdf5`, `.xml` | hdf5 + xml | 🟡 | R: `rhdf5`, `hdf5r`, `xml2`; Py: `h5py`, `lxml` | Standards-based container, but schema is FGI-specific → needs a dedicated mapper. |
| Excel spectral tables | `.xls`, `.xlsx` | tabular | ✅ | `readxl`, `openpyxl`, `pandas.read_excel()` | `.xlsx` is ZIP/XML, `.xls` is OLE/CFB. Trivial if header convention is documented. |
| MFR Sun Photometer | `.OUT` | ascii | 🟠 | Ad-hoc parser; SPECCHIO | Regular fixed-width text. Needs a small format detector. |
| Ocean Optics SpectraSuite | `.csv` (non-comma) | ascii | ✅ | R: `lightr`, `pavo::getspec()`; Py: ad-hoc | Variant CSV with `;` or tab separator + multi-line header. |
| Ocean Optics OceanView | `.txt`, `.ProcSpec`, `.spc` (Ocean Optics flavour, not Galactic) | mixed | 🟡 | R: [`lightr::lr_parse_procspec()`](https://www.rdocumentation.org/packages/lightr/versions/1.9.0/topics/lr_parse_procspec) | `.ProcSpec` is a proprietary container (XML + binary payload, optionally archived) with a checksum that `lightr` validates. Layout drifts across OceanView versions. |
| PP Systems UniSpec SC | `.SPT` | ascii | 🟠 | Ad-hoc | Limited literature; SPECCHIO claims support. |
| PP Systems UniSpec DC | `.SPU` | ascii | 🟠 | Ad-hoc | Same family as `.SPT`. |
| Microtops Sun Photometer | `.TXT` | ascii | 🟠 | Ad-hoc | Text but rich metadata (geo, sun angle, pressure, water vapor). Parser must preserve them. |
| GER 3700 / SVC | `.sig` | ascii (with variants) | ✅ | R: `spectrolab`; Py: `specdal` | Two header conventions (PDA vs. laptop). |
| SVC HR-1024 / HR-1024i | `.sig` | ascii (with variants) | ✅ | Same as above | Date/GPS/units differ across firmware. |
| Spectral Evolution / PSR | `.sed` | ascii | ✅ | R: `spectrolab::read_spectra(format="sed")`; Py: `specdal` | Best-documented field spectrometer ASCII format. |
| MODTRAN5 albedo | `.dat` | ascii | ✅ | Text loader | Not really an instrument format — auxiliary scientific text. |
| IDL / ENVI text output | `.txt` (whitespace-sep) | ascii | ✅ | Text loader | Needs header-line skipping and column auto-detection. |
| USGS SPECPR / PRISM | `SPECPR` (no ext.) | bin (historical) | 🟠 | USGS free SW; convert to ENVI/ASCII | Practical approach: shell out to USGS converter once, then ingest ASCII/ENVI. |

---

## 2. Benchtop / industrial / FT-NIR (extension of the user list)

| Vendor / instrument | Extensions | Container | Open status | Reference readers | Notes |
|---|---|---|---|---|---|
| Bruker OPUS — Tango / MPA / Matrix series | OPUS native | bin | ✅ | Same as OPUS above | Production NIR analyzers also write `.0` style OPUS files. |
| Thermo / Galactic GRAMS | `.spc` | bin | ✅ | Py: [`spc-spectra`](https://github.com/nick-macro/spc-spectra), [`rohanisaac/spc`](https://github.com/rohanisaac/spc), `specio`, `spectrochempy`; xylib (C++); JS: [`cheminfo/spc-parser`](https://cheminfo.github.io/spc-parser/) | De-facto interchange format. Multiple binary variants: **old vs. new** header, **LSB vs. MSB** byte order, and several data layouts (`-XY`, `-XYY`, `-XYXY`) for single-spectrum, common-X multi-spectrum, or independent-X multi-spectrum files. Test fixtures must cover every combination. |
| Thermo Nicolet OMNIC | `.spa`, `.spg`, `.srs`, `.srsx` | bin | 🟡 | Py: [`spectrochempy.read_omnic()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_omnic.html), `lerkoah/spa-on-python` (.spa only) | `.spa` single, `.spg` group, `.srs/.srsx` time series. Multiple undocumented variants per OMNIC release. |
| Perkin Elmer Spectrum / IR | `.sp`, `.fsm` | bin | 🟡 | Py: `specio` | `.fsm` is an imaging variant we will treat as out-of-scope for v1. |
| Foss NIRSystems / WinISI | `.NIR`, `.DA`, `.cal`, `.eqa` | bin | 🔴 | None reliable | **⚠ `.NIR` extension is shared with BUCHI NIRCal — never route by extension alone, always sniff the header signature.** Spectra are exportable from WinISI but the native binary has no open reader. v1 strategy: ingest WinISI text exports only. |
| Foss DA1650 / DS2500 / DS3 | CSV/report exports + optional `.NIR` spectrum export | mixed (export) | 🟡 via export | Standard text loader; `.NIR` export needs reverse engineering | DS3 manual ([p. 45](https://www.manualslib.com/manual/2155011/Foss-Nirs-Ds3.html?page=45)) confirms CSV report + optional binary `.NIR`. JCAMP/OPUS interoperability is **not** a documented capability of stock firmware. |
| Metrohm NIRS XDS / DS2500 / Vision / Vision Air | CSV/Excel exports; native [Vision](https://www.metrohm.com/cs_cz/service/software-center/vision.html) project DB | mixed | 🟡 via export | Standard text loader | Heavily used in pharma/agro/feed routine. Native Vision project DB has no open reader; CSV/Excel exports are the practical path for v1. |
| Bruker Tango (FT-NIR) | OPUS native | bin | ✅ | Same as OPUS | Same loader as benchtop OPUS. |
| BUCHI NIRFlex / NIRMaster (NIRCal) | `.nir`, JCAMP-DX export | bin / ascii | 🟡 / ✅ via export | R: [`prospectr::read_nircal()`](https://rdrr.io/cran/prospectr/man/read_nircal.html) | NIRCal `.nir` is a binary that bundles spectra + metadata + reference properties (protein, moisture…). Distinct from FOSS `.NIR`. `prospectr::read_nircal()` is the reference reader; no Python port yet. |
| Perten DA / Inframatic | vendor proprietary, CSV export | bin / ascii | 🔴 / ✅ via export | Same strategy | Field-feed analyzer; spectra typically already pre-calibrated. |
| Avantes AvaSoft v8 (modern) | `.RAW8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.IRR8`, `.RIR8`, `.RMN8`, `.RMD8` | bin | 🟡 | R: `lightr` (subset); MATLAB community tools; **no maintained Python reader** | Each suffix encodes the measurement mode (scope, dark-corrected scope, absorbance, transmittance, reflectance, absolute/relative irradiance, Raman). [AvaSoft 8 manual](https://www.avantes.com/content/uploads/2022/02/020379-AvaSoft-8-Manual.pdf) is the reference. |
| Avantes AvaSoft ASCII exports | `.ttt`, `.trt`, `.tit`, `.tat` | ascii | ✅ | Any text loader (`pandas`) | Cheap fallback when the binary parser is missing. Worth supporting before the binaries. |
| JASCO V-series / FT-IR | `.jws`, `.txt` export | bin / ascii | 🟡 | Py: [`jws2txt`](https://pypi.org/project/jws2txt/), `jwsProcessor`; conversion via OMNIC | Reverse-engineered; mostly UV-Vis but used in NIR mode too. |
| Shimadzu UVProbe | `.spc` (Shimadzu proprietary, **not** Galactic), `.txt` export | bin / ascii | 🟠 | Experimental Py readers ([`pyfasma-spc`](https://pypi.org/project/pyfasma-spc/) note); vendor [converter](https://www.an.shimadzu.co.jp/products/molecular-spectroscopy/uv-vis/semicustom/uv-13/index.html) | Same `.spc` extension as Galactic but different binary format — sniffer must disambiguate. v1 strategy: rely on CSV/TXT export. |
| VIAVI MicroNIR (handheld) | CSV export (`.csv`) | ascii | ✅ | Any CSV loader | Native ".pri" project files: out of scope for v1 (no public spec). |
| Si-Ware NeoSpectra | CSV export | ascii | ✅ | Any CSV loader | Handheld MEMS spectrometer; exports CSV with site/soil metadata block. |
| Spectro Inc. SiWare API | JSON/CSV | ascii | ✅ | Standard JSON/CSV | Recent cloud-attached spectrometers; consider streaming. |

---

## 3. Standardized / vendor-neutral formats

| Format | Extensions | Container | Open status | Reference readers | Notes |
|---|---|---|---|---|---|
| JCAMP-DX | `.jdx`, `.dx`, `.jcm`, `.jcamp` | ascii | ✅ (with caveats) | Py: [`jcamp`](https://pypi.org/project/jcamp/), [`spectrochempy.read_jcamp()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_jcamp.html), `nmrglue`; R: `ChemoSpec`, `hyperSpec` | [IUPAC standard](https://iupac.org/what-we-do/digital-standards/jcamp-dx/). The `.dx`/`.jdx` payload can use `AFFN`, `XYDATA`, `DIF/DUP`, or `NTUPLES` encoding — each existing reader covers a subset. Test fixtures must exercise every encoding. v1 priority. |
| ANDI / NetCDF | `.cdf`, `.nc` | nc | ✅ | Py: `netCDF4`, `xarray` | [ASTM E1947](https://store.astm.org/e1947-98r22.html) is the **chromatography-MS** ANDI standard, *not* a NIR/FTIR standard. Listed as adjacent inspiration; cite a specific FTIR vendor before treating it as a target. |
| AnIML | `.animl` (xml) | xml | 🟡 | Py: `animl-python` (early), Schema validators | IUPAC + ASTM XML standard. Good long-term target. |
| Allotrope ADF | `.adf` | hdf5 + triplestore | 🟡 | Allotrope Foundation SDK | Pharma-grade standard, heavy stack. Not a v1 priority. |
| mzML / mzMLb | `.mzML`, `.mzMLb` | xml / hdf5 | ✅ | `pyteomics`, `pymzml` | MS-oriented but cited as design inspiration for our internal schema. |
| Plain CSV / TSV / TXT | `.csv`, `.tsv`, `.txt` | ascii | ✅ | `pandas`, `nirs4all.data.loaders.CSVLoader` | Already supported in `nirs4all`. We extend it with header heuristics. |
| Parquet | `.parquet` | columnar bin | ✅ | `pyarrow`, `fastparquet`, `nirs4all.data.loaders.ParquetLoader` | Already in `nirs4all`. Used as the internal cache format. |
| HDF5 (generic) | `.h5`, `.hdf5` | hdf5 | ✅ | `h5py`, `tables` | Generic loader + schema mapping per vendor. |

---

## 4. Hyperspectral imaging (out-of-scope for v1, listed for completeness)

These are explicitly not the primary target (we focus on point spectra), but
their formats reuse many of the same containers and several
hyperspectral-imaging users may want to extract pixel spectra:

- ENVI image cubes (`.dat`/`.img` + `.hdr`) — supported by Spectral Python.
- ENVI Spectral Library (`.sli`) — already in section 1.
- Specim / HySpex / Headwall raw cubes (often ENVI-compatible).
- BIL/BIP/BSQ raw with sidecar header.
- HDF5-based imaging (NEON AOP, AVIRIS-NG).

v1 policy: detect these as "imaging" and refuse to load, with a clear pointer
to `spectral` / `rasterio`. A future `extract_point_spectra(cube, mask)` helper
is a credible v2 feature — many agro / pharma users do own Specim or HySpex
data and would benefit from a single pipeline that pulls pixel spectra into
the same `SpectralRecord` schema as point spectroradiometers.

---

## 5. Adjacent useful formats

| Format | Why it matters | Decision |
|---|---|---|
| `.mat` (MATLAB) | Many academic NIR datasets are shared as MATLAB files | Already in `nirs4all` via `MatlabLoader`. Reuse. |
| `.npy` / `.npz` | Common in ML workflows | Already in `nirs4all`. Reuse. |
| `.xlsx` | Many lab transfers happen via Excel | Already in `nirs4all`. Reuse. |
| Raman / UV-Vis "look-alikes" (Renishaw WDF, Horiba LabSpec, WiTec WIP, JASCO) | Same `.spc`/`.jws` family, often confused with NIR | Detect, report instrument type, refuse only if explicitly NIRS-only mode is requested. |

---

## 6. Coverage summary (target for v1)

| Tier | Formats | Rationale |
|---|---|---|
| **A — must-have** | `.asd`, `.sig`, `.sed`, OPUS native, `.spc` (Galactic, all sub-variants), JCAMP-DX (`AFFN`/`XYDATA`/`DIF/DUP`/`NTUPLES`), ENVI SLI, Avantes AvaSoft 6/7 (`.TRM`/`.ABS`/`.ROH`/`.DRK`/`.REF`) and AvaSoft 8 (`.RAW8`/`.RFL8`/`.ABS8`/`.TRM8`/`.IRR8`), Avantes ASCII exports, CSV/TSV variants, Excel | Cover the majority of academic and industrial NIR field/benchtop deployments. |
| **B — high value** | `.spa`/`.spg`/`.srs` (Nicolet OMNIC), `.sp` (PE), Foss/Metrohm/Buchi CSV/JCAMP exports, BUCHI NIRCal `.nir` (via `prospectr` port), ASD `.ILL`/`.REF`/`.RAW`, OceanView `.ProcSpec`, JASCO `.jws`, Shimadzu UVProbe `.spc` | High-impact but partial open support; budget reverse-engineering or R-port work. |
| **C — opportunistic** | FGI HDF5+XML, Foss `.NIR` native, Perten native, AnIML, Allotrope ADF, USGS SPECPR | Either niche, vendor-locked, or covered by export workflows. |

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
