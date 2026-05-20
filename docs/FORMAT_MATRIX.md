# Matrice compacte des formats

Statuts utilisés: `fait`, `partiel`, `pas fait`, `bloqué`.

| Nom | Vendeur | Extension | Version (si applicable) | Status nirs4allio | Lib référence |
|---|---|---|---|---|---|
| Tables spectrales delimitees | Generique | `.csv`, `.tsv`, `.txt` | en-tetes numeriques | fait | pandas, read.table, nirs4all CSVLoader |
| Tables axe-first | Generique / exports instrument | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | une colonne axe + signaux | fait | pandas, read.table |
| Matrices spectrales | Generique / Foss / Metrohm / VIAVI | `.csv`, `.txt` | un spectre par ligne | fait | pandas, read.table |
| Excel spectral | Generique / lab | `.xlsx`, `.xlsm`, `.xls` | xlsx/xlsm OK, xls manquant | partiel | calamine, openpyxl, pandas, readxl |
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | revisions 1, 6, 7, 8 | partiel | asdreader, prospectr, spectrolab, specdal, pyASDReader |
| ASD calibration | ASD / Malvern Panalytical | `.ILL`, `.REF`, `.RAW` | compagnons calibration | bloqué | SPECCHIO, asdreader |
| Avantes AvaSoft 6/7 binaire | Avantes | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | legacy 6/7 | partiel | lightr |
| Avantes AvaSoft 8 binaire | Avantes | `.Raw8`, `.IRR8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8` | AVS8 | partiel | lightr, manuel AvaSoft |
| Avantes ASCII | Avantes | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR` | exports texte | fait | pandas, read.table |
| Bruker OPUS DPT | Bruker | `.dpt` | export ASCII OPUS | fait | pandas, read.table |
| Bruker OPUS natif | Bruker | `.0`, `.1`, `.001`, `.0000`, sans extension fixe | OPUS moderne; OPUS 5/6 manquant | partiel | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Bruker Tango / MPA / Matrix | Bruker | OPUS natif | meme famille OPUS | partiel | opusreader2, SpectroChemPy |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr`, `.slb` | BSQ float32/float64 | partiel | spectral, RStoolbox, pysptools |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5 | ENVI Standard BSQ/BIL/BIP point extraction | partiel | spectral, rasterio |
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
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | exports matrices | partiel | parseur texte |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, base projet native | exports OK; DB native manquante | partiel | parseur texte, pandas, readxl |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | fixture NIRCal avec cibles nulles | partiel | prospectr::read_nircal |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire vendeur, `.csv` | binaire ferme; CSV cible seule refuse | bloqué | export CSV/Excel vendeur |
| JASCO JWS | JASCO | `.jws`, `.txt` | OLE2 DataInfo/Y-Data | partiel | jws2txt, jwsProcessor |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | texte OK; spc proprietaire manquant | partiel | pyfasma-spc, convertisseur Shimadzu |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.pri` | CSV OK; pri manquant | partiel | parseur texte |
| Si-Ware NeoSpectra | Si-Ware | `.csv` | export CSV synthetique | partiel | parseur texte |
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
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | XML/TXT OK; binaire manquant | partiel | RosettaSciIO, SpectroChemPy, horiba-raman |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | XML Frame payloads | partiel | RosettaSciIO |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | spectra/maps/surfaces | partiel | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | format adjacent 2D | partiel | RosettaSciIO |
| WiTec WIP / WID | WiTec | `.wip`, `.wid`, `.txt` | binaire detecte/refuse; ASCII OK | partiel | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | ISO 22029 XY/Y | fait | RosettaSciIO |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | hors scope NIRS spectroscopy | pas fait | MNE-NIRS, SNIRF |

## Notes pour les statuts non finis

Les lignes `fait` ne sont pas repetees ici. La note indique ce qui manque pour
passer le format a `fait`.

| Nom | Status nirs4allio | Note / manque |
|---|---|---|
| Excel spectral | partiel | Ajouter `.xls` legacy et plus de fixtures multi-feuilles reelles. |
| ASD FieldSpec | partiel | Decoder les blocs reference/calibration et couvrir les revisions legacy. |
| ASD calibration | bloqué | Obtenir un jeu redistribuable `.asd` + `.ILL/.REF/.RAW`. |
| Avantes AvaSoft 6/7 binaire | partiel | Ajouter fixtures `.ABS/.IRR/.RMN` et comparaison `lightr`. |
| Avantes AvaSoft 8 binaire | partiel | Ajouter fixtures pour chaque suffixe AVS8 et valider les modes. |
| Bruker OPUS natif | partiel | Couvrir OPUS 5/6, metadata Tango et blocs 2D/imaging. |
| Bruker Tango / MPA / Matrix | partiel | Ajouter fixtures instrument NIR et verifier les metadata propres Tango/MPA. |
| ENVI Spectral Library | partiel | Ajouter vraies `.sli/.hdr` vendeur et tests de variantes header. |
| ENVI / hyperspectral cubes | partiel | ENVI Standard `.img/.dat + .hdr` est charge en spectres par pixel; restent NEON/Specim/HySpex/Headwall, HDF5 cubes et API masque/extraction. |
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
| Foss / WinISI / DS exports | partiel | Ajouter exports reels DS/WinISI avec cibles et metadata. |
| Metrohm Vision / Vision Air | partiel | Decoder DB native ou documenter uniquement le chemin export. |
| BUCHI NIRCal | partiel | Obtenir fixtures avec cibles non nulles et variantes NIRMaster/calibration. |
| Perten DA / Inframatic | bloqué | Pas de fixture spectrale native; CSV actuel sans axe spectral. |
| JASCO JWS | partiel | Ajouter blocs V-series NIR et variantes Raman NRS. |
| Shimadzu UVProbe | partiel | Obtenir vrai `.spc` Shimadzu et comparaison convertisseur. |
| VIAVI MicroNIR | partiel | Decoder `.pri` ou obtenir specification/sample; CSV seulement aujourd'hui. |
| Si-Ware NeoSpectra | partiel | Remplacer fixture synthetique par export client reel. |
| Spectro Inc. SiWare API | partiel | Ajouter reponse API reelle et tests de schemas variantes. |
| JCAMP-DX | partiel | Couvrir plus de `LINK`, `PEAK TABLE` et variantes NTUPLES. |
| NetCDF NIRS generique | partiel | Ajouter schemas NIRS reels au-dela de `spectra+wavelengths`. |
| AnIML | partiel | Couvrir plus de schemas spectraux et valider contre XSD. |
| Allotrope ASM | partiel | Ajouter conversions vendeurs multiples et cas ASM hors plate-reader. |
| Allotrope ADF | bloqué | Pas de sample public ni SDK librement utilisable. |
| HDF5 NIRS generique | partiel | Ajouter schemas reels et metadata/axes complexes. |
| MATLAB MAT / RData | partiel | Couvrir plus de structures MAT/RData et metadata/targets heterogenes. |
| Renishaw WDF | partiel | Finaliser `MAP` derived data et fixtures par modele. |
| Horiba LabSpec / JobinYvon | partiel | Ajouter binaires `.l6s/.l6m` et axes energy mieux typés. |
| Princeton TriVista TVF | partiel | Durcir metadata multi-frame/Step-and-Glue et comparaisons reference. |
| DigitalSurf MountainsMap | partiel | Ajouter variantes compressees/non compressees et metadata surfaces. |
| Hamamatsu HPD-TA IMG | partiel | Clarifier si le format reste adjacent ou devient export spectral supporte. |
| WiTec WIP / WID | partiel | Obtenir `.wip/.wid` reel avec export ASCII equivalent. |
| fNIRS neuroscience | pas fait | Domaine physiologie hors scope; rediriger vers SNIRF/MNE-NIRS. |
