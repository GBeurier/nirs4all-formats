# Compact Format Matrix

Control columns:

- `Variants`: subformats, versions, layouts or modes explicitly followed.
- `Validated`: sufficient analysis and metadata for the variant, with sample/test/doc.
- `Partial`: useful but incomplete parser for this variant (metadata, calibration,
conformance, incomplete axes or targets).
- `Planned`: variant identified and actionable, but not yet coded.
- `Blocked`: variant identified but blocked by sample/spec/license/reference.
- `NIRS coverage`: product reading of the line. `publishable` means that the
main active variants are covered; `targeted publishable` means
publishable if the scope is explicitly announced; `useful but incomplete`
still asks for code before strong communication; `not viable` request
  d'abord des samples/specs ou un parser significatif.
- `Missing impact`: business severity of the remaining lack. `none` or `minor`
does not prevent publish if the scope is clear; `medium` asks for a
useful complement but the main spectra are preserved; `severe`
means that an active variant, an essential calibration or a part
significant spectra missing; `blocking` means that we cannot
claim the format; `out of scope` means intentionally adjacent or
  hors cible.
- `Popularity`: expected frequency in NIRS field/industry/research, not
only the number of samples available.
- `Priority`: recommended project effort. `P0` conditions the NIRS value
industrial or field; `P1` adds strong coverage; `P2` is useful or
  adjacent; `P3` peut attendre.
- `Critical gap`: which prevents the line from being considered fully
  couverte ou ce qu'il faut faire ensuite.

Reading rule: `Popularity` high + `Missing impact` `severe`/`blocking` +
`Priority` P0/P1 indicates where to search for original files or encode first.
`NIRS coverage = publishable` with impact `none`/`minor`/`medium` indicates a
publishable scope if the limits are announced.

Visual synthesis: see `IMPLEMENTATION_DASHBOARD.md` for compact graphs
format / confiance probe / maturite / manquants.

Sorting: the main matrix is ​​sorted by `Priority`, then `Missing impact`, then
`Popularity`. The monitoring tables are sorted by their control column
visible (`Current tracking`, `Corpus` ou `Format`).

| Name | Vendor | Extensions | Variants | Validated | Partial | Planned | Blocked | NIRS coverage | Missing impact | Popularity | Priority | Critical gap | Reference lib |
|---|---|---|---:|---:|---:|---:|---:|---|---|---|---|---|---|
| Foss NIRSystems / WinISI native | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | 4 | 3 | 0 | 0 | 1 | publishable with scoped support | minor | very common in industry | P2 source | Native `.cal`/`.nir` decodes and validates vs export ISIscan `.txt`; benches DS-series DS2500/DS3F (`foss-ds-nir`) also decoded via the same reader. Real fixtures local-only + goldens synthetic (`synthetic.cal`, `synthetic_ds2500.nir`, `synthetic_ds3f.nir`). Remains `.DA`/`.eqa` unsampled. | export ISIscan `.txt` |
| Perten DA / Inframatic | Perten / PerkinElmer | vendor binary, `.csv` | 2 | 0 | 0 | 0 | 2 | not viable | blocking | very common in industry | P0 source | Key industrial format, none sample spectral native/export usable. | export CSV/Excel seller |
| ASD calibration | ASD / Malvern Panalytical | `.ILL`, `.REF`, `.RAW` | 3 | 0 | 0 | 0 | 3 | not viable | blocking | specialized | P1 source | Companion samples missing; useful but not essential to the `.asd` reader. | SPECCHIO, asdreader |
| PP Systems UniSpec DC | PP Systems | `.SPU` | 1 | 0 | 1 | 0 | 0 | not viable | blocking | specialized field use | P1 source | Parser synthetic only; two-channel field acquisition required. | SPECCHIO, ad hoc parsers |
| PP Systems UniSpec SC | PP Systems | `.SPT` | 1 | 0 | 1 | 0 | 0 | not viable | blocking | specialized field use | P1 source | Parser synthetic only; real land acquisition necessary. | SPECCHIO, ad hoc parsers |
| Avantes AvaSoft 8 binary | Advantages | `.Raw8`, `.IRR8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8` | 9 | 1 | 1 | 0 | 7 | targeted publishable | severe | common | P1 source | Lots of AVS8 suffixes without fixture; `.IRR8` remains partial calibration but its factors are exposed as `irradiance_calibration`. | lightr, AvaSoft manual |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, native project base | 3 | 1 | 0 | 1 | 1 | targeted publishable | severe | common in industry | P1 source | CSV ok; native base/real client exports to obtain. | text parser, pandas, readxl |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | 3 | 2 | 0 | 0 | 1 | useful but incomplete | severe | specialized | P1 source | Synthetic fasteners; Real API response necessary before large diffusion. | Standard JSON/CSV |
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | 8 | 4 | 0 | 3 | 1 | publishable | medium | common in field use | P1 source | Main revisions ok; look for v3-v5 and internal calibration blocks. | asdreader, prospectr, spectrolab, specdal, pyASDReader |
| Avantes AvaSoft 6/7 binary | Advantages | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | 5 | 4 | 0 | 0 | 1 | targeted publishable | medium | common | P1 source | `.ABS` binary missing; current perimeter quite useful. | lighter |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | 4 | 1 | 1 | 1 | 1 | targeted publishable | medium | common in industry | P1 source | `.nir` useful with spectra, targets, replicats and metadata spectrum type prospectr; missing redistributable fixture non-null targets, `.cal` and NIRMaster variants. | prospectr::read_nircal |
| JCAMP-DX | Vendor-neutral / IUPAC | `.jdx`, `.dx`, `.jcm`, `.jcamp` | 6 | 5 | 0 | 0 | 1 | publishable | medium | common exchange | P1 source/complete | XYDATA/ASDF/NTUPLES/Ocean LINK/PEAK TABLE ok, with verification of checkpoints LINK general real remains to be framed. | jcamp, SpectroChemPy, nmrglue, ChemoSpec, hyperSpec |
| HDF5 NIRS generic | Vendor-neutral | `.h5`, `.hdf5` | 4 | 3 | 0 | 0 | 1 | targeted publishable | medium | specialized research | P1 source | Canonical multi-signal scheme + alias/transpose ok; schemas real metadata-rich to source. | h5py, hdf5-reader, tables |
| Si-Ware NeoSpectra | Si-Ware | `.csv`, `.xlsx` | 3 | 2 | 0 | 0 | 1 | publishable | minor | common handheld | P1 source | Real matrices ok; single-measurement Scanner missing. | text parser, openpyxl |
| Spectral Evolution / PSR | Spectral Evolution | `.sed` | 4 | 3 | 1 | 0 | 0 | publishable | minor | common in field use | P1 complete | Useful ground coverage; complete SR variants and conformation reference. | spectrolab, specdal |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | 6 | 5 | 1 | 0 | 0 | publishable | minor | common in field use | P1 complete | Very useful land; conformance metadata `spectrolab`/`specdal` (instrument/foreoptic/integration/coadds/temp/battery/error/factors) covered; remain calibrated physical radiance and byte-level conformance. | spectrolab, specdal |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.xlsx`, `.sam`, `.pri` | 4 | 3 | 0 | 0 | 1 | publishable | minor | common handheld | P2 source | Exports real ok; native `.sam` decode (125 canaux + 128 pixels bruts, fixtures local-only); `.pri` projet customer-only. | parseur texte, openpyxl |
| Allotrope ADF | Allotrope Foundation | `.adf` | 4 | 0 | 2 | 0 | 2 | not viable | blocking | emerging industry | P2 complete | ADF local partial; SDK/ontologie/fixtures vendeurs manquent. | Allotrope SDK, adfsee |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | 5 | 2 | 1 | 0 | 2 | adjacent targeted | severe | common Raman | P2 source | XML/TXT ok; `.l6m` experimental, `.l6s`/layouts LabSpec6 absent. | RosettaSciIO, SpectroChemPy, horiba-raman |
| WiTec WIP/WID | WiTec | `.wip`, `.wid`, `.txt` | 5 | 1 | 1 | 0 | 3 | adjacent targeted | severe | common Raman | P2 source | An ok WIP map layout with Raman axis and coordinates; other layouts/projects and ASCII export paired with source. | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| AnIML | IUPAC / ASTM | `.animl` | 5 | 2 | 0 | 2 | 1 | useful but incomplete | severe | emerging/niche | P2 source | Vrais AnIML spectraux/XSD/conformance manquent. | animl-python, validateurs XML |
| FGI HDF5 + XML | FGI | `.h5`, `.hdf5`, `.xml` | 2 | 1 | 0 | 0 | 1 | targeted publishable | severe | niche | P2 source | Real FGI pair absent; synthetic only. | h5py, hdf5r, rhdf5, lxml |
| Bruker OPUS native | Bruker | `.0`, `.1`, `.001`, `.0000`, without fixed extension | 5 | 2 | 1 | 0 | 2 | publishable | medium | very common | P2 complete | OPUS 7/8 and MPA ok; OPUS 5/6 legacy and imaging remain secondary. | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.jdx`, `.spc` | 11 | 8 | 0 | 3 | 0 | publishable | medium | very common | P2 complete | Wide active coverage; add QE Pro/Maya/Apex if samples; Flame-NIR (InGaAs 950-1650 nm) in-family, OceanView export to source. | lightr, pavo |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc`, `.SPC` | 6 | 3 | 1 | 1 | 1 | publishable | medium | very common | P2 complete | Useful new/old LSBs; BE and old logs remain secondary. | spc-spectra, rohanisaac/spc, specio, SpectroChemPy, xylib, spc-parser |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs`, `.srsx` | 5 | 3 | 1 | 0 | 1 | publishable | medium | very common | P2 complete | Useful SPA/SPG/SRS; `.srsx` absent, SRS secondary axis to be enriched; Thermo Antaris II FT-NIR `.spa`/`.spg` in-family, fixture branded at source. | SpectroChemPy, spa-on-python |
| Bruker Tango / MPA / Matrix | Bruker | native OPUS | 3 | 1 | 0 | 2 | 0 | targeted publishable | medium | common | P2 source | MPA covered; look for dedicated Tango/Matrix for marketing vendor. | opusreader2, SpectroChemPy |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5, `.lan`, `.mat` | 7 | 3 | 1 | 1 | 2 | targeted publishable | medium | common HSI | P2 source | ENVI/AVIRIS cubes ok with rectangular ROI and sparse mask `(row, col)`; Specim/HySpex/NEON/HDF5 remain at source. | spectral, rasterio, scipy |
| JASCO JWS | JASCO | `.jws`, `.txt` | 7 | 4 | 0 | 0 | 3 | publishable | medium | common lab | P2 source | NIR/Raman JWS variants absent; main public streams covered. | jws2txt, jwsProcessor |
| MATLAB MAT/RData | MATLAB/R ecosystem | `.mat`, `.MAT`, `.RData` | 6 | 5 | 1 | 0 | 0 | publishable | medium | common search | P2 complete | Useful ML coverage; arbitrary structures to expand. | scipy, hdf5-reader, R serialization, prospectr |
| OpenSpecy Raman/(FT)IR | OpenSpecy (R) | `.rds` | 3 | 3 | 0 | 0 | 0 | publishable | none | common in microplastics | P3 publish | Canonical OpenSpecy list (gzip/XDR) + legacy wavenumber/intensity data.frame via pure-Rust rds2rust; large libraries still load spectra via structural fallback when names/metadata are dropped; bzip2/xz `.rds` are the known gap. | OpenSpecy::read_spec, readRDS |
| Perkin Elmer Spectrum / IR | PerkinElmer | `.sp`, `.fsm` | 2 | 1 | 0 | 0 | 1 | targeted publishable | medium | common | P3 source | `.sp` ok (incl. FT-MIR Spectrum 10 has residual blocks); `.fsm` imaging out of scope v1. | specio |
| Renishaw WDF | Renishaw | `.wdf` | 12 | 9 | 1 | 0 | 2 | adjacent publishable | medium | common Raman | P2 complete | Adjacent Raman heavily covered; complete MAP layouts/conformance. | RosettaSciIO, SpectroChemPy |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | 2 | 1 | 0 | 0 | 1 | targeted publishable | medium | common lab | P2 source | `.txt` ok; Missing native `.spc`. | pyfasma-spc, Shimadzu converter |
| Felix Instruments F-750 | Felix Instruments / CID Bio-Science | `.csv` (DataViewer) | 4 | 1 | 0 | 2 | 1 | targeted publishable | medium | common handheld terrain | P2 complete | Export CSV absorbance (mango DMC, CC-BY) covered via `csv_like`; reflectance/2nd-derivative and native-to-source blind modes. | pandas, read.table |
| ASM allotrope | Allotrope / Benchling | `.json` | 3 | 2 | 0 | 1 | 0 | targeted publishable | medium | emerging industry | P2 source | Benching ok; seller conversions to obtain. | Allotropy benchmarking |
| Generic NetCDF NIRS | Vendor-neutral | `.nc`, `.cdf` | 5 | 3 | 1 | 1 | 0 | targeted publishable | medium | specialized research | P2 complete | Dedicated schematics ok; generic Real spectral NetCDF to expand. | netcdf-reader, xarray, netcdf, ARM ACT |
| MFR Sun Photometer | Solar Light / YES Inc. | `.OUT`, `.nc` local | 3 | 2 | 0 | 0 | 1 | targeted publishable | medium | niche | P2 source | `.OUT` real redistributable absent; NetCDF ARM local only. | SPECCHIO, ad hoc parsers, xarray, ARM ACT |
| Microtops Sun Photometer | Solar Light | `.TXT`, `.nc`, `.lev10/.lev15/.lev20` | 4 | 2 | 1 | 0 | 1 | targeted publishable | medium | niche | P2 source | MAN NetCDF/ASCII ok; Real legacy `.TXT` and generic NetCDF remain incomplete. | ad hoc parsers, xarray |
| Excel spectral | Generic / lab | `.xlsx`, `.xlsm`, `.xls` | 3 | 2 | 0 | 0 | 1 | publishable | minor | common | P2 source | `.xls` legacy OLE is missing; non-blocking for modern broadcasting. | calamine, openpyxl, pandas, readxl |
| USGS SPECPR / PRISM / ECOSTRESS text | USGS / JHU / ECOSTRESS | `SPECPR`, `.asc`, `.txt`, `.spectrum.txt` | 4 | 3 | 0 | 0 | 1 | publishable | minor | common in datasets | P2 source | Texts ok; binary SPECPR missing but little blocking v1. | USGS converter |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr` | 3 | 2 | 0 | 0 | 1 | publishable | minor | specialized | P3 publish | `.slb` non fixture; faible impact NIRS. | spectral, RStoolbox, pysptools |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | 5 | 5 | 0 | 0 | 0 | adjacent publishable | minor | adjacent niche | P3 publish | No known sample blocking; Adjacent AFM/Raman. | RosettaSciIO |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | 8 | 8 | 0 | 0 | 0 | adjacent publishable | minor | niche Raman | P3 publish | Aucun sample blocking connu; Raman adjacent. | RosettaSciIO |
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | 2 | 2 | 0 | 0 | 0 | publishable | none | very common in industry | P3 publish | None; does not replace the native Foss. | text parser |
| Axis-first tables | Generic / instrument exports | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | 8 | 8 | 0 | 0 | 0 | publishable | none | very common | P3 publish | None; covers a lot of vendor exports. | pandas, read.table |
| Delimited spectral tables | Generic | `.csv`, `.tsv`, `.txt` | 3 | 3 | 0 | 0 | 0 | publishable | none | very common | P3 publish | None; useful base for external imports. | pandas, read.table, nirs4all CSVLoader |
| Advantages ASCII | Advantages | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.txt` | 6 | 6 | 0 | 0 | 0 | publishable | none | common | P3 publish | None; good path recommended when export available. | pandas, read.table |
| Bruker OPUS DPT | Bruker | `.dpt` | 1 | 1 | 0 | 0 | 0 | publishable | none | common | P3 publish | None; ASCII export only. | pandas, read.table, lightr |
| Consumer Physics SCiO | Consumer Physics | `.csv` (developer app) | 3 | 3 | 0 | 0 | 0 | publishable | none | common handheld | P3 publish | None; Handheld NIR useful. | kebasaa/SCIO-read |
| Spectral matrices | Generic / Foss / Metrohm / VIAVI | `.csv`, `.txt` | 3 | 3 | 0 | 0 | 0 | publishable | none | common | P3 publish | None; useful for ML and wide exports. | pandas, read.table |
| NumPy | Python/NumPy | `.npy`, `.npz` | 2 | 2 | 0 | 0 | 0 | publishable | none | common data | P3 publish | None; Useful Python bindings. | numpy |
| Parquet | Apache / generic | `.parquet` | 1 | 1 | 0 | 0 | 0 | publishable | none | common data | P3 publish | None; useful internal distribution format. | pyarrow, fastparquet, nirs4all ParquetLoader |
| IDL / ENVI texte | IDL / ENVI | `.txt` | 1 | 1 | 0 | 0 | 0 | publishable | none | specialized | P3 publish | Aucun. | parseur texte |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | 3 | 3 | 0 | 0 | 0 | adjacent publishable | none | adjacent | P3 publish | Aucun; surtout microscopie/spectro adjacent. | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | 2 | 2 | 0 | 0 | 0 | adjacent | out of scope | adjacent niche | P3 monitor | Excluding NIRS point-spectra; keep as disambiguation. | RosettaSciIO |
| MODTRAN albedo | Spectral Sciences / AFRL | `.dat` | 1 | 0 | 1 | 0 | 0 | not viable | out of scope | niche | P3 source | No core NIRS; real redistributable sample missing. | text parser |
| ANDI / NetCDF MS | ASTM / vendor-neutral | `.cdf`, `.nc` | 1 | 1 | 0 | 0 | 0 | adjacent | out of scope | adjacent | P3 monitor | Non-NIRS refusal useful for disambiguation. | pyteomics, PyMassSpec, pyOpenMS |
| mzML / mzMLb | HUPO PSI / MS vendors | `.mzML`, `.mzMLb` | 2 | 1 | 0 | 0 | 1 | adjacent | out of scope | adjacent | P3 monitor | Non-NIRS refusal; `.mzMLb` only documented. | pyteomics, pymzML, pyOpenMS |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | 5 | 0 | 0 | 0 | 5 | out of scope | out of scope | out of scope | P3 out of scope | Untargeted physiology by nirs4all-formats spectroscopy. | MNE-NIRS, SNIRF |

## Files to Source Next

This list is the external request to be transmitted to a colleague with
machine access. Each useful batch must contain, if possible, the raw file
original, a readable export produced by the vendor software (`.csv`, `.txt`,
`.xlsx`, JCAMP-DX, etc.), le nom du logiciel et sa version, le modele
instrument, le mode de mesure (raw, absorbance, reflectance, transmittance,
radiance, irradiance), et quelques longueurs d'onde/valeurs verifiables. Les
donnees peuvent etre anonymisees; il faut surtout conserver le format original
et les metadata structurelles. Les lots sont tries par priorite projet.

| Priority | Format / machine | Files to request | Why |
|---|---|---|---|
| P2 | Foss NIRSystems / WinISI / ISIscan | Native `.DA` and `.eqa` (the `.cal`/`.nir` are decoded via fixtures local-only); ideally a redistributable game `.cal`/`.nir` + export `.txt` of the same game. | Native `.cal`/`.nir` decodes and validates vs export ISIscan `.txt`; remain `.DA`/`.eqa` and a public redistributable fixture. |
| P0 | Perten DA / Inframatic | Native spectral vendor file, plus CSV/XLSX export containing wavelength columns and spectral values; avoid target-only relationships. | Key industrial format; none sample spectral native/export usable. |
| P1 | ASD FieldSpec calibration | Full games `.asd` + companions `.ILL`, `.REF`, `.RAW`; if possible with white/dark/reference and corresponding ASCII export. | Unlocks currently missing calibration companion files. |
| P1 | ASD FieldSpec revisions missing | `.asd` revisions v3/v4/v5, files with internal secondary/dependent/reference/calibration blocks, audit or signatures. | Recent major revisions have been read, but these variants remain to be confirmed. |
| P1 | Avantes AvaSoft 8 | `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8`; if possible a multi-subfile set and an `.IRR8` with complete irradiance calibration. | Many active AVS8 suffixes do not yet have a fixture. |
| P1 | Avantes AvaSoft 6/7 binary | `.ABS` binary legacy, plus any other non-export ASCII legacy mode; attach readable AvaSoft export if available. | The `.ABS` binary is the remaining hole in the legacy drive. |
| P1 | BUCHI NIRCal / NIRMaster | Redistributable `.nir` with non-zero properties/targets, `.cal` calibration-only files, JCAMP-DX exports and recent NIRMaster/NIRFlex variants. | The reader reads the `.nir` and its main spectrum metadata, but misses a public fixture rich in targets and variants. |
| P1 | HDF5 NIRS generic | `.h5`/`.hdf5` real from spectrometers or NIRS pipelines, with datasets spectra/absorbance/reflectance + axes wavelengths/wavenumbers + metadata; include nested groups, transpose matrices, multi-signals and targets if possible. | Simple schemes, common aliases and synthetic multi-signals pass; Real schemas are needed to harden metadata, complex groups and field conventions. |
| P1 | JCAMP-DX spectral | `.jdx`, `.dx`, `.jcm`, `.jcamp` with `LINK` general multi-blocks, PEAK TABLE/ASSIGNMENTS real, spectroscopic NTUPLES not already covered; join export seller if possible. | The XYDATA/ASDF/NTUPLES/Ocean LINK/PEAK TABLE core works; Above all, it lacks real generic LINKs and peak-table conformance. |
| P1 | Metrohm Vision / Vision Air / OMNIS NIR | Vision Air exports real CSV/XLSX with spectral axis, native base/project if possible, and any OMNIS NIR export. | Synthetic CSV only; the native database/project remains closed. |
| P1 | PP Systems UniSpec SC | Raw terrain acquisition `.SPT` from a UniSpec SC, with metadata header and possible export. | The parser is only valid on synthetic. |
| P1 | PP Systems UniSpec DC | Raw terrain acquisition `.SPU` from a two-channel UniSpec DC, with metadata header and possible export. | The two-channel parser is only valid on synthetic. |
| P1 | Si-Ware NeoSpectra Scanner | Export single-measurement NeoSpectra Scanner, CSV/XLSX or other app format, separate from OSSL wide matrices. | The actual dies are covered; the one measurement per file format is missing. |
| P1 | Spectral Evolution / PSR / SR | `.sed` of SR-3500, SR-6500 and recent firmwares, with reflectance and/or radiance/DN, explicit units, GPS if available. | Useful terrain coverage with promoted units/metadata; the SR variants and `spectrolab`/`specdal` comparisons remain to be expanded. |
| P1 | Spectro Inc. SiWare API | Actual JSON API responses and associated CSV exports, with optional wavelengths, absorbance/reflectance, predictions and metadata. | Current fixtures are synthetic. |
| P1 | SVC / GER SIG | `.sig` HR-1024i firmware >= 3.0, files with explicit physical radiance (W/m^2/sr/nm calibers), exports `spectrolab` comparable byte-a-byte resamples and possible `.sig` GER 1500 histories. | The main field variants pass and the `spectrolab`/`specdal` metadata is covered; These files improve the physical radiance unit and enable byte-level conformance. |
| P2 | VIAVI MicroNIR | MicroNIR native `.pri` project file, plus CSV/XLSX exports of the same scan. | Real exports and native `.sam` pass; only the `.pri` project remains customer-only. |
| P2 | Allotrope ADF seller | `.adf` vendor instrumentals (Waters, Sciex, Agilent, Bruker or other), ideally spectral, with ontology/units and equivalent export. | Local ADF proves detection; Instrumental ADFs and SDK validation are missing. |
| P2 | ASM allotrope | JSON ASM from multiple vendor conversions, not just Benchling/plate-reader; include spectral cases if available. | Benchling is covered; industrial diversity must be validated. |
| P2 | AnIML | True spectral `.animl` with XSD/conformance, non-zero segmented indices and multiple SeriesSets. | Current spectral examples are synthetic or non-spectral. |
| P2 | Bruker OPUS legacy | OPUS 5/6 archives `.0`, `.1`, `.001`, `.0000` or without extension; 2D/imaging blocks if available. | OPUS 7/8 and MPA are well covered; legacy and imaging remain secondary. |
| P2 | Bruker Tango / Matrix | Native OPUS files from Tango FT-NIR and Matrix, with DPT/CSV export of the same scan. | MPA is covered; Dedicated Tango/Matrix fixtures are missing. |
| P2 | ENVI / hyperspectral cubes | Games `.hdr` + `.dat/.img` Specim, HySpex, Headwall; NEON AOP HDF5 cubes; Specim IQ if usable archive; HDF5 cubes with metadata. | ENVI/AVIRIS works; these HSI families remain a source. |
| P2 | FGI HDF5 + XML | Pair real `.h5`/`.hdf5` + sidecar `.xml` FGI, with complete XML schema. | The current mapping is synthetic only. |
| P2 | Horiba LabSpec / JobinYvon | `.l6s` single-spectrum, other `.l6m` LabSpec6, and corresponding text/XML export pair. | `.l6m` map experimental and XML/TXT are covered; single-spectrum is missing. |
| P2 | JASCO JWS | `.jws` V-780/V-series NIR and NRS Raman, alternative streams `Data`, `Header`, `XdataValue`; attach JASCO text export. | The current public streams are passing, but not these lab/NIR/Raman variants. |
| P2 | MATLAB MAT/RData Spectral | `.mat` v5/v7.3 and `.RData` real with heterogeneous structures, metadata, targets, cubes or multi-signals. | Useful ML coverage; arbitrary structures to expand. |
| P2 | MFR-7 / MFRSR | `.OUT` MFR-7/MFRSR real redistributable and additional NetCDF ARM with calibration, `_FillValue`, filters and QC. | NetCDF ARM local only; Redistributable `.OUT` missing. |
| P2 | Microtops II / MAN | `.TXT` legacy Microtops II redistributable, generic MAN ASCII/NetCDF exports without restrictive policy, and complete header. | Local MAN works, but no public legacy `.TXT` or generic NetCDF reader without fallback. |
| P2 | Generic NetCDF NIRS | `.nc`/`.cdf` real spectral with wavelengths, spectra, metadata, QC, multi-signal groups. | Dedicated schemes pass; it is necessary to expand the real NIRS schemes. |
| P2 | Ocean Optics / Ocean Insight | Exports QE Pro, Maya, Apex; true `.spc` Ocean non-Galactic; Jaz/OceanView texts with explicit metadata. | Wide active coverage, but several recent devices remain without fixture. |
| P2 | PerkinElmer Spectrum / Lambda / Spotlight | `.sp` PerkinElmer NIR/Lambda, `.fsm` Spotlight imaging, and CSV/TXT exports of the same scan. | `.sp` single-spectrum pass; imaging and NIR/Lambda variants remain a source. |
| P2 | Renishaw WDF | `.wdf` InVia Qontor/Apollo, other `MAP` layouts, maps/depth/time-series with equivalent CSV/ASCII export. | Strong adjacent Raman coverage; some layouts and full-array conformance are missing. |
| P2 | Shimadzu UVProbe | True `.spc` native Shimadzu and true `.txt` UVProbe, with export compare if possible. | The current `.txt` is synthetic; the native `.spc` is missing. |
| P2 | Specim IQ / terrain cubes | Reduced usable Specim IQ archive, with raw/processed identifiers and clear license. | Mentioned in the sweep as a possible source but too big/not isolated for the moment. |
| P2 | Thermo / Galactic GRAMS SPC | `.spc` new big-endian, old headers/logs, atypical multi-subfile IR/NIR files; exclude pure NMR/FID if possible. | Useful LSB variants pass; BE and old logs remain secondary. |
| P2 | Thermo Nicolet OMNIC | `.srsx`, other `.srs` high-speed/rapid-scan, and `.spa/.spg` variants with ASCII export. | SPA/SPG/SRS are useful; `.srsx` remains absent. |
| P2 | Thermo Antaris II FT-NIR | `.spa`/`.spg` native to an Antaris II (RESULT/OMNIC) and RESULT CSV/XLSX export of the same scan. | Decode via OMNIC/GRAMS/tabular reader; missing a branded Antaris fixture (CC-BY datasets: Mendeley 9z7dgdtggk tabac, h8mht3jsbz sol). |
| P2 | Ocean Optics Flame-NIR | OceanView export (text/`.ProcSpec`/`.jdx`) of a Flame-NIR InGaAs 950-1650 nm. | Decode via `ocean_optics`; missing a Flame-NIR export covering the InGaAs axis (vs current CCD fixtures). |
| P2 | Felix Instruments F-750 | DataViewer exports Raw-Spectra (reflectance) and Interpolated-Spectra (2nd derivative), plus the native on-device store. | CSV absorbance (mango DMC) is covered; other export modes and the native format are missing. |
| P2 | WiTec WIP/WID | `.wip`, `.wid` of various WiTec layouts, with equivalent ASCII export of the same project. | A WIP map layout is decoded with Raman axis and coordinates; the general layouts remain at source before expanding the code. |
| P3 | ENVI Spectral Library legacy | `.slb` accompanies `.hdr` if available. | Low NIRS impact, but closes the legacy variant. |
| P3 | Excel legacy | `.xls` OLE spectral, real `.xlsm` with macros, real multi-sheet workbooks, cases where Excel converts wavelengths to dates. | Non-blocking for modern distribution, useful for import robustness. |
| P3 | MODTRAN albedo | `.dat` MODTRAN/ONTAR output redistributable under clear license. | Out of core NIRS; real sample missing. |
| P3 | USGS SPECPR | Original SPECPR binary and AREF dumps with verifiable axes. | USGS/ECOSTRESS texts are covered; the binary is missing. |

Do not prioritize source for this project: fNIRS neuroscience (`.snirf`,
`.nirs`, `.wl1/.wl2`), ANDI/mzML/mzMLb MS, Hamamatsu HPD-TA, DigitalSurf
MountainsMap et Princeton TriVista, sauf si l'objectif change explicitement
vers physiologie, MS ou Raman/AFM adjacent. Ces formats sont hors perimetre ou
already sufficiently covered for current NIRS usage.

## Notes for Unfinished Rows

Les lignes `NIRS coverage = publishable` peuvent rester listees quand il
There are still secondary variants to source, code or complete, as long as
the `Missing impact` remains `minor` or `medium`. The note indicates the concrete shortage:
sample, metadata, calibration, conformance, non-code variant or outside scope
NIRS.

| Name | Current tracking | Note / missing |
|---|---|---|
| Foss NIRSystems / WinISI native | partial | Native `.cal` (spectra + constituents) and `.nir` (spectra) decoded by the `foss_winisi` reader, sniffed by header signature (version word + `ISIscan`/`NIRSystems`), never by extension — which resolves the `.nir` collision with BUCHI NIRCal. A record per sample: absorbance spectrum (`nm` axis from the segment table), target constituent values, metadata (sample_number, product_code, timestamp, instrument). Valid vs ISIscan `.txt` exports at precision float32 (~5e-8) on `yamtot_2026.cal` (18 ech.), `ando4_2026.cal` (20/9 const.), `D2026_20240328.cal` (18/5), `yamtot_2026.nir` (16/0). **FOSS DS-series benches** (DS2500, DS3 F): same binary container but without ISIscan identity string — pinned by the `NIRS DS` model at `0x82` and sniffed as `foss-ds-nir` (segmented `nm` axis, absorbance spectra, instrument metadata). Reels local-only (`fileDS2500CRAW.nir` 10 scale 400–2498 nm, `fileDS3FCRAW.nir` 20 scale 1100–2498 nm, customer data CEPICOP/SYNGENTA). Goldens synthetic CC0 `samples/foss_winisi/synthetic.cal`, `synthetic_ds2500.nir`, `synthetic_ds3f.nir`; real fixtures local-only (license to be defined). Remains `.DA`/`.eqa` unsampled. |
| Perten DA / Inframatic | blocked | No native spectral fixture; the current CSV is a target-only ratio without a spectral axis. A CSV/Excel export with wavelength columns would be processable by tabular readers. |
| ASD calibration | blocked | Get a redistributable game `.asd` + `.ILL/.REF/.RAW`; the current `.asd` samples do not contain the calibration companions, and the `.REF` present in `samples/avantes/` is Avantes, not ASD. |
| fNIRS neuroscience | not done | Physiology field out of scope; redirect to SNIRF/MNE-NIRS. No fNIRS samples are present; current `.hdr` are ENVI and should not be routed by extension alone. |
| PP Systems UniSpec DC | partial | The synthetic `.SPU` is locked by golden and semantic test on `nm` axis, metadata header, `channel_a_dn`/`channel_b_dn` raw and reflectance. A real field acquisition is missing to validate the two channels and UniSpec DC metadata. Local Arctic LTER indices are derived products, not raw UniSpec spectra; the reader now refuses them with the diagnosis `pp-systems-unispec-derived-indices`. |
| PP Systems UniSpec SC | partial | The synthetic `.SPT` is locked by golden and semantic test on `nm` axis, metadata header, `dn_white`/`dn_target` raw and reflectance. A real field acquisition is missing to validate UniSpec SC headers, units and metadata. Local Arctic LTER indices are derived products, not raw UniSpec spectra; the reader now refuses them with the diagnosis `pp-systems-unispec-derived-indices`. |
| Avantes AvaSoft 8 binary | partial | `.Raw8` and `.IRR8` are covered by fixtures/goldens/semantic tests and probe (`AVS84`, modes 0/4). In addition to the SPC date/time, the reader now promotes `measurement_mode`, `point_count`, `first_pixel`/`last_pixel`, `integration_time_ms`, `averages_count`, `integration_delay`, `magic` and, when the slot is full, `instrument_serial`, `operator`, `comment` at the top-level, keeping `metadata.avantes` for raw provenance. For `.IRR8`, the 4th vector is now exposed under `irradiance_calibration` (and no longer `white_reference`), with warning `avantes_avasoft8_extension_mode_mismatch:*` when the extension contradicts the `measure_mode`. Fixed ASCII strings (`spec_id`, `user_name`, `comment`) are trimmed at the first NUL to avoid binary trailers. What remains are `.RWD8/.ABS8/.TRM8/.RFL8/.RIR8/.RMN8/.RMD8`, multi-subfile AVS8 and complete irradiance calibration for `.IRR8`. |
| Metrohm Vision / Vision Air | partial | The synthetic CSV Vision Air is locked by golden and semantic testing on 50 records, `nm` axis, absorbance signal and `protein`/`moisture`/`fat` targets. A real client export, a reference comparison is missing and the native project database remains closed. |
| HDF5 NIRS generic | partial | Multi-signal `spectra+wavelengths` schemes, nested groups, common aliases (`absorbance`, `reflectance`, `data`, `wn`, etc.) and unambiguous `bands_by_samples` matrices are covered by fixtures; non-spectral refusals remain locked. Real schemas with rich metadata, complex axes, non-trivial targets and heterogeneous group conventions are missing. |
| Spectro Inc. SiWare API | partial | Native JSON `measurement.wavelengths`/`measurement.absorbance` and CSV axis-first are locked by goldens/semantic tests. The fixtures remain synthetic; a real API response, schema variants and a comparison reference on predictions, units and optional metadata are missing. |
| ASD FieldSpec | partial | Revisions 1/6/7/8 primary spectra covered by six committed fixtures with direct semantic tests; undecoded internal block bytes are exposed via `metadata.asd.trailing_block_bytes`. Remaining possible v3/v4/v5, secondary/dependent/reference/calibration internal blocks, audit/signatures and separate calibration companions `.ILL/.REF/.RAW`. |
| Avantes AvaSoft 6/7 binary | partial | Two `.TRM` fixtures and `.ROH/.DRK/.REF` modes are golden-backed with semantic tests and probes locked for each available suffix. The reader promotes `measurement_mode`, `point_count`, `first_pixel`/`last_pixel`, `integration_time_ms`, `averages_count`, `integration_delay`, `detector_temperature_c`, `version_id` and, when the slot is filled, `instrument_serial`/`operator` to the top-level, keeping `metadata.avantes` for raw provenance (including axis coefficients, native `measure_mode` and `smooth_pixels`/`trigger`). Single-channel modes `.ROH/.DRK/.REF` are annotated with `avantes_legacy_single_channel:<mode>:companion_files_required` to signal to consumers that companion files are needed to recompose transmittance/absorbance. Remain `.ABS` and other legacy binary modes then `lightr` comparison; the `.IRR` present is an ASCII export covered by Avantes ASCII, not a proof of the legacy binary. |
| BUCHI NIRCal | partial | The `.nir` path reads spectra/wavenumbers/properties, GUID/project version, replica indexes and metadata `Spectra Info` per spectrum: spectrum GUID, scans/resolution, declared wavenumber geometry, timestamps, creator/modifier, device/serials, cell/option and gain/temperatures when present. Non-zero targets are validated locally on `transpec_DEMO_cannabis.nir`, and zeros remain numeric whenever a property table contains true values. What remains is a redistributable fixture with non-zero targets, `.cal` calibration-only, JCAMP-DX exports and NIRMaster/NIRFlex variants. |
| JCAMP-DX | partial | XYDATA/AFFN/ASDF/NTUPLES/LINK Ocean Optics and PEAK TABLE/PEAK ASSIGNMENTS top-level are covered, including multi-block files (`nist_sucrose_ir.jdx` -> 2 records), NTUPLES FID with `time` axis, line X checkpoint checking and sparse `peak_intensity` fixture. Remain general `LINK` with heterogeneous semantics, real peak tables for conformance and more NTUPLES variants. |
| Si-Ware NeoSpectra | partial | Reels OSSL Woodwell + UvA forensic XLSX commits and locks by reading tests + probe; the non-spectral OSSL descriptor is explicitly refused. It remains to cover a NeoSpectra Scanner native single-measurement export. |
| Spectral Evolution / PSR | partial | PSR DN brett + PSR-3500 grape leaf real committes; reflectance `%`/fraction and DN are types (`%`, `1`, `DN`), instrument/model/serial/mode/range/signals source/GPS/date/time, detector channels, temperatures ref/target, integrations ref/target, batteries, averages, dark mode, foreoptic and `declared_column_count` are promoted. The DN-only broken-but-valid remains signaled by `sed_missing_reflectance_signal` / `missing_reflectance_signal`, and `Channels`/`Columns` inconsistencies produce dedicated warnings. What remains are SR-3500 / SR-6500 firmware specifications, explicit radiance/irradiance and `spectrolab`/`specdal` conformance. |
| SVC / GER SIG | partial | The 15 committed fixtures are golden-backed with direct semantic assertions for SVC laptop, SVC PDA Acer clean/white-reference, matched-overlap-corrected, two BAD declared, GER 3700 PDA and BEO HR-1024i field. The reader now promotes `instrument_model`/`instrument_serial` (HI: serial (model)), `foreoptic`, integration time/coadds/temperatures by Si/InGaAs1/InGaAs2 detector and by reference/target scan, `battery_voltages_volts`, `error_codes`, `memory_slots`, `radiometric_factors`, `overlap_policy`, `matching_type` and `overlap_break_wavelengths_nm` taken from the `factors=` bracket. Quality flags `detector_overlap_preserved` (raw PDA / laptop), `white_reference` (`_WR_`) and `resampled_export` are added to `matched_overlap_corrected` / `overlap_removed`. What remains are the HR-1024i firmware >=3.0, the calibrated physical radiance unit when provided by the seller and the automated `spectrolab`/`specdal` byte-level comparisons. |
| VIAVI MicroNIR | partial | Reels CSV/XLSX MicroNIR 1700 committed and locked by reading + probe tests (UvA forensic). The native `.sam` (`MNIR` .NET container) is now decoded by the `viavi_micronir` reader: a record with `absorbance` signal (125 channels, `nm` axis interpolated between the stored terminals ~908.1-1676.2) and `raw_single_beam` (128 detector pixels, Index axis), rich metadata (serial, operator, integration, scans, timestamp). Sniff magic `\x04MNIR` (Definite) which resolves the Galactic-SPC false positive. Golden synthetic `samples/viavi_micronir/synthetic_micronir.sam` (CC0); real local-only fixtures. The native `.pri` project remains out of reach (customer-only). |
| ADF Allotrope | partial | `samples_local/allotrope_adf/adfsee_example.adf` validates ADF detection, numeric `/data-cubes`, cube titles, time axis `SecondTimeValue` type `time`, secondary scale `NanometerValue` and measurements `AbsorbanceUnitValue`. What remains is the complete Allotrope ontology, vendor exports, SDK validation and a redistributable CI fixture. |
| Horiba LabSpec / JobinYvon | partial | `.l6m` real Gd₂O₃/AlN map decodes in experimental mode and compares fully against the text export (intensities + coordinates); the XML `eV` axes are `energy` types, and the XML range/linescan branches are locked by semantic tests. Remains `.l6s`, other LabSpec6 layouts and complete metadata. |
| WiTec WIP/WID | partial | Real `Sa4.wip` decodes into 4410 TDGraph `WIT_PR06` spectra, with strict Boolean `LineValid` validation, Raman-shift axis derived from `ExcitationWaveLength`, physical coordinates derived from `SpaceTransformationID`, 4950 physical slots, 49 valid lines and 6 invalid lines. Remaining general WiTec layouts and equivalent ASCII export for comparison. |
| AnIML | partial | Synthetic spectral `SeriesSet` are covered with explicit values ​​and uniform axis `AutoIncrementedValueSet`; `Example3.animl` is a real non-spectral AnIML sample refuses as expected. Remain true spectral AnIML, non-zero segmented indices, XSD validation and conformance with AnIML tooling. |
| FGI HDF5 + XML | partial | Synthetic XML sidecar maps to HDF5 and dual provenance; It remains to validate an FGI real pair and the complete XML schema. |
| Bruker OPUS native | partial | The entire corpus commit `samples/bruker_opus/` is golden-backed and the remaining cross-reader fixtures have direct semantic tests: spectral-cockpit/opusreader2, pierreroudier/opusreader, brukeropus MIT, SpectroChemPy and cran soil.spec AfSIS/MPA. OPUS `MIN` axes are now `time` types when encounters. What remains is OPUS 5/6 legacy archives, 2D/imaging blocks and automated full-array conformance against external readers. |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | partial | The 12 Ocean Optics committes samples are golden-backed: texts SpectraSuite/OceanView/Jaz/JazIrrad/CRAIC/CSV/Master.Transmission, ProcSpec Linux/Windows types transmittance and white-reference type reflectance via XML core processor / `yUnits`, JCAMP LINK via `jcamp-dx` and `.spc` OceanView Galactic route. What remains are QE Pro/Maya/Apex, true `.spc` Ocean non-Galactic, typing of Jaz/generic texts without explicit metadata and `lightr`/`pavo` reference reports. |
| Thermo / Galactic GRAMS SPC | partial | Golden coverage extended to the open IR/Raman/UV-vis/NIR/NMR-FID corpus, with direct semantic tests for multi-subfile generated-X, directory-backed `TXYXYS`, old ordered-Z limit, minute/second SPC axes `time` types on `s_xy.spc` and `NMR_FID.SPC`, and metadata `data_layout` exposed for layouts single/common/independent-X. What remains is new big-endian `0x4C`, old headers/complete logs and final scope decision for NMR/FID. |
| Thermo Nicolet OMNIC | partial | SPA/SPG/SRS TGA-GC are locked by goldens/semantic tests on the committed corpus, including 2D matrix, offsets and metadata `series_y_*`; the three SpectroChemPy local `.srs` cover `tg_gc`, `rapid_scan_raw` and `rapid_scan_reprocessed`. Remains `.srsx` and more high-speed variants. |
| Bruker Tango / MPA / Matrix | partial | AfSIS Bruker MPA `icr_*.0` real committes (cran/soil.spec). What remains is dedicated Bruker Tango FT-NIR and complete MPA/Matrix metadata. |
| ENVI / hyperspectral cubes | partial | ENVI Standard `.hdr` and direct input `.img/.dat` are loaded in per-pixel spectra with `map info` parse, normalized spatial unit, projection/reference/pixel-size and order `row_slowest_x_fastest`; ENVI Standard and AVIRIS/Indian Pines `.lan/.spc/.GIS` now accept rectangular ROIs `rows/cols` and sparse masks `(row, col)` (order preserving, duplicates allowed) in Rust API (`CubeWindow`/`CubeMask`) and CLI (`--rows`/`--cols`, `--pixel`/`--pixels-file`), and the local-only MATLAB cube `indian_pines_corrected.mat` is also covered. What remains is generic ERDAS LAN, NEON/Specim/HySpex/Headwall and HDF5 cubes. |
| JASCO JWS | partial | The OLE2 `DataInfo`/`Y-Data` FT/IR transmittance, FP-8300 fluorescence and CD-1500/J-1500 CD/HT/Abs fixtures are locked by goldens/semantic tests and probe; JASCO text export is covered by `row-spectral-table`. What remains are distinct V-series NIR blocks, Raman NRS variants and alternative streams (`Data`, `Header`, `XdataValue`). |
| MATLAB MAT/RData | partial | Simple MAT v5/v7.3, academic DSO, `NIRsoil.RData` prospectr and local-only Indian Pines cube are covered; remain generic MAT/RData structures, MAT v7.3 cubes and heterogeneous metadata/targets. |
| Perkin Elmer Spectrum / IR | partial | The real single-spectrum `.sp` PEPE `specio` is golden-backed and semantically tested. The reader now tolerates the residual top-level blocks that FT-MIR Spectrum 10 exports add after the root block (audit block/CustomColumn), indicated by the warning `perkin_elmer_reverse_engineered_trailing_blocks` without affecting the spectral data; 31 valid `.sp` FT-MIR real vs their CSV exports (~5e-5). Hardened robustness (negative point count rejected, block recursion capped — clean errors, never panic). Golden synthetic `samples/perkin_elmer/synthetic_trailing.sp` (CC0); real local-only MIR corpus. The status remains partial because the line includes `.fsm` Spotlight imaging, detects/refuses as outside v1, and because PE NIR/Lambda variants are missing. |
| Renishaw WDF | partial | The 15 versioned spectral fixtures cover single, map, line, depth/zscan, FocusTrack, time-series, StreamLine and interrupted; both fixtures `measurement_type=0` are expected rejections. PSET observed `MAP`s now expose inventory + `dataRange` derived by record when length matches the number of spectra, and map/depth `dataRange` fixtures are golden-backed. What remains are other `MAP` layouts, authoritative derived units/algorithms, full-array conformance and fixtures by InVia Qontor/Apollo model. |
| Shimadzu UVProbe | partial | The synthetic `.txt` UVProbe is locked by golden/semantic test on `nm` axis, signal `sample_s000` and title `Spectrum Data`; the registry also tests that `.spc` is not claimed by extension alone. Remain true `.txt`, true `.spc` Shimadzu and converter/`pyfasma-spc` comparison. |
| Felix Instruments F-750 | partial | The CSV DataViewer absorbance (mango DMC, CC-BY 4.0, Mendeley 10.17632/46htwnp833.1) is golden-backed via `csv_like` (`csv_felix_f750_mango_slice`, 26 records, axis `nm` 285-1200, 306 points, target `DM`, metadata cultivar/region/season). Like VIAVI MicroNIR and NeoSpectra, no dedicated reader is required. There remain the DataViewer Raw-Spectra (reflectance) and Interpolated-Spectra (2nd derivative) modes, and the native on-device store. |
| ASM allotrope | partial | All three spectral Benchling fixtures/endpoints are covered; remain multiple seller conversions, ASM cases outside the platform-reader and validation against Allotrope tooling. |
| Generic NetCDF NIRS | partial | The synthetic `spectra+wavelengths` scheme, Microtops MAN, local ARM MFRSR and local SURFSPECALB derivative are covered; PyrNet and AOSMET are non-spectral expected rejections. What remains are real generic NIRS schemes, more robust QC NetCDF4/HDF5 and ACT/xarray validation. |
| MFR Sun Photometer | partial | The synthetic `.OUT` validates the text parser; the local MFRSR NetCDF ARM is decoded into 4,320 records x 7 filters with hemispheric/diffuse/direct/alltime/ratio signals, QC NetCDF and YAML sidecar of suspect/incorrect ranges. What remains is a redistributable MFR-7/MFRSR dump, a broader ARM mapping (`_FillValue`, calibration, filters) and ACT/xarray comparison. |
| Microtops Sun Photometer | partial | MAN NetCDF real commit and test (PANGAEA MSM114/2, CC-BY-4.0). The generic discovery `aot_<nm>` remains; when `hdf5-reader` 0.5 fails to resolve the shared attributes of this layout, the reader switches to a generic decoder based on `DataLayout::Contiguous` (scan of link records of the fractal heap + `get_or_parse_header(addr)` + raw reading), without hash table or fixture offsets. It emits `microtops_man_netcdf_contiguous_layout_fallback` and, for fixed global attributes string, `microtops_man_netcdf_global_attributes_byte_scan`. The 7 local AERONET MAN ASCII exports `.lev10/.lev15/.lev20` are tested with AOD and AOD-STD; the primary AOT signals are type `aerosol_optical_thickness`, and `aot_std` is type `uncertainty`. What remains is a true redistributable legacy `.TXT` and the complete disappearance of the fallback once `hdf5-reader` correctly resolves the NetCDF4 shared attributes. |
| Excel spectral | partial | `.xlsx` synthetic/multi-sheet/real UvA and `.xlsm` macro-compatible OOXML are golden-backed; workbooks metadata-only AuroraNIR/Foss XDS explicitly refuses. What remains is `.xls` legacy OLE, a real `.xlsm` with macros if VBA metadata is needed, no more real multi-sheet fixtures and cases where Excel converts wavelengths into dates. |
| USGS SPECPR / PRISM / ECOSTRESS text | partial | ASCII `.asc`, ECOSTRESS/ASTER `.spectrum.txt` and AREF single-column are covered; remain the SPECPR binary and true axes for AREF dumps without sidecar. |
| DigitalSurf MountainsMap | partial | RosettaSciIO spectrum, multi-spectrum, hyperspectral maps, surface and zlib compressed/uncompressed golden-backed fixtures. Maps now expose `map_x_index`/`map_y_index` and `map_axis_order`; surfaces expose `spatial_y_index`, X/Y units and `surface_axis_order`. No known sample blocking; remain full-array conformance against `rsciio.digitalsurf`, richer object/comment metadata and perimeter decision for MountainsMap variants outside corpus/branded AFM-Raman. |
| Princeton TriVista TVF | partial | Covered and golden-backed RosettaSciIO corpus, including single/multi-frame, time series, line/map, multi-spectrometer and Step-and-Glue. The spectral axis is derived from `xDim/Calibration`, `xDim@Length` and `Frame@xDim` are valid, numbered metadata detector/spectrometers are promoted, and absent spatial units remain explicit (`unknown`) instead of being invented. No known sample blocking; remain automated full-array conformance against `rsciio.trivista`, richer objective/hardware-branch metadata and scope decision for non-corpus variants. |
| Hamamatsu HPD-TA IMG | partial | Adjacent 2D HPD-TA fixtures are covered, with time-calibrated Y axes exposed in `time` metadata and uncalibrated detector axes kept in `index`. Remain explicitly adjacent until none spectral export point-sample Hamamatsu is targeted. |
| MODTRAN albedo | partial | The synthetic `.dat` validates axis-first; a clearly licensed redistributable MODTRAN output is missing. |

## Local Corpus Verification (2026-05-20)

Last CLI sweep after updating the matrix. The counters below
relate to files evaluable by the CLI: `README`, licenses, PDF,
archives brutes, sidecars de documentation et YAML de QC sont exclus du
denominateur.

| Corpus | OK | Refus attendus | Refus inattendus | Notes |
|---|---:|---:|---:|---|
| `samples/` | 245 | 20 | 0 | Les refus attendus sont des formats volontairement non-NIRS, des fixtures negatives, des sidecars seuls (`92AV3C.spc`, `92AV3GT.GIS`, header Microtops), des workbooks metadata-only accompagnateurs, des rapports cible-seule Foss/Perten ou des descripteurs non spectraux (`neospectra_ossl_column_names.csv`). |
| `samples_local/` | 15 | 5 | 0 | Lectures OK: Indian Pines MATLAB v5, BUCHI cannabis, ARM MFRSR NetCDF + sidecar QC YAML, ARM SURFSPECALB derive, Allotrope ADF adfsee, 3 OMNIC `.srs` locaux et 7 exports Microtops MAN ASCII `.lev*`. Refus attendus: `_gt.mat` sidecar, NOAA `.lev2`, ARM AOSMET et PP Systems indices non raw/derives. |

## Public Sample Sweep (2026-05-20)

Online search for redistributable fixtures for `blocked` formats /
`partial`. Resultats:

### New Committed Fixtures

| Format | File adds | Source | License | Matrix effect |
|---|---|---|---|---|
| AVIRIS / hyperspectral cubes | `samples/hyperspectral_cubes/92AV3C.lan`, `92AV3C.spc`, `92AV3GT.GIS` | Public Indian Pines / AVIRIS fixture already mirrored locally | dataset terms to confirm before release | `partial` (`92AV3C` ERDAS LAN decode experimental) |
| Excel spectral | `samples/excel/scio_forensic_P_avg.xlsx`, `nirone_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — Consumer Physics SCiO + Spectral Engines NIRone 2.0 | CC-BY-4.0 | `partial` (synthetique seul) → `partial` (vrais XLSX vendeurs handheld) |
| Foss / WinISI / DS exports | `samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv`, `foss_xds_barleyground_sensAIfood.csv` (+metadata) | [Zenodo 16759587](https://zenodo.org/records/16759587) — sensAIfood Univ. Cordoba (Foss XDS XM-1000 + NIRSYSTEM-5000) | CC-BY-4.0 | `partial` → `done` |
| Horiba LabSpec / JobinYvon | `samples/raman_horiba/AlN_Gd2O3_indepth.l6m` | [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) | MIT | `partial` (XML/TXT seul) → `partial` (`.l6m` decode experimental) |
| Si-Ware NeoSpectra | `samples/siware_neospectra/neospectra_ossl_column_names.csv`, `neospectra_ossl_50samples_slice.csv`, `neospectra_forensic_K_avg.xlsx` | [Zenodo 13122321 OSSL](https://zenodo.org/records/13122321) + [Figshare 21252300 UvA forensic](https://doi.org/10.21942/uva.21252300) | CC-BY-4.0 | `partial` (synthetique seul) → `partial` (vrais clients OSSL + forensique) |
| Tables spectrales delimitees (handheld) | `samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv` (+metadata) | [Zenodo 15838272](https://zenodo.org/records/15838272) — sensAIfood Grainit (AuroraNIR 950-1650 nm) | CC-BY-4.0 | bonus handheld miniaturise |
| VIAVI MicroNIR | `samples/viavi_micronir/micronir_forensic_K_avg.xlsx`, `micronir_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — MicroNIR 1700 forensique UvA | CC-BY-4.0 | `partial` (synthetique seul) → `partial` (CSV/XLSX real) |
| WiTec WIP/WID | `samples/raman_witec/Sa4.wip` | [Zenodo 7907659](https://zenodo.org/records/7907659) — Raman analysis ZrO₂ | ODbL v1.0 | `partial` (ASCII only) → `partial` (`WIT_PR06` TDGraph decodes experimental with Raman axis and map coordinates) |

### Public Sample Sweep (2026-05-20 — second passage)

After the first pass, extensive search on PANGAEA, GitLab Allotrope,
github.com/pierreroudier/opusreader, github.com/joshduran/brukeropus,
github.com/cran/soil.spec, github.com/serbinsh/R-FieldSpectra,
github.com/capstone-coal/pycoal, github.com/hdeneke/PyrNet,
github.com/kebasaa/SCIO-read, ehu.eus/ccwintco (Indian Pines), NOAA Lauder.

#### New committed fixtures (second pass)

| Format | File adds | Source | License | Matrix effect |
|---|---|---|---|---|
| Bruker OPUS native (cross-reader) | `samples/bruker_opus/brukeropus_file.0`, `opusreader_test_spectra.0`, `icr_087266_B2.0`, `icr_087273_G3.0` | [`joshduran/brukeropus`](https://github.com/joshduran/brukeropus) (MIT), [`pierreroudier/opusreader`](https://github.com/pierreroudier/opusreader) (GPL-3), [`cran/soil.spec`](https://github.com/cran/soil.spec) AfSIS (GPL-2/3) | mixed (MIT + GPL) | remains `partial` but expanded cross-vendor coverage |
| Consumer Physics SCiO | `samples/scio/scio_app_scan.csv`, `scio_calibration_plate_Polypen.csv`, `scio_scans_from_tech_support.csv` | [`kebasaa/SCIO-read`](https://github.com/kebasaa/SCIO-read) | GPL-3 | `done`: `band*`, axis-first calibration and `spectrum`/`wr_raw`/`sample_raw` groups tested; also adds `excel/scio_forensic_*.xlsx` UvA Figshare in addition |
| ENVI Spectral Library | `samples/envi_sli/usgs_splib06a_aviris95_envi.sli|hdr` + `usgs_splib07_aviris95_envi.sli|hdr` | [`capstone-coal/pycoal`](https://github.com/capstone-coal/pycoal) | GPL-2 (wrapper) + USGS public domain (data) | `partial` → `done` |
| Microtops Sun Photometer | `samples/microtops/microtops_arc_msm114_2.nc` + `_header.txt` | [PANGAEA 966645](https://doi.pangaea.de/10.1594/PANGAEA.966645) (republished from AERONET MAN) | CC-BY-4.0 | `partial` (synthetic only) -> `partial` (real NetCDF MAN tested, AOT type, generic contiguous fallback based on `DataLayout` + scan link records of the fractal heap, plus none SHA-256 template nor fixture offsets); legacy `.TXT` and full pass through high-level `hdf5-reader` still pending |
| NetCDF NIRS-adjacent | `samples/netcdf/pyrnet_to_l1a_output.nc` | [`hdeneke/PyrNet`](https://github.com/hdeneke/PyrNet) | academic share | refusal non-NIRS tests: no spectral axis or channels Microtops AOT |
| Spectral Evolution / PSR | `samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | remains `partial`, PSR-3500 firmware variant adds |
| SVC / GER SIG | `samples/svc_ger/serbinsh_gr070214_003.sig`, `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | GER 3700 PDA + HR-1024i Barrow firmware variants ajoutees |

#### Fixtures non-redistribuables (uniquement en local — `samples_local/`, gitignore)

| Format | File | Source | License/non-commit reason | Effect |
|---|---|---|---|---|
| Allotrope ADF adfsee | `samples_local/allotrope_adf/adfsee_example.adf` | [`allotrope-open-source/adfsee`](https://gitlab.com/allotrope-open-source/adfsee) | ADF/ontology terms Allotrope, keep local | experimental ADF reader tested: 4 records from 3 digital data-cubes; Minimal RDF maps titles, time axis type `time`, secondary scale nm and absorbance mAU |
| Adjacent ARM MFRSR / ARM NetCDF | `samples_local/mfr/*.nc`, `samples_local/netcdf/*.nc` | DOE ARM / ARM test data | ARM Data Use Policy -> local only | MFRSR b1 local decodes into 4,320 observations x 7 filters with QC YAML sidecar; SURFSPECALB local decodes into 986 useful lines x 6 filters; AOSMET remains non-spectral |
| BUCHI NIRCal cannabis | `samples_local/buchi_nircal/transpec_DEMO_cannabis.nir` | orellano-c/transpec_info | unclarified license for redistribution of the fixture -> locally only | reader local tests: 105 spectra, axis 1501 wavenumbers, 35 groups of 3 replicates, non-zero targets `CBDA`/`THCA`, comments/timestamps/device/serials and path without gain/temperature |
| Hyperspectral cube (AVIRIS Indian Pines) | `samples_local/hyperspectral_cubes/indian_pines_corrected.mat` + `_gt.mat` | [EHU/Grupo de Inteligencia Computacional](http://www.ehu.eus/ccwintco/index.php/Hyperspectral_Remote_Sensing_Scenes) | "academic use" without clear SPDX → local only | reader MAT v5 local-only tests: 21,025 spectra x 200 bands + target `land_cover_class`; the smaller version `92AV3C.lan` remains committee |
| Microtops `.lev2` disambiguation | `samples_local/microtops/noaa_lauder_sonde_la20170315.lev2` | [NOAA GML Lauder](https://gml.noaa.gov/aftp/data/ozwv/WaterVapor/Lauder_LEV/) | US Gov public domain BUT the file is actually a water vapor/ozone radiosonde, not a Microtops sun-photometer | local disambiguation help `.lev2`; not committed to avoid confusion |
| Microtops MAN ASCII Okeanos | `samples_local/microtops/aeronet_man_Okeanos_19_2_*.lev10/.lev15/.lev20` | AERONET Maritime Aerosol Network | AERONET MAN PI/coauthorship policy -> local only | local reader tested: valid AOD types `aerosol_optical_thickness`, channels `-999` omitted, AOD-STD for daily/series exports |
| PP Systems Arctic LTER indices | `samples_local/pp_systems/*.csv/.xlsx` | Arctic LTER/EDI | uncommitted local dataset | derived product NDVI/EVI/PRI/WBI/Chl/LAI + metadata; explicit deny `pp-systems-unispec-derived-indices`; doesn't close lack of raw `.SPT/.SPU` or reflectance wavelength-indexed table |
| Thermo Nicolet OMNIC SRS premises | `samples_local/nicolet_omnic/spectrochempy_TGA_demo.srs`, `spectrochempy_rapid_scan.srs`, `spectrochempy_rapid_scan_reprocessed.srs` | [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) | CeCILL-B but large files -> local only | TGA_demo absorbance, rapid-scan raw interferogram/index and rapid-scan reprocessed absorbance are tested locally; `.srsx` remains missing |

### Formats remaining closed (sweep without usable results, after 3 passes)

| Format | Why not find |
|---|---|
| Allotrope ADF seller | The local `adfsee` sample closes the missing "none ADF"; remain the vendor instrumental ADFs (Waters/Sciex/Agilent/etc.), the complete ontology, the units and the Allotrope SDK validation. |
| ASD calibration `.ILL/.REF/.RAW` | SDK vendor distribution only; SPECCHIO partial behind login partnership; none GitHub/Wayback/Mendeley sample. |
| Bruker OPUS 5/6 legacy | Private archives, no public mirror; OPUS 7/8 covered via 4 independent readers is enough. |
| Foss `.NIR/.DA/.cal/.eqa` native | Native `.cal`/`.nir` decodes (reader `foss_winisi`), including DS-series DS2500/DS3F benches (`foss-ds-nir`), valid vs export ISIscan `.txt` where it exists, but on real fixtures local-only (license to be defined); no public redistributable binary fixtures found (only `synthetic_ds*.nir` CC0 are committed). `.DA`/`.eqa` not sampled. |
| Horiba `.l6s` single-spectrum | No public fixtures found; only `.l6m` (map) commits. |
| JASCO V-780 NIR/NRS Raman `.jws` variants | No separate samples of the V-770 IR + V-series UV-Vis already committed. |
| Metrohm Vision Air / OMNIS NIR native | Format ferme, seul l'export CSV est documente publiquement. |
| MFR-7 / MFRSR `.OUT` real | ARM Data Center requires account; `samples_local/mfr/` locally closes a NetCDF ARM MFRSR b1, but not a redistributable MFR-7 `.OUT` — uncommit. |
| Microtops II `.TXT` real | AERONET MAN requests co-authorship; `samples_local/microtops/` locally closes MAN ASCII `.lev*` exports, but not a legacy redistributable `.TXT` — not commit. |
| MODTRAN albedo `.dat` real | Distribution under license MODTRAN/ONTAR ($2400); MIT OCW pcmodwin/RIT tutorials only ship USGS references already covered. |
| NEON AOP HDF5 reflectance tile | Tiles 1 km × 1 km require neon.science registration (free account but conditional distribution); minimum file ~50 MB. |
| Perten DA / Inframatic | No native fixture or real public CSV (clients only). |
| PP Systems UniSpec `.SPT/.SPU` real raw | No public raw `.spu/.spt` fixtures; `samples_local/pp_systems/` contains only Arctic LTER derived indices — not committed. The workbook references a separate reflectance data scan file that is not present locally. |
| Shimadzu UVProbe `.spc` native | Only one candidate (`uri-t/shimadzu-spc-converter`) without a clear license; aucune autre source apres sweep. |
| Si-Ware NeoSpectra Scanner native single-measurement | The OSSL pipeline only publishes wide matrices; no public “1 measurement per CSV” fixture. |
| Specim IQ demo cube | Specim has discontinued the product (“end-of-life” page); only the 7z archive Arabidopsis Zenodo 1345007 (123 MB) exists — too big, and the raw/processed mix is ​​not isolated. |
| Thermo OMNIC `.srsx` | No public fixture found (S.T.Japan demo libraries `.spg` behind form); the `.srs` channel, including local rapid-scan, is experimentally covered. |
| VIAVI MicroNIR `.pri` native | Binary project format, customer-only; the native `.sam` (sample) is decoded (reader `viavi_micronir`, fixtures local-only) and real CSV/XLSX exports are covered via UvA forensic. Only the `.pri` project remains unaffected. |
