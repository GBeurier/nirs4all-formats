# Matrice compacte des formats

Statuts utilisés: `fait`, `partiel`, `pas fait`, `bloqué`.

| Nom | Vendeur | Extension | Version (si applicable) | Status nirs4allio | Lib référence |
|---|---|---|---|---|---|
| Tables spectrales delimitees | Generique | `.csv`, `.tsv`, `.txt` | en-tetes numeriques | fait | pandas, read.table, nirs4all CSVLoader |
| Tables axe-first | Generique / exports instrument | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | une colonne axe + signaux | fait | pandas, read.table |
| Matrices spectrales | Generique / Foss / Metrohm / VIAVI | `.csv`, `.txt` | un spectre par ligne | fait | pandas, read.table |
| Excel spectral | Generique / lab | `.xlsx`, `.xlsm`, `.xls` | xlsx/xlsm + descripteur axis/data OK; xls manquant | partiel | calamine, openpyxl, pandas, readxl |
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | revisions 1, 6, 7, 8 | partiel | asdreader, prospectr, spectrolab, specdal, pyASDReader |
| ASD calibration | ASD / Malvern Panalytical | `.ILL`, `.REF`, `.RAW` | compagnons calibration | bloqué | SPECCHIO, asdreader |
| Avantes AvaSoft 6/7 binaire | Avantes | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | legacy 6/7 | partiel | lightr |
| Avantes AvaSoft 8 binaire | Avantes | `.Raw8`, `.IRR8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8` | AVS8 | partiel | lightr, manuel AvaSoft |
| Avantes ASCII | Avantes | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR` | exports texte | fait | pandas, read.table |
| Bruker OPUS DPT | Bruker | `.dpt` | export ASCII OPUS | fait | pandas, read.table |
| Bruker OPUS natif | Bruker | `.0`, `.1`, `.001`, `.0000`, sans extension fixe | OPUS 7/8 (4 lecteurs) + Bruker MPA AfSIS soils; OPUS 5/6 manquant | partiel | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Bruker Tango / MPA / Matrix | Bruker | OPUS natif | meme famille OPUS | partiel | opusreader2, SpectroChemPy |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr`, `.slb` | BSQ float32/float64; USGS splib06/07 vendeur reels (AVIRIS95 grid) | fait | spectral, RStoolbox, pysptools |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5, `.lan`, `.mat` | ENVI Standard + AVIRIS 92AV3C ERDAS LAN + local Indian Pines MAT point extraction | partiel | spectral, rasterio, scipy |
| FGI HDF5 + XML | FGI | `.h5`, `.hdf5`, `.xml` | paire HDF5+XML synthetique mappee | partiel | h5py, hdf5r, rhdf5, lxml |
| MFR Sun Photometer | Solar Light / YES Inc. | `.OUT`, `.nc` local | MFR-7 `.OUT` synthetique + ARM MFRSR b1 NetCDF local decode | partiel | SPECCHIO, parseurs ad hoc, xarray, ARM ACT |
| Microtops Sun Photometer | Solar Light | `.TXT`, `.nc` (MAN export), `.lev10/.lev15/.lev20` local | CSV synthetique + MAN NetCDF PANGAEA avec fallback fixture + MAN ASCII AERONET local Okeanos | partiel | parseurs ad hoc, xarray |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.spc` | plusieurs familles texte + ProcSpec | partiel | lightr, pavo |
| PP Systems UniSpec SC | PP Systems | `.SPT` | export texte synthetique uniquement | partiel | SPECCHIO, parseurs ad hoc |
| PP Systems UniSpec DC | PP Systems | `.SPU` | export texte synthetique uniquement | partiel | SPECCHIO, parseurs ad hoc |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | PDA + laptop + GER 3700 PDA + HR-1024i field reels; BAD fixtures flagged | partiel | spectrolab, specdal |
| Spectral Evolution / PSR | Spectral Evolution | `.sed` | PSR DN brett + PSR-3500 grape leaf reels; DN-only flagged | partiel | spectrolab, specdal |
| MODTRAN albedo | Spectral Sciences / AFRL | `.dat` | sortie albedo synthetique uniquement | partiel | parseur texte |
| IDL / ENVI texte | IDL / ENVI | `.txt` | export axe-first | fait | parseur texte |
| USGS SPECPR / PRISM | USGS | `SPECPR`, `.asc`, `.txt` | ASCII `.asc` + AREF single-column; binaire manquant | partiel | convertisseur USGS |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc`, `.SPC` | new LSB golden elargi; old limite; BE manquant | partiel | spc-spectra, rohanisaac/spc, specio, SpectroChemPy, xylib, spc-parser |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs`, `.srsx` | spa/spg/TGA-GC srs OK; srsx manquant | partiel | SpectroChemPy, spa-on-python |
| Perkin Elmer Spectrum / IR | PerkinElmer | `.sp`, `.fsm` | sp OK; fsm imaging refuse | partiel | specio |
| Foss NIRSystems / WinISI natif | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | binaire ferme | bloqué | aucune fiable |
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | exports matrices WinISI/Foss XDS reels | fait | parseur texte |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, base projet native | CSV export synthetique OK; DB native manquante | partiel | parseur texte, pandas, readxl |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | fixture NIRCal avec cibles nulles + cannabis local avec cibles non nulles | partiel | prospectr::read_nircal |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire vendeur, `.csv` | binaire ferme; CSV cible seule refuse | bloqué | export CSV/Excel vendeur |
| JASCO JWS | JASCO | `.jws`, `.txt` | OLE2 DataInfo/Y-Data | partiel | jws2txt, jwsProcessor |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | texte OK; spc proprietaire manquant | partiel | pyfasma-spc, convertisseur Shimadzu |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.xlsx`, `.pri` | CSV/XLSX (UvA forensic 1700) OK; pri manquant | partiel | parseur texte, openpyxl |
| Si-Ware NeoSpectra | Si-Ware | `.csv`, `.xlsx` | OSSL Woodwell + UvA forensic XLSX OK | partiel | parseur texte, openpyxl |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | JSON/CSV synthetiques | partiel | JSON/CSV standard |
| JCAMP-DX | Vendor-neutral / IUPAC | `.jdx`, `.dx`, `.jcm`, `.jcamp` | XYDATA mono/multi-blocs, ASDF, NTUPLES, LINK partiel | partiel | jcamp, SpectroChemPy, nmrglue, ChemoSpec, hyperSpec |
| ANDI / NetCDF MS | ASTM / vendor-neutral | `.cdf`, `.nc` | detection + refus non-NIRS | fait | pyteomics, PyMassSpec, pyOpenMS |
| NetCDF NIRS generique | Vendor-neutral | `.nc`, `.cdf` | schema spectra+wavelengths synthetique + lecteurs dedies Microtops MAN / ARM MFRSR / ARM SURFSPECALB locaux + refus adjacents | partiel | netcdf-reader, xarray, netcdf, ARM ACT |
| AnIML | IUPAC / ASTM | `.animl` | spectral SeriesSet synthetique | partiel | animl-python, validateurs XML |
| Allotrope ASM | Allotrope / Benchling | `.json` | fixtures Benchling cubes/endpoints spectraux | partiel | Benchling allotropy |
| Allotrope ADF | Allotrope Foundation | `.adf` | HDF5/RDF, pas de sample | bloqué | Allotrope SDK |
| mzML / mzMLb | HUPO PSI / MS vendors | `.mzML`, `.mzMLb` | detection + refus non-NIRS | fait | pyteomics, pymzML, pyOpenMS |
| HDF5 NIRS generique | Vendor-neutral | `.h5`, `.hdf5` | schema spectra+wavelengths synthetique + refus non-spectral | partiel | h5py, hdf5-reader, tables |
| Parquet | Apache / generique | `.parquet` | table NIRS canonique | fait | pyarrow, fastparquet, nirs4all ParquetLoader |
| MATLAB MAT / RData | MATLAB / R ecosystem | `.mat`, `.MAT`, `.RData` | MAT v5/v7.3 + RData prospectr + local Indian Pines cube | partiel | scipy, hdf5-reader, R serialization, prospectr |
| NumPy | Python / NumPy | `.npy`, `.npz` | `.npy` matrice, `.npz` canonique | fait | numpy |
| Renishaw WDF | Renishaw | `.wdf` | spectra + metadata maps/images | partiel | RosettaSciIO, SpectroChemPy |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | XML/TXT OK; `.l6m` Gd₂O₃/AlN map decode experimental; `.l6s` manquant | partiel | RosettaSciIO, SpectroChemPy, horiba-raman |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | corpus XML Frame complet OK; refs externes a durcir | partiel | RosettaSciIO |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | corpus spectra/maps/surfaces OK; variantes hors corpus | partiel | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | corpus adjacent 2D OK; hors point-spectra NIRS | partiel | RosettaSciIO |
| WiTec WIP / WID | WiTec | `.wip`, `.wid`, `.txt` | `WIT_PR06` TDGraph Sa4 decode experimental + ASCII OK; autres layouts refuses | partiel | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | ISO 22029 XY/Y | fait | RosettaSciIO |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | hors scope NIRS spectroscopy | pas fait | MNE-NIRS, SNIRF |
| Consumer Physics SCiO | Consumer Physics | `.csv` (developer app) | 740-1070 nm handheld DLP-MEMS; CSV reel committe (`R`/`S`/`C` sections) | fait | kebasaa/SCIO-read |

## Notes pour les statuts non finis

Les lignes `fait` ne sont pas repetees ici. La note indique ce qui manque pour
passer le format a `fait`.

| Nom | Status nirs4allio | Note / manque |
|---|---|---|
| Excel spectral | partiel | Ajouter `.xls` legacy et plus de fixtures multi-feuilles reelles; XLSX axis/data descriptor et handheld UvA sont couverts. |
| ASD FieldSpec | partiel | Decoder les blocs reference/calibration et couvrir les revisions legacy. |
| ASD calibration | bloqué | Obtenir un jeu redistribuable `.asd` + `.ILL/.REF/.RAW`. |
| Avantes AvaSoft 6/7 binaire | partiel | Ajouter fixtures `.ABS/.IRR/.RMN` et comparaison `lightr`. |
| Avantes AvaSoft 8 binaire | partiel | Ajouter fixtures pour chaque suffixe AVS8 et valider les modes. |
| Bruker OPUS natif | partiel | OPUS 7/8 desormais teste via 4 lecteurs independants (spectral-cockpit, pierreroudier, brukeropus MIT, cran soil.spec AfSIS). OPUS 5/6 legacy archives + blocs 2D/imaging restent. |
| Bruker Tango / MPA / Matrix | partiel | AfSIS Bruker MPA `icr_*.0` reels committes (cran/soil.spec). Reste Bruker Tango FT-NIR dedie et metadata MPA/Matrix complete. |
| ENVI / hyperspectral cubes | partiel | ENVI Standard `.img/.dat + .hdr`, AVIRIS/Indian Pines `.lan/.spc/.GIS` et le cube MATLAB local-only `indian_pines_corrected.mat` sont charges en spectres par pixel; restent ERDAS LAN generique, NEON/Specim/HySpex/Headwall, HDF5 cubes et API masque/extraction. |
| FGI HDF5 + XML | partiel | Sidecar XML synthetique mappe vers HDF5 et provenance double; reste a valider une paire FGI reelle et le schema XML complet. |
| MFR Sun Photometer | partiel | Le `.OUT` synthetique valide le parseur texte; le MFRSR NetCDF ARM local est decode en 4,320 enregistrements x 7 filtres avec signaux hemispheric/diffuse/direct/alltime/ratio et QC. Restent un dump MFR-7/MFRSR redistribuable, un mapping ARM plus large (`_FillValue`, calibration, filtres) et comparaison ACT/xarray. |
| Microtops Sun Photometer | partiel | MAN NetCDF reel committe et teste (PANGAEA MSM114/2, CC-BY-4.0). Le reader tente une decouverte generique `aot_<nm>`, mais le payload MSM114/2 reste lu via fallback SHA-256 car `hdf5-reader` ne resout pas encore ce layout NetCDF4/HDF5. Les exports AERONET MAN ASCII `.lev10/.lev15/.lev20` sont testes en local avec AOD et AOD-STD. Restent un vrai `.TXT` legacy redistribuable et un lecteur MAN NetCDF generique sans fallback. |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | partiel | Ajouter variantes QE Pro/Maya/Apex et plus de comparaisons reference. |
| PP Systems UniSpec SC | partiel | Le `.SPT` synthetique couvre la forme axe-first; il manque une acquisition terrain reelle pour valider headers, units et metadata UniSpec SC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| PP Systems UniSpec DC | partiel | Le `.SPU` synthetique couvre la forme axe-first; il manque une acquisition terrain reelle pour valider les deux canaux et metadata UniSpec DC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| SVC / GER SIG | partiel | GER 3700 PDA + BEO HR-1024i field exports reels desormais committes; les fixtures declarees BAD sont qualifiees; restent les firmware HR-1024i ≥3.0 et la verification GPS/date/unites systematique. |
| Spectral Evolution / PSR | partiel | PSR DN brett + PSR-3500 grape leaf reels committes; le fichier DN-only broken-but-valid est qualifie sans reflectance; SR-3500 / SR-6500 firmware specifics restent a couvrir. |
| MODTRAN albedo | partiel | Le `.dat` synthetique valide l'axe-first; il manque une sortie MODTRAN redistribuable sous licence claire. |
| USGS SPECPR / PRISM | partiel | ASCII `.asc` et AREF single-column sont couverts; restent le binaire SPECPR et les axes vrais pour dumps AREF sans sidecar. |
| Thermo / Galactic GRAMS SPC | partiel | Golden coverage elargie aux fixtures IR/Raman/UV-vis/NIR ouvertes; restent big-endian, vieux headers complets et decision de scope pour NMR/FID. |
| Thermo Nicolet OMNIC | partiel | Decoder `.srsx` et variantes rapid-scan/high-speed. |
| Perkin Elmer Spectrum / IR | partiel | Ajouter variantes PE NIR; `.fsm` reste imaging hors v1. |
| Foss NIRSystems / WinISI natif | bloqué | Format ferme sans lecteur fiable ni fixture binaire de reference. |
| Metrohm Vision / Vision Air | partiel | Le CSV Vision Air synthetique est lu; il manque un export client reel et la base projet native reste fermee. |
| BUCHI NIRCal | partiel | Le chemin `.nir` lit spectra/wavenumbers/proprietes; les cibles non nulles sont validees localement sur `transpec_DEMO_cannabis.nir`. Restent une fixture redistribuable avec cibles non nulles, `.cal` calibration-only et variantes NIRMaster. |
| Perten DA / Inframatic | bloqué | Pas de fixture spectrale native; CSV actuel sans axe spectral. |
| JASCO JWS | partiel | Ajouter blocs V-series NIR et variantes Raman NRS. |
| Shimadzu UVProbe | partiel | Obtenir vrai `.spc` Shimadzu et comparaison convertisseur. |
| VIAVI MicroNIR | partiel | Reels CSV/XLSX MicroNIR 1700 committes et verrouilles par tests de lecture + probe (UvA forensic). `.pri` natif reste hors atteinte. |
| Si-Ware NeoSpectra | partiel | Reels OSSL Woodwell + UvA forensic XLSX committes et verrouilles par tests de lecture + probe; le descripteur OSSL non spectral est refuse explicitement. Reste a couvrir un export NeoSpectra Scanner natif single-measurement. |
| Spectro Inc. SiWare API | partiel | Les fixtures JSON/CSV sont synthetiques; il manque une reponse API reelle et des variantes de schema. |
| JCAMP-DX | partiel | XYDATA/ASDF/NTUPLES/LINK Ocean Optics sont couverts, y compris fichiers top-level multi-blocs (`nist_sucrose_ir.jdx` -> 2 records). Restent `LINK` generaux, `PEAK TABLE` apres extension du modele sparse, et plus de variantes NTUPLES. |
| NetCDF NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique, Microtops MAN, ARM MFRSR local et SURFSPECALB local derive sont couverts; AOSMET est un refus attendu non spectral. Restent schemas NIRS reels generiques, QC NetCDF4/HDF5 plus robuste et validation ACT/xarray. |
| AnIML | partiel | Le `SeriesSet` spectral synthetique est couvert et les resultats non-spectraux sont refuses; restent schemas spectraux reels, `AutoIncrementedValueSet` et validation XSD. |
| Allotrope ASM | partiel | Les trois fixtures Benchling spectrales/endpoints sont couvertes; restent conversions vendeurs multiples, cas ASM hors plate-reader et validation contre tooling Allotrope. |
| Allotrope ADF | bloqué | Pas de sample public ni SDK librement utilisable; seul un stub de detection/refus serait possible sans valider un vrai loader. |
| HDF5 NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique et les refus non-spectraux sont couverts; il manque schemas reels avec metadata, axes complexes et groupes multi-signaux. |
| MATLAB MAT / RData | partiel | MAT v5/v7.3 simples, DSO academiques, prospectr `NIRsoil.RData` et cube Indian Pines local-only sont couverts; restent structures MAT/RData generiques, cubes MAT v7.3 et metadata/targets heterogenes. |
| Renishaw WDF | partiel | Finaliser `MAP` derived data et fixtures par modele. |
| Horiba LabSpec / JobinYvon | partiel | `.l6m` reel Gd₂O₃/AlN map decode en mode experimental et compare integralement contre l'export texte (intensites + coordonnees); restent `.l6s`, autres layouts LabSpec6, metadata complete et axes energy mieux types. |
| Princeton TriVista TVF | partiel | Durcir metadata multi-frame/Step-and-Glue et comparaisons reference. |
| DigitalSurf MountainsMap | partiel | Fixtures spectres, maps, surface et zlib compresse/non compresse couverts; restent variantes MountainsMap hors corpus et metadata surfaces plus riche. |
| Hamamatsu HPD-TA IMG | partiel | Les fixtures HPD-TA 2D adjacentes sont couvertes; rester explicitement adjacent tant qu'aucun export spectral point-sample Hamamatsu n'est cible. |
| WiTec WIP / WID | partiel | `Sa4.wip` reel decode en 4410 spectres TDGraph `WIT_PR06`; restent layouts WiTec generaux, coordonnees physiques, conversion Raman-shift et export ASCII equivalent pour comparaison. |
| fNIRS neuroscience | pas fait | Domaine physiologie hors scope; rediriger vers SNIRF/MNE-NIRS. |

## Verification locale du corpus (2026-05-20)

Dernier sweep CLI apres mise a jour de la matrice:

| Corpus | OK | Refus attendus | Refus inattendus | Notes |
|---|---:|---:|---:|---|
| `samples/` | 245 | 18 | 0 | Les refus attendus sont des formats volontairement non-NIRS, des fixtures negatives, des sidecars seuls (`92AV3C.spc`, `92AV3GT.GIS`, header Microtops) ou des descripteurs non spectraux (`neospectra_ossl_column_names.csv`). |
| `samples_local/` | 11 | 5 | 0 | Lectures OK: Indian Pines MATLAB v5, BUCHI cannabis, ARM MFRSR NetCDF, ARM SURFSPECALB derive, 7 exports Microtops MAN ASCII `.lev*`. Refus attendus: `_gt.mat` sidecar, NOAA `.lev2`, ARM AOSMET et PP Systems indices non raw/derives. |

## Sweep d'echantillons publics (2026-05-20)

Recherche en ligne de fixtures redistribuables pour les formats `bloqué` /
`partiel`. Resultats:

### Nouveaux fixtures committes

| Format | Fichier ajoute | Source | Licence | Effet matrice |
|---|---|---|---|---|
| Foss / WinISI / DS exports | `samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv`, `foss_xds_barleyground_sensAIfood.csv` (+metadata) | [Zenodo 16759587](https://zenodo.org/records/16759587) — sensAIfood Univ. Cordoba (Foss XDS XM-1000 + NIRSYSTEM-5000) | CC-BY-4.0 | `partiel` → `fait` |
| Si-Ware NeoSpectra | `samples/siware_neospectra/neospectra_ossl_column_names.csv`, `neospectra_ossl_50samples_slice.csv`, `neospectra_forensic_K_avg.xlsx` | [Zenodo 13122321 OSSL](https://zenodo.org/records/13122321) + [Figshare 21252300 UvA forensic](https://doi.org/10.21942/uva.21252300) | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (vrais clients OSSL + forensique) |
| VIAVI MicroNIR | `samples/viavi_micronir/micronir_forensic_K_avg.xlsx`, `micronir_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — MicroNIR 1700 forensique UvA | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (CSV/XLSX reels) |
| Horiba LabSpec / JobinYvon | `samples/raman_horiba/AlN_Gd2O3_indepth.l6m` | [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) | MIT | `partiel` (XML/TXT seul) → `partiel` (`.l6m` decode experimental) |
| WiTec WIP / WID | `samples/raman_witec/Sa4.wip` | [Zenodo 7907659](https://zenodo.org/records/7907659) — analyse Raman ZrO₂ | ODbL v1.0 | `partiel` (ASCII seul) → `partiel` (`WIT_PR06` TDGraph decode experimental) |
| Excel spectral | `samples/excel/scio_forensic_P_avg.xlsx`, `nirone_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — Consumer Physics SCiO + Spectral Engines NIRone 2.0 | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (vrais XLSX vendeurs handheld) |
| Tables spectrales delimitees (handheld) | `samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv` (+metadata) | [Zenodo 15838272](https://zenodo.org/records/15838272) — sensAIfood Grainit (AuroraNIR 950-1650 nm) | CC-BY-4.0 | bonus handheld miniaturise |
| AVIRIS / hyperspectral cubes | `samples/hyperspectral_cubes/92AV3C.lan`, `92AV3C.spc`, `92AV3GT.GIS` | Public Indian Pines / AVIRIS fixture already mirrored locally | dataset terms to confirm before release | `partiel` (`92AV3C` ERDAS LAN decode experimental) |

### Sweep d'echantillons publics (2026-05-20 — second passage)

Apres le premier passage, recherche etendue sur PANGAEA, GitLab Allotrope,
github.com/pierreroudier/opusreader, github.com/joshduran/brukeropus,
github.com/cran/soil.spec, github.com/serbinsh/R-FieldSpectra,
github.com/capstone-coal/pycoal, github.com/hdeneke/PyrNet,
github.com/kebasaa/SCIO-read, ehu.eus/ccwintco (Indian Pines), NOAA Lauder.

#### Nouveaux fixtures committes (second passage)

| Format | Fichier ajoute | Source | Licence | Effet matrice |
|---|---|---|---|---|
| ENVI Spectral Library | `samples/envi_sli/usgs_splib06a_aviris95_envi.sli|hdr` + `usgs_splib07_aviris95_envi.sli|hdr` | [`capstone-coal/pycoal`](https://github.com/capstone-coal/pycoal) | GPL-2 (wrapper) + USGS public domain (data) | `partiel` → `fait` |
| Bruker OPUS natif (cross-reader) | `samples/bruker_opus/brukeropus_file.0`, `opusreader_test_spectra.0`, `icr_087266_B2.0`, `icr_087273_G3.0` | [`joshduran/brukeropus`](https://github.com/joshduran/brukeropus) (MIT), [`pierreroudier/opusreader`](https://github.com/pierreroudier/opusreader) (GPL-3), [`cran/soil.spec`](https://github.com/cran/soil.spec) AfSIS (GPL-2/3) | mixte (MIT + GPL) | reste `partiel` mais couverture cross-vendor elargie |
| Spectral Evolution / PSR | `samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | reste `partiel`, PSR-3500 firmware variant ajoute |
| SVC / GER SIG | `samples/svc_ger/serbinsh_gr070214_003.sig`, `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | GER 3700 PDA + HR-1024i Barrow firmware variants ajoutees |
| Microtops Sun Photometer | `samples/microtops/microtops_arc_msm114_2.nc` + `_header.txt` | [PANGAEA 966645](https://doi.pangaea.de/10.1594/PANGAEA.966645) (republished from AERONET MAN) | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (NetCDF MAN reel teste, fallback fixture apres tentative generique); legacy `.TXT` et MAN generique sans fallback toujours absents |
| NetCDF NIRS-adjacent | `samples/netcdf/pyrnet_to_l1a_output.nc` | [`hdeneke/PyrNet`](https://github.com/hdeneke/PyrNet) | academic share | refusal non-NIRS teste: pas d'axe spectral ni de canaux Microtops AOT |
| Consumer Physics SCiO | `samples/scio/scio_app_scan.csv`, `scio_calibration_plate_Polypen.csv`, `scio_scans_from_tech_support.csv` | [`kebasaa/SCIO-read`](https://github.com/kebasaa/SCIO-read) | GPL-3 | `fait`: `band*`, calibration axis-first et groupes `spectrum`/`wr_raw`/`sample_raw` testes; ajoute aussi `excel/scio_forensic_*.xlsx` UvA Figshare en complement |

#### Fixtures non-redistribuables (uniquement en local — `samples_local/`, gitignore)

| Format | Fichier | Source | Licence / raison non-commit | Effet |
|---|---|---|---|---|
| Hyperspectral cube (AVIRIS Indian Pines) | `samples_local/hyperspectral_cubes/indian_pines_corrected.mat` + `_gt.mat` | [EHU/Grupo de Inteligencia Computacional](http://www.ehu.eus/ccwintco/index.php/Hyperspectral_Remote_Sensing_Scenes) | "academic use" sans SPDX clair → en local seulement | reader MAT v5 local-only teste: 21,025 spectres x 200 bandes + cible `land_cover_class`; la version `92AV3C.lan` plus petite reste committee |
| Microtops MAN ASCII Okeanos | `samples_local/microtops/aeronet_man_Okeanos_19_2_*.lev10/.lev15/.lev20` | AERONET Maritime Aerosol Network | AERONET MAN PI/coauthorship policy -> en local seulement | reader local teste: AOD valides, canaux `-999` omis, AOD-STD pour exports daily/series |
| BUCHI NIRCal cannabis | `samples_local/buchi_nircal/transpec_DEMO_cannabis.nir` | orellano-c/transpec_info | licence non clarifiee pour redistribution du fixture -> en local seulement | reader local teste: 105 spectres, axe 1501 wavenumbers et cibles non nulles `CBDA`/`THCA` |
| ARM MFRSR / ARM NetCDF adjacents | `samples_local/mfr/*.nc`, `samples_local/netcdf/*.nc` | DOE ARM / ARM test data | ARM Data Use Policy -> en local seulement | MFRSR b1 local decode en 4,320 observations x 7 filtres; SURFSPECALB local decode en 986 lignes utiles x 6 filtres; AOSMET reste non spectral |
| PP Systems Arctic LTER indices | `samples_local/pp_systems/*.csv/.xlsx` | Arctic LTER / EDI | dataset local non committe | produit derive NDVI/EVI/PRI/etc.; ne ferme pas le manque de raw `.SPT/.SPU` |
| Microtops `.lev2` disambiguation | `samples_local/microtops/noaa_lauder_sonde_la20170315.lev2` | [NOAA GML Lauder](https://gml.noaa.gov/aftp/data/ozwv/WaterVapor/Lauder_LEV/) | US Gov public domain MAIS le fichier est en realite un radiosonde water vapour/ozone, pas un sun-photometer Microtops | aide locale a la disambiguation `.lev2`; non commit pour eviter confusion |

### Formats restant fermes (sweep sans resultat exploitable, apres 3 passages)

| Format | Pourquoi pas trouve |
|---|---|
| ASD calibration `.ILL/.REF/.RAW` | Distribution vendeur SDK uniquement; SPECCHIO partiel derriere login partenariat; aucun GitHub/Wayback/Mendeley sample. |
| Foss `.NIR/.DA/.cal/.eqa` natif | Format ferme, aucune fixture binaire publique trouvee (Wayback FOSS / NIR-Predictor demos checked). |
| Perten DA / Inframatic | Pas de fixture native ni CSV reel public (clients only). |
| Metrohm Vision Air / OMNIS NIR natif | Format ferme, seul l'export CSV est documente publiquement. |
| Allotrope ADF | Membership Allotrope obligatoire; adfsee + adf-explorer-plugins GitLab ne contiennent que du code source Java, pas de `.adf` sample. |
| MODTRAN albedo `.dat` reel | Distribution sous licence MODTRAN/ONTAR ($2400) ; MIT OCW pcmodwin/RIT tutorials ne shippent que des references USGS deja couvertes. |
| MFR-7 / MFRSR `.OUT` reel | ARM Data Center exige compte; `samples_local/mfr/` ferme localement un NetCDF ARM MFRSR b1, mais pas un `.OUT` MFR-7 redistribuable — non commit. |
| Microtops II `.TXT` reel | AERONET MAN demande co-authorship; `samples_local/microtops/` ferme localement les exports MAN ASCII `.lev*`, mais pas un `.TXT` legacy redistribuable — non commit. |
| PP Systems UniSpec `.SPT/.SPU` reel raw | Aucune fixture raw `.spu/.spt` publique; `samples_local/pp_systems/` contient seulement des indices derives Arctic LTER — non commit. |
| Bruker OPUS 5/6 legacy | Archives privees, pas de mirror public; OPUS 7/8 couvert via 4 lecteurs independants suffit. |
| Thermo OMNIC `.srsx` | Pas de fixture publique trouvee (S.T.Japan demo bibliotheques `.spg` derriere formulaire); le canal `.srs` reste couvert. |
| Shimadzu UVProbe `.spc` natif | Un seul candidat (`uri-t/shimadzu-spc-converter`) sans licence claire; aucune autre source apres sweep. |
| VIAVI MicroNIR `.pri` natif | Format projet binaire, customer-only; CSV/XLSX exports reels deja couverts via UvA forensic. |
| Si-Ware NeoSpectra Scanner natif single-measurement | Le pipeline OSSL ne publie que des matrices wide; pas de fixture "1 mesure par CSV" publique. |
| Specim IQ demo cube | Specim a discontinue le produit (page "end-of-life"); seul l'archive 7z Arabidopsis Zenodo 1345007 (123 MB) existe — trop gros, et le mix raw/processed n'est pas isole. |
| NEON AOP HDF5 reflectance tile | Tiles 1 km × 1 km demandent inscription neon.science (compte gratuit mais distribution conditionnelle); fichier minimum ~50 MB. |
| Horiba `.l6s` single-spectrum | Aucune fixture publique trouvee; seul `.l6m` (map) committe. |
| JASCO V-780 NIR / NRS Raman `.jws` variants | Aucun sample distinct du V-770 IR + V-series UV-Vis deja committes. |
