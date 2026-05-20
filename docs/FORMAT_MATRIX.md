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
| Bruker OPUS natif | Bruker | `.0`, `.1`, `.001`, `.0000`, sans extension fixe | OPUS moderne; OPUS 5/6 manquant | partiel | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Bruker Tango / MPA / Matrix | Bruker | OPUS natif | meme famille OPUS | partiel | opusreader2, SpectroChemPy |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr`, `.slb` | BSQ float32/float64 | partiel | spectral, RStoolbox, pysptools |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5, `.lan` | ENVI Standard BSQ/BIL/BIP point extraction; AVIRIS LAN fixture present non lu | partiel | spectral, rasterio |
| FGI HDF5 + XML | FGI | `.h5`, `.hdf5`, `.xml` | schema FGI | partiel | h5py, hdf5r, rhdf5, lxml |
| MFR Sun Photometer | Solar Light | `.OUT` | MFR-7 | partiel | SPECCHIO, parseurs ad hoc |
| Microtops Sun Photometer | Solar Light | `.TXT` | export texte | partiel | parseurs ad hoc |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.spc` | plusieurs familles texte + ProcSpec | partiel | lightr, pavo |
| PP Systems UniSpec SC | PP Systems | `.SPT` | export texte | partiel | SPECCHIO, parseurs ad hoc |
| PP Systems UniSpec DC | PP Systems | `.SPU` | export texte | partiel | SPECCHIO, parseurs ad hoc |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | PDA / laptop | partiel | spectrolab, specdal |
| Spectral Evolution / PSR | Spectral Evolution | `.sed` | export texte | partiel | spectrolab, specdal |
| MODTRAN albedo | Spectral Sciences / AFRL | `.dat` | sortie albedo | partiel | parseur texte |
| IDL / ENVI texte | IDL / ENVI | `.txt` | export axe-first | fait | parseur texte |
| USGS SPECPR / PRISM | USGS | `SPECPR`, `.asc` | ASCII seulement | partiel | convertisseur USGS |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc`, `.SPC` | new LSB OK; old limite; BE manquant | partiel | spc-spectra, rohanisaac/spc, specio, SpectroChemPy, xylib, spc-parser |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs`, `.srsx` | spa/spg/TGA-GC srs OK; srsx manquant | partiel | SpectroChemPy, spa-on-python |
| Perkin Elmer Spectrum / IR | PerkinElmer | `.sp`, `.fsm` | sp OK; fsm imaging refuse | partiel | specio |
| Foss NIRSystems / WinISI natif | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | binaire ferme | bloqué | aucune fiable |
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | exports matrices WinISI/Foss XDS reels | fait | parseur texte |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, base projet native | exports OK; DB native manquante | partiel | parseur texte, pandas, readxl |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | fixture NIRCal avec cibles nulles | partiel | prospectr::read_nircal |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire vendeur, `.csv` | binaire ferme; CSV cible seule refuse | bloqué | export CSV/Excel vendeur |
| JASCO JWS | JASCO | `.jws`, `.txt` | OLE2 DataInfo/Y-Data | partiel | jws2txt, jwsProcessor |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | texte OK; spc proprietaire manquant | partiel | pyfasma-spc, convertisseur Shimadzu |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.xlsx`, `.pri` | CSV/XLSX (UvA forensic 1700) OK; pri manquant | partiel | parseur texte, openpyxl |
| Si-Ware NeoSpectra | Si-Ware | `.csv`, `.xlsx` | OSSL Woodwell + UvA forensic XLSX OK | partiel | parseur texte, openpyxl |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | JSON synthetique | partiel | JSON/CSV standard |
| JCAMP-DX | Vendor-neutral / IUPAC | `.jdx`, `.dx`, `.jcm`, `.jcamp` | XYDATA, ASDF, NTUPLES, LINK partiel | partiel | jcamp, SpectroChemPy, nmrglue, ChemoSpec, hyperSpec |
| ANDI / NetCDF MS | ASTM / vendor-neutral | `.cdf`, `.nc` | detection + refus non-NIRS | fait | pyteomics, PyMassSpec, pyOpenMS |
| NetCDF NIRS generique | Vendor-neutral | `.nc`, `.cdf` | schema spectra+wavelengths | partiel | netcdf-reader, xarray, netcdf |
| AnIML | IUPAC / ASTM | `.animl` | spectral SeriesSet | partiel | animl-python, validateurs XML |
| Allotrope ASM | Allotrope / Benchling | `.json` | cubes/endpoints spectraux | partiel | Benchling allotropy |
| Allotrope ADF | Allotrope Foundation | `.adf` | HDF5/RDF, pas de sample | bloqué | Allotrope SDK |
| mzML / mzMLb | HUPO PSI / MS vendors | `.mzML`, `.mzMLb` | detection + refus non-NIRS | fait | pyteomics, pymzML, pyOpenMS |
| HDF5 NIRS generique | Vendor-neutral | `.h5`, `.hdf5` | schema spectra+wavelengths | partiel | h5py, hdf5-reader, tables |
| Parquet | Apache / generique | `.parquet` | table NIRS canonique | fait | pyarrow, fastparquet, nirs4all ParquetLoader |
| MATLAB MAT / RData | MATLAB / R ecosystem | `.mat`, `.MAT`, `.RData` | MAT v5/v7.3 + RData prospectr | partiel | scipy, hdf5-reader, R serialization, prospectr |
| NumPy | Python / NumPy | `.npy`, `.npz` | `.npy` matrice, `.npz` canonique | fait | numpy |
| Renishaw WDF | Renishaw | `.wdf` | spectra + metadata maps/images | partiel | RosettaSciIO, SpectroChemPy |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | XML/TXT OK; `.l6m` Gd₂O₃/AlN map decode experimental; `.l6s` manquant | partiel | RosettaSciIO, SpectroChemPy, horiba-raman |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | XML Frame payloads | partiel | RosettaSciIO |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | spectra/maps/surfaces | partiel | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | format adjacent 2D | partiel | RosettaSciIO |
| WiTec WIP / WID | WiTec | `.wip`, `.wid`, `.txt` | `WIT_PR06` TDGraph Sa4 decode experimental + ASCII OK; autres layouts refuses | partiel | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | ISO 22029 XY/Y | fait | RosettaSciIO |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | hors scope NIRS spectroscopy | pas fait | MNE-NIRS, SNIRF |

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
| Bruker OPUS natif | partiel | Couvrir OPUS 5/6, metadata Tango et blocs 2D/imaging. |
| Bruker Tango / MPA / Matrix | partiel | Ajouter fixtures instrument NIR et verifier les metadata propres Tango/MPA. |
| ENVI Spectral Library | partiel | Ajouter vraies `.sli/.hdr` vendeur et tests de variantes header. |
| ENVI / hyperspectral cubes | partiel | ENVI Standard `.img/.dat + .hdr` est charge en spectres par pixel; fixture AVIRIS/Indian Pines `.lan/.spc/.GIS` present mais lecteur ERDAS LAN absent; restent NEON/Specim/HySpex/Headwall, HDF5 cubes et API masque/extraction. |
| FGI HDF5 + XML | partiel | Ajouter paire HDF5/XML reelle et mapper le sidecar XML. |
| MFR Sun Photometer | partiel | Remplacer/complete par dumps instrument reels. |
| Microtops Sun Photometer | partiel | Ajouter exports Microtops reels avec metadata. |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | partiel | Ajouter variantes QE Pro/Maya/Apex et plus de comparaisons reference. |
| PP Systems UniSpec SC | partiel | Ajouter acquisitions terrain reelles. |
| PP Systems UniSpec DC | partiel | Ajouter acquisitions terrain reelles. |
| SVC / GER SIG | partiel | Ajouter variantes firmware et verifier GPS/date/unites. |
| Spectral Evolution / PSR | partiel | Ajouter plus de fixtures instrument et comparaisons `specdal`. |
| MODTRAN albedo | partiel | Ajouter sortie MODTRAN redistribuable sous licence claire. |
| USGS SPECPR / PRISM | partiel | Implementer/valider le binaire SPECPR ou un flux de conversion stable. |
| Thermo / Galactic GRAMS SPC | partiel | Couvrir big-endian, vieux headers et fixtures multi-canaux. |
| Thermo Nicolet OMNIC | partiel | Decoder `.srsx` et variantes rapid-scan/high-speed. |
| Perkin Elmer Spectrum / IR | partiel | Ajouter variantes PE NIR; `.fsm` reste imaging hors v1. |
| Foss NIRSystems / WinISI natif | bloqué | Format ferme sans lecteur fiable ni fixture binaire de reference. |
| Foss / WinISI / DS exports | fait | Real Foss XDS / NIRSYSTEM-5000 exports commits (sensAIfood Cordoba). |
| Metrohm Vision / Vision Air | partiel | Decoder DB native ou documenter uniquement le chemin export. |
| BUCHI NIRCal | partiel | Obtenir fixtures avec cibles non nulles et variantes NIRMaster/calibration. |
| Perten DA / Inframatic | bloqué | Pas de fixture spectrale native; CSV actuel sans axe spectral. |
| JASCO JWS | partiel | Ajouter blocs V-series NIR et variantes Raman NRS. |
| Shimadzu UVProbe | partiel | Obtenir vrai `.spc` Shimadzu et comparaison convertisseur. |
| VIAVI MicroNIR | partiel | Reel CSV/XLSX MicroNIR 1700 desormais committe (UvA forensic). `.pri` natif reste hors atteinte. |
| Si-Ware NeoSpectra | partiel | Reels OSSL Woodwell + UvA forensic XLSX desormais committes; resterait a couvrir un export NeoSpectra Scanner natif single-measurement. |
| Spectro Inc. SiWare API | partiel | Ajouter reponse API reelle et tests de schemas variantes. |
| JCAMP-DX | partiel | Couvrir plus de `LINK`, `PEAK TABLE` et variantes NTUPLES. |
| NetCDF NIRS generique | partiel | Ajouter schemas NIRS reels au-dela de `spectra+wavelengths`. |
| AnIML | partiel | Couvrir plus de schemas spectraux et valider contre XSD. |
| Allotrope ASM | partiel | Ajouter conversions vendeurs multiples et cas ASM hors plate-reader. |
| Allotrope ADF | bloqué | Pas de sample public ni SDK librement utilisable. |
| HDF5 NIRS generique | partiel | Ajouter schemas reels et metadata/axes complexes. |
| MATLAB MAT / RData | partiel | Couvrir plus de structures MAT/RData et metadata/targets heterogenes. |
| Renishaw WDF | partiel | Finaliser `MAP` derived data et fixtures par modele. |
| Horiba LabSpec / JobinYvon | partiel | `.l6m` reel Gd₂O₃/AlN map decode en mode experimental et valide contre l'export texte; restent `.l6s`, autres layouts LabSpec6, metadata complete et axes energy mieux types. |
| Princeton TriVista TVF | partiel | Durcir metadata multi-frame/Step-and-Glue et comparaisons reference. |
| DigitalSurf MountainsMap | partiel | Ajouter variantes compressees/non compressees et metadata surfaces. |
| Hamamatsu HPD-TA IMG | partiel | Clarifier si le format reste adjacent ou devient export spectral supporte. |
| WiTec WIP / WID | partiel | `Sa4.wip` reel decode en 4410 spectres TDGraph `WIT_PR06`; restent layouts WiTec generaux, coordonnees physiques, conversion Raman-shift et export ASCII equivalent pour comparaison. |
| fNIRS neuroscience | pas fait | Domaine physiologie hors scope; rediriger vers SNIRF/MNE-NIRS. |

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
| AVIRIS / hyperspectral cubes | `samples/hyperspectral_cubes/92AV3C.lan`, `92AV3C.spc`, `92AV3GT.GIS` | Public Indian Pines / AVIRIS fixture already mirrored locally | dataset terms to confirm before release | fixture present; lecteur ERDAS LAN absent |

### Formats restant fermes (sweep sans resultat exploitable)

| Format | Pourquoi pas trouve |
|---|---|
| ASD calibration `.ILL/.REF/.RAW` | Distribution vendeur SDK uniquement; SPECCHIO partiel derriere login partenariat. |
| Foss `.NIR/.DA/.cal/.eqa` natif | Format ferme, aucune fixture binaire publique trouvee. |
| Perten DA / Inframatic | Pas de fixture native ni CSV reel public (clients only). |
| Metrohm Vision Air / OMNIS NIR natif | Format ferme, seul l'export CSV est documente publiquement. |
| Allotrope ADF | Membership Allotrope obligatoire, pas de sample public. |
| MODTRAN albedo `.dat` reel | Distribution sous licence MODTRAN, pas de fixture publique redistribuable. |
| MFR-7 / MFRSR `.OUT` reel | ARM Data Center exige compte (gratuit mais non-redistribution claire). |
| Microtops II `.TXT` reel | AERONET Maritime Aerosol Network derriere login; aucun mirror GitHub trouve. |
| PP Systems UniSpec `.SPT/.SPU` reel | Outils de processing (ARC-LTER, rUnispecDC) publics, mais aucun raw binaire/text committe. |
| Bruker OPUS 5/6 legacy | Archives privees, pas de mirror public. |
| Thermo OMNIC `.srsx` | Pas de fixture publique trouvee; le canal de chargement `.srs` reste couvert via spectrochempy_data. |
| Shimadzu UVProbe `.spc` natif | Un seul candidat (`uri-t/shimadzu-spc-converter`) sans licence claire. |
| VIAVI MicroNIR `.pri` natif | Format projet binaire, customer-only. |
| Si-Ware NeoSpectra Scanner natif single-measurement | Le pipeline OSSL ne publie que des matrices wide; pas de fixture "1 mesure par CSV" publique. |
