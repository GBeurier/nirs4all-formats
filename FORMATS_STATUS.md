# Statut synthétique des formats (nirs4all-io)

Vue compacte de tous les formats de la lib, dérivée de
[`docs/FORMAT_MATRIX.md`](docs/FORMAT_MATRIX.md).

**Légende**

- `ok` — variants principaux lus, fixtures réelles, diffusable.
- `partiel` — lecteur réel utile mais périmètre ciblé / variants manquants.
- `limité` — parser synthétique seulement, ou détection/refus seulement.
- `rien` — format non décodé (natif fermé bloqué, ou hors-scope refusé).
- `sample_required` — `oui` si un échantillon réel diffusable manque encore (fixture synthétique ou aucune), `non` s'il existe déjà.

| Format | Constructeur | Version / variant | Extension | Status | sample_required |
|---|---|---|---|---|---|
| ASD FieldSpec | ASD / Malvern Panalytical | rév. 1/6/7/8 | `.asd` | ok | non |
| JCAMP-DX | IUPAC (neutre) | XYDATA/ASDF/NTUPLES/LINK/PEAK | `.jdx` `.dx` `.jcm` `.jcamp` | ok | non |
| Si-Ware NeoSpectra | Si-Ware | matrices OSSL / forensic | `.csv` `.xlsx` | ok | non |
| Spectral Evolution / PSR | Spectral Evolution | PSR, PSR-3500 | `.sed` | ok | non |
| SVC / GER SIG | Spectra Vista / GER | laptop/PDA, GER 3700, HR-1024i | `.sig` | ok | non |
| VIAVI MicroNIR | VIAVI / JDSU | MicroNIR 1700 (exports) | `.csv` `.xlsx` | ok | non |
| Bruker OPUS natif | Bruker | OPUS 7/8, MPA | `.0` `.1` `.001` … | ok | non |
| Ocean Optics / Ocean Insight | Ocean Optics | SpectraSuite/OceanView/Jaz/CRAIC/ProcSpec | `.txt` `.csv` `.jaz` `.ProcSpec` … | ok | non |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | new/old LSB | `.spc` | ok | non |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`/`.spg`/`.srs` (pas `.srsx`) | `.spa` `.spg` `.srs` | ok | non |
| JASCO JWS | JASCO | streams principaux | `.jws` `.txt` | ok | non |
| MATLAB MAT / RData | MATLAB / R | MAT v5/v7.3, RData | `.mat` `.RData` | ok | non |
| Renishaw WDF | Renishaw | InVia, MAP | `.wdf` | ok | non |
| Excel spectral | générique | `.xlsx`/`.xlsm` (pas `.xls`) | `.xlsx` `.xlsm` | ok | non |
| USGS SPECPR / PRISM / ECOSTRESS | USGS / JHU | texte (pas binaire SPECPR) | `.asc` `.txt` | ok | non |
| ENVI Spectral Library | L3Harris / ENVI | splib06/07 | `.sli` + `.hdr` | ok | non |
| DigitalSurf MountainsMap | DigitalSurf | — | `.sur` `.pro` | ok (adjacent) | non |
| Princeton TriVista TVF | Princeton Instruments | — | `.tvf` | ok (adjacent) | non |
| Foss / WinISI / DS exports | Foss | exports texte | `.txt` `.csv` | ok | non |
| Tables axe-first | générique | — | `.csv` `.tsv` `.dat` `.asc` … | ok | non |
| Tables spectrales délimitées | générique | — | `.csv` `.tsv` `.txt` | ok | non |
| Avantes ASCII | Avantes | exports AvaSoft | `.ttt` `.trt` `.IRR` `.txt` … | ok | non |
| Bruker OPUS DPT | Bruker | export ASCII | `.dpt` | ok | non |
| Consumer Physics SCiO | Consumer Physics | export developer app | `.csv` | ok | non |
| Matrices spectrales (wide) | générique | — | `.csv` `.txt` | ok | non |
| NumPy | NumPy | — | `.npy` `.npz` | ok | non |
| Parquet | Apache | — | `.parquet` | ok | non |
| IDL / ENVI texte | IDL / ENVI | — | `.txt` | ok | non |
| EMSA/MAS MSA | ISO / EMSA | — | `.msa` | ok (adjacent) | non |
| Hamamatsu HPD-TA | Hamamatsu | — | `.img` | ok (adjacent, hors-NIRS) | non |
| Avantes AvaSoft 8 binaire | Avantes | `.Raw8`/`.IRR8` ok, reste planifié | `.Raw8` `.IRR8` `.ABS8` … | partiel | non |
| Avantes AvaSoft 6/7 binaire | Avantes | `.TRM`/`.ROH`/`.DRK`/`.REF` (pas `.ABS`) | `.TRM` `.ABS` … | partiel | non |
| BUCHI NIRCal / NIRFlex | BUCHI / Bühler | `.nir` (pas `.cal`, ni variants NIRMaster) | `.nir` | partiel | oui |
| HDF5 NIRS générique | neutre | schéma canonique + alias | `.h5` `.hdf5` | partiel | oui |
| Horiba LabSpec / JobinYvon | Horiba | XML/TXT + `.l6m` exp. (pas `.l6s`) | `.xml` `.txt` `.l6m` | partiel | non |
| WiTec WIP / WID | WiTec | 1 layout map | `.wip` `.wid` `.txt` | partiel | non |
| Bruker Tango / MPA / Matrix | Bruker | MPA ok (Tango/Matrix à sourcer) | OPUS natif | partiel | oui |
| ENVI / cubes hyperspectraux | ENVI / Specim / AVIRIS… | ENVI Std + AVIRIS (pas Specim/HySpex/NEON) | `.hdr`+`.dat`/`.img` `.lan` | partiel | non |
| PerkinElmer Spectrum / IR | PerkinElmer | `.sp` mono (`.fsm` refusé) | `.sp` | partiel | non |
| Shimadzu UVProbe | Shimadzu | `.txt` (pas `.spc` natif) | `.txt` `.spc` | partiel | oui |
| Allotrope ASM | Allotrope / Benchling | Benchling ok | `.json` | partiel | non |
| NetCDF NIRS générique | neutre | schémas dédiés | `.nc` `.cdf` | partiel | oui |
| MFR Sun Photometer | Solar Light / YES | NetCDF ARM local (`.OUT` réel absent) | `.OUT` `.nc` | partiel | oui |
| Microtops Sun Photometer | Solar Light | MAN ASCII/NetCDF | `.TXT` `.nc` `.lev*` | partiel | non |
| Felix Instruments F-750 | Felix / CID Bio-Science | DataViewer CSV (absorbance), via `csv_like` | `.csv` | partiel | non |
| Ocean Optics Flame-NIR | Ocean Optics / Ocean Insight | InGaAs 950-1650 nm (OceanView), via `ocean_optics` | `.txt` `.csv` `.ProcSpec` | partiel | oui |
| Thermo Antaris II FT-NIR | Thermo Fisher | FT-NIR 1000-2500 nm (RESULT), via `nicolet_omnic`/`galactic_spc`/`csv_like` | `.spa` `.spg` `.spc` `.csv` | partiel | oui |
| PP Systems UniSpec DC | PP Systems | parser synthétique | `.SPU` | limité | oui |
| PP Systems UniSpec SC | PP Systems | parser synthétique | `.SPT` | limité | oui |
| Metrohm Vision / Vision Air | Metrohm | CSV synthétique (natif fermé) | `.csv` `.xlsx` | limité | oui |
| Spectro Inc. SiWare API | Spectro Inc. | fixtures synthétiques | `.json` `.csv` | limité | oui |
| Allotrope ADF | Allotrope | détection locale partielle | `.adf` | limité | oui |
| AnIML | IUPAC / ASTM | spectral synthétique | `.animl` | limité | oui |
| FGI HDF5 + XML | FGI | mapping synthétique | `.h5` + `.xml` | limité | oui |
| MODTRAN albedo | Spectral Sciences | synthétique, hors-scope | `.dat` | limité | oui |
| Foss NIRSystems / WinISI **natif** | Foss | binaire fermé non décodé | `.NIR` `.DA` `.cal` `.eqa` | rien | oui |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire non décodé | binaire `.csv` | rien | oui |
| ASD calibration | ASD / Malvern | compagnons absents | `.ILL` `.REF` `.RAW` | rien | oui |
| ANDI / NetCDF MS | ASTM | détecté/refusé (hors-scope) | `.cdf` `.nc` | rien | non |
| mzML / mzMLb | HUPO PSI | détecté/refusé (hors-scope) | `.mzML` `.mzMLb` | rien | non |
| fNIRS neuroscience | NIRx / SNIRF | hors-scope | `.snirf` `.nirs` `.wl1/2` | rien | non |

**Bilan** : ~30 `ok`, 17 `partiel`, 8 `limité`, 6 `rien`.