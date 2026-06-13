# Synthetic status of formats (nirs4all-formats)

Compact view of all formats in the library, derived from
[`docs/FORMAT_MATRIX.md`](docs/FORMAT_MATRIX.md).

**Legend**

- `ok` — main variants read, real fixtures, publishable.
- `partial` — useful real reader but targeted scope / missing variants.
- `limited` — synthetic parser only, or detection/refusal only.
- `none` — format not decoded (blocked closed native, or out-of-scope refused).
- `sample_required` — `yes` if a real redistributable sample is still missing (synthetic fixture or none), `no` if one already exists.

| Format | Manufacturer | Version / variant | Extension | Status | sample_required |
|---|---|---|---|---|---|
| ASD FieldSpec | ASD / Malvern Panalytical | rev. 1/6/7/8 | `.asd` | ok | no |
| JCAMP-DX | IUPAC (neutral) | XYDATA/ASDF/NTUPLES/LINK/PEAK | `.jdx` `.dx` `.jcm` `.jcamp` | ok | no |
| Si-Ware NeoSpectra | Si-Ware | OSSL / forensic matrices | `.csv` `.xlsx` | ok | no |
| Spectral Evolution / PSR | Spectral Evolution | PSR, PSR-3500 | `.sed` | ok | no |
| SVC / GER SIG | Spectra Vista / GER | laptop/PDA, GER 3700, HR-1024i | `.sig` | ok | no |
| VIAVI MicroNIR | VIAVI / JDSU | MicroNIR 1700 (exports) | `.csv` `.xlsx` | ok | no |
| Bruker OPUS native | Bruker | OPUS 7/8, MPA | `.0` `.1` `.001` … | ok | no |
| Ocean Optics / Ocean Insight | Ocean Optics | SpectraSuite/OceanView/Jaz/CRAIC/ProcSpec | `.txt` `.csv` `.jaz` `.ProcSpec` … | ok | no |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | new/old LSB | `.spc` | ok | no |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`/`.spg`/`.srs` (not `.srsx`) | `.spa` `.spg` `.srs` | ok | no |
| JASCO JWS | JASCO | main streams | `.jws` `.txt` | ok | no |
| MATLAB MAT / RData | MATLAB / R | MAT v5/v7.3, RData | `.mat` `.RData` | ok | no |
| Renishaw WDF | Renishaw | InVia, MAP | `.wdf` | ok | no |
| Excel spectral | generic | `.xlsx`/`.xlsm` (not `.xls`) | `.xlsx` `.xlsm` | ok | no |
| USGS SPECPR / PRISM / ECOSTRESS | USGS / JHU | text (not binary SPECPR) | `.asc` `.txt` | ok | no |
| ENVI Spectral Library | L3Harris / ENVI | splib06/07 | `.sli` + `.hdr` | ok | no |
| DigitalSurf MountainsMap | DigitalSurf | — | `.sur` `.pro` | ok (adjacent) | no |
| Princeton TriVista TVF | Princeton Instruments | — | `.tvf` | ok (adjacent) | no |
| Foss / WinISI / DS exports | Foss | text exports | `.txt` `.csv` | ok | no |
| Axis-first tables | generic | — | `.csv` `.tsv` `.dat` `.asc` … | ok | no |
| Delimited spectral tables | generic | — | `.csv` `.tsv` `.txt` | ok | no |
| Avantes ASCII | Avantes | AvaSoft exports | `.ttt` `.trt` `.IRR` `.txt` … | ok | no |
| Bruker OPUS DPT | Bruker | ASCII export | `.dpt` | ok | no |
| Consumer Physics SCiO | Consumer Physics | developer app export | `.csv` | ok | no |
| Spectral matrices (wide) | generic | — | `.csv` `.txt` | ok | no |
| NumPy | NumPy | — | `.npy` `.npz` | ok | no |
| Parquet | Apache | — | `.parquet` | ok | no |
| IDL / ENVI text | IDL / ENVI | — | `.txt` | ok | no |
| EMSA/MAS MSA | ISO / EMSA | — | `.msa` | ok (adjacent) | no |
| Hamamatsu HPD-TA | Hamamatsu | — | `.img` | ok (adjacent, non-NIRS) | no |
| Avantes AvaSoft 8 binary | Avantes | `.Raw8`/`.IRR8` ok, rest planned | `.Raw8` `.IRR8` `.ABS8` … | partial | no |
| Avantes AvaSoft 6/7 binary | Avantes | `.TRM`/`.ROH`/`.DRK`/`.REF` (not `.ABS`) | `.TRM` `.ABS` … | partial | no |
| BUCHI NIRCal / NIRFlex | BUCHI / Bühler | `.nir` (not `.cal`, nor NIRMaster variants) | `.nir` | partial | yes |
| HDF5 NIRS generic | neutral | canonical schema + aliases | `.h5` `.hdf5` | partial | yes |
| Horiba LabSpec / JobinYvon | Horiba | XML/TXT + `.l6m` exp. (not `.l6s`) | `.xml` `.txt` `.l6m` | partial | no |
| WiTec WIP / WID | WiTec | 1 map layout | `.wip` `.wid` `.txt` | partial | no |
| Bruker Tango / MPA / Matrix | Bruker | MPA ok (Tango/Matrix to be sourced) | OPUS native | partial | yes |
| ENVI / hyperspectral cubes | ENVI / Specim / AVIRIS… | ENVI Std + AVIRIS (not Specim/HySpex/NEON) | `.hdr`+`.dat`/`.img` `.lan` | partial | no |
| PerkinElmer Spectrum / IR | PerkinElmer | `.sp` mono (`.fsm` refused) | `.sp` | partial | no |
| Shimadzu UVProbe | Shimadzu | `.txt` (not native `.spc`) | `.txt` `.spc` | partial | yes |
| Allotrope ASM | Allotrope / Benchling | Benchling ok | `.json` | partial | no |
| NetCDF NIRS generic | neutral | dedicated schemas | `.nc` `.cdf` | partial | yes |
| MFR Sun Photometer | Solar Light / YES | ARM local NetCDF (real `.OUT` absent) | `.OUT` `.nc` | partial | yes |
| Microtops Sun Photometer | Solar Light | MAN ASCII/NetCDF | `.TXT` `.nc` `.lev*` | partial | no |
| Felix Instruments F-750 | Felix / CID Bio-Science | DataViewer CSV (absorbance), via `csv_like` | `.csv` | partial | no |
| Ocean Optics Flame-NIR | Ocean Optics / Ocean Insight | InGaAs 950-1650 nm (OceanView), via `ocean_optics` | `.txt` `.csv` `.ProcSpec` | partial | yes |
| Thermo Antaris II FT-NIR | Thermo Fisher | FT-NIR 1000-2500 nm (RESULT), via `nicolet_omnic`/`galactic_spc`/`csv_like` | `.spa` `.spg` `.spc` `.csv` | partial | yes |
| PP Systems UniSpec DC | PP Systems | synthetic parser | `.SPU` | limited | yes |
| PP Systems UniSpec SC | PP Systems | synthetic parser | `.SPT` | limited | yes |
| Metrohm Vision / Vision Air | Metrohm | synthetic CSV (closed native) | `.csv` `.xlsx` | limited | yes |
| Spectro Inc. SiWare API | Spectro Inc. | synthetic fixtures | `.json` `.csv` | limited | yes |
| Allotrope ADF | Allotrope | partial local detection | `.adf` | limited | yes |
| AnIML | IUPAC / ASTM | synthetic spectral | `.animl` | limited | yes |
| FGI HDF5 + XML | FGI | synthetic mapping | `.h5` + `.xml` | limited | yes |
| MODTRAN albedo | Spectral Sciences | synthetic, out-of-scope | `.dat` | limited | yes |
| Foss NIRSystems / WinISI **native** | Foss | `.cal`/`.nir` decoded (ISIscan + DS2500/DS3F); remaining `.DA`/`.eqa` | `.NIR` `.DA` `.cal` `.eqa` | partial | yes |
| Perten DA / Inframatic | Perten / PerkinElmer | binary not decoded | binary `.csv` | none | yes |
| ASD calibration | ASD / Malvern | companion files absent | `.ILL` `.REF` `.RAW` | none | yes |
| ANDI / NetCDF MS | ASTM | detected/refused (out-of-scope) | `.cdf` `.nc` | none | no |
| mzML / mzMLb | HUPO PSI | detected/refused (out-of-scope) | `.mzML` `.mzMLb` | none | no |
| fNIRS neuroscience | NIRx / SNIRF | out-of-scope | `.snirf` `.nirs` `.wl1/2` | none | no |

**Summary**: ~30 `ok`, 18 `partial`, 8 `limited`, 5 `none`.
