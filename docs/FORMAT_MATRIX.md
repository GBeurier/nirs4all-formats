# Matrice compacte des formats

Statuts utilisés: `fait`, `partiel`, `pas fait`, `bloqué`.

| Nom | Vendeur | Extension | Version (si applicable) | Status nirs4allio | Lib référence |
|---|---|---|---|---|---|
| Tables spectrales delimitees | Generique | `.csv`, `.tsv`, `.txt` | en-tetes numeriques; CSV virgule, CSV point-virgule et TSV golden-backed | fait | pandas, read.table, nirs4all CSVLoader |
| Tables axe-first | Generique / exports instrument | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | une colonne axe + signaux | fait | pandas, read.table |
| Matrices spectrales | Generique / Foss / Metrohm / VIAVI | `.csv`, `.txt` | un spectre par ligne | fait | pandas, read.table |
| Excel spectral | Generique / lab | `.xlsx`, `.xlsm`, `.xls` | `.xlsx` reel + descripteur axis/data OK; `.xlsm` fixture-backed; `.xls` manquant | partiel | calamine, openpyxl, pandas, readxl |
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | revisions 1, 6, 7, 8 | partiel | asdreader, prospectr, spectrolab, specdal, pyASDReader |
| ASD calibration | ASD / Malvern Panalytical | `.ILL`, `.REF`, `.RAW` | compagnons calibration; aucun sample ASD compagnon | bloqué | SPECCHIO, asdreader |
| Avantes AvaSoft 6/7 binaire | Avantes | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | legacy 6/7; `.IRR` present est ASCII | partiel | lightr |
| Avantes AvaSoft 8 binaire | Avantes | `.Raw8`, `.IRR8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8` | AVS8 `.Raw8/.IRR8` fixture-backed | partiel | lightr, manuel AvaSoft |
| Avantes ASCII | Avantes | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.txt` | wave-table mono/multi-colonnes, irradiance deux-colonnes, AvaSoft 8 text | fait | pandas, read.table |
| Bruker OPUS DPT | Bruker | `.dpt` | export ASCII OPUS synthetique + RS-1 `lightr` reel | fait | pandas, read.table, lightr |
| Bruker OPUS natif | Bruker | `.0`, `.1`, `.001`, `.0000`, sans extension fixe | corpus OPUS 7/8 complet golden-backed + tests semantiques cross-reader (4 lecteurs) + Bruker MPA AfSIS soils; axes `MIN` types `time`; OPUS 5/6 manquant | partiel | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Bruker Tango / MPA / Matrix | Bruker | OPUS natif | meme famille OPUS | partiel | opusreader2, SpectroChemPy |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr` | BSQ float32/float64; USGS splib06/07 vendeur reels (AVIRIS95 grid) via `.hdr` ou `.sli`; `.slb` non fixture | fait | spectral, RStoolbox, pysptools |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5, `.lan`, `.mat` | ENVI Standard `.hdr` ou `.img` + AVIRIS 92AV3C ERDAS LAN + local Indian Pines MAT point extraction | partiel | spectral, rasterio, scipy |
| FGI HDF5 + XML | FGI | `.h5`, `.hdf5`, `.xml` | paire HDF5+XML synthetique mappee | partiel | h5py, hdf5r, rhdf5, lxml |
| MFR Sun Photometer | Solar Light / YES Inc. | `.OUT`, `.nc` local | MFR-7 `.OUT` synthetique + ARM MFRSR b1 NetCDF local decode + sidecar QC YAML local | partiel | SPECCHIO, parseurs ad hoc, xarray, ARM ACT |
| Microtops Sun Photometer | Solar Light | `.TXT`, `.nc` (MAN export), `.lev10/.lev15/.lev20` local | CSV synthetique + MAN NetCDF PANGAEA avec fallback fixture + MAN ASCII AERONET local Okeanos; AOT type `aerosol_optical_thickness` | partiel | parseurs ad hoc, xarray |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.jdx`, `.spc` | textes SpectraSuite/OceanView/Jaz/CRAIC + CSV + ProcSpec transmittance/reflectance type; `.jdx` via JCAMP; `.spc` Galactic route | partiel | lightr, pavo |
| PP Systems UniSpec SC | PP Systems | `.SPT` | export texte synthetique teste semantiquement | partiel | SPECCHIO, parseurs ad hoc |
| PP Systems UniSpec DC | PP Systems | `.SPU` | export texte synthetique teste semantiquement | partiel | SPECCHIO, parseurs ad hoc |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | 15-fixture corpus golden-backed + semantic assertions PDA/laptop/WR/moc + GER 3700 + HR-1024i field; BAD fixtures flagged | partiel | spectrolab, specdal |
| Spectral Evolution / PSR | Spectral Evolution | `.sed` | PSR DN brett + PSR-3500 grape leaf reels; DN-only semantic flag + GPS/date/time promotion testes | partiel | spectrolab, specdal |
| MODTRAN albedo | Spectral Sciences / AFRL | `.dat` | sortie albedo synthetique uniquement | partiel | parseur texte |
| IDL / ENVI texte | IDL / ENVI | `.txt` | export axe-first | fait | parseur texte |
| USGS SPECPR / PRISM / ECOSTRESS text | USGS / JHU / ECOSTRESS | `SPECPR`, `.asc`, `.txt`, `.spectrum.txt` | ASCII `.asc` + ECOSTRESS/ASTER `.spectrum.txt` + AREF single-column; binaire manquant | partiel | convertisseur USGS |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc`, `.SPC` | corpus new LSB golden elargi + axes temps SPC types `time`; old LSB partiel; BE manquant | partiel | spc-spectra, rohanisaac/spc, specio, SpectroChemPy, xylib, spc-parser |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs`, `.srsx` | spa/spg corpus elargi + `.srs` `tg_gc`, `rapid_scan_raw`, `rapid_scan_reprocessed`; srsx manquant | partiel | SpectroChemPy, spa-on-python |
| Perkin Elmer Spectrum / IR | PerkinElmer | `.sp`, `.fsm` | `.sp` PEPE mono golden-backed; `.fsm` imaging refuse/out of scope | partiel | specio |
| Foss NIRSystems / WinISI natif | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | binaire ferme | bloqué | aucune fiable |
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | exports matrices WinISI/Foss XDS reels | fait | parseur texte |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, base projet native | CSV export synthetique teste semantiquement; DB native manquante | partiel | parseur texte, pandas, readxl |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | fixture NIRCal avec cibles nulles + cannabis local avec cibles non nulles; zeros conserves quand la table cible est non nulle | partiel | prospectr::read_nircal |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire vendeur, `.csv` | binaire ferme; CSV cible seule refuse | bloqué | export CSV/Excel vendeur |
| JASCO JWS | JASCO | `.jws`, `.txt` | OLE2 DataInfo/Y-Data FT/IR + fluorescence + CD/HT/Abs; texte export | partiel | jws2txt, jwsProcessor |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | `.txt` synthetique teste semantiquement; `.spc` proprietaire manquant | partiel | pyfasma-spc, convertisseur Shimadzu |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.xlsx`, `.pri` | CSV/XLSX (UvA forensic 1700) OK; pri manquant | partiel | parseur texte, openpyxl |
| Si-Ware NeoSpectra | Si-Ware | `.csv`, `.xlsx` | OSSL Woodwell + UvA forensic XLSX OK | partiel | parseur texte, openpyxl |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | JSON natif + CSV axis-first synthetiques golden-backed | partiel | JSON/CSV standard |
| JCAMP-DX | Vendor-neutral / IUPAC | `.jdx`, `.dx`, `.jcm`, `.jcamp` | XYDATA mono/multi-blocs, ASDF, NTUPLES avec axes frequence/temps, LINK partiel | partiel | jcamp, SpectroChemPy, nmrglue, ChemoSpec, hyperSpec |
| ANDI / NetCDF MS | ASTM / vendor-neutral | `.cdf`, `.nc` | detection + refus non-NIRS | fait | pyteomics, PyMassSpec, pyOpenMS |
| NetCDF NIRS generique | Vendor-neutral | `.nc`, `.cdf` | schema spectra+wavelengths synthetique + lecteurs dedies Microtops MAN / ARM MFRSR / ARM SURFSPECALB locaux + refus adjacents | partiel | netcdf-reader, xarray, netcdf, ARM ACT |
| AnIML | IUPAC / ASTM | `.animl` | spectral SeriesSet synthetique explicite + `AutoIncrementedValueSet` uniforme | partiel | animl-python, validateurs XML |
| Allotrope ASM | Allotrope / Benchling | `.json` | fixtures Benchling cubes/endpoints spectraux | partiel | Benchling allotropy |
| Allotrope ADF | Allotrope Foundation | `.adf` | HDF5/RDF; `adfsee` GitLab `example.adf` local data-cube subset + RDF component mapping minimal avec axe temps type | partiel | Allotrope SDK, adfsee |
| mzML / mzMLb | HUPO PSI / MS vendors | `.mzML`, `.mzMLb` | `.mzML` detection + refus non-NIRS; `.mzMLb` documente sans fixture | fait | pyteomics, pymzML, pyOpenMS |
| HDF5 NIRS generique | Vendor-neutral | `.h5`, `.hdf5` | schema spectra+wavelengths synthetique + refus non-spectral | partiel | h5py, hdf5-reader, tables |
| Parquet | Apache / generique | `.parquet` | table NIRS canonique | fait | pyarrow, fastparquet, nirs4all ParquetLoader |
| MATLAB MAT / RData | MATLAB / R ecosystem | `.mat`, `.MAT`, `.RData` | MAT v5/v7.3 + RData prospectr + local Indian Pines cube | partiel | scipy, hdf5-reader, R serialization, prospectr |
| NumPy | Python / NumPy | `.npy`, `.npz` | `.npy` matrice, `.npz` canonique | fait | numpy |
| Renishaw WDF | Renishaw | `.wdf` | WDF1 spectra + maps/lines/depth/zscan/focustrack/timeseries/streamline/interrupted; `MAP ` PSET inventory + `dataRange` tail sur fixtures map/depth | partiel | RosettaSciIO, SpectroChemPy |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | XML/TXT OK; `.l6m` Gd₂O₃/AlN map decode experimental; `.l6s` manquant | partiel | RosettaSciIO, SpectroChemPy, horiba-raman |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | corpus RosettaSciIO XML Frame golden-backed; conformance metadata a durcir | partiel | RosettaSciIO |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | corpus RosettaSciIO spectra/maps/surfaces/zlib golden-backed; conformance metadata a durcir | partiel | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | corpus adjacent 2D OK; hors point-spectra NIRS | partiel | RosettaSciIO |
| WiTec WIP / WID | WiTec | `.wip`, `.wid`, `.txt` | `WIT_PR06` TDGraph Sa4 decode experimental + ASCII OK; autres layouts refuses | partiel | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | ISO 22029 XY/Y avec axes `eV` types `energy`, notation scientifique, titres multi-lignes, metadata non conformes preservees | fait | RosettaSciIO |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | hors scope NIRS spectroscopy | pas fait | MNE-NIRS, SNIRF |
| Consumer Physics SCiO | Consumer Physics | `.csv` (developer app) | 740-1070 nm handheld DLP-MEMS; `band*`, `spectrum`/`wr_raw`/`sample_raw` et calibration axis-first golden-backed | fait | kebasaa/SCIO-read |

## Notes pour les statuts non finis

Les lignes `fait` ne sont pas repetees ici. La note indique ce qui manque pour
passer le format a `fait`.

| Nom | Status nirs4allio | Note / manque |
|---|---|---|
| Excel spectral | partiel | `.xlsx` synthetique/multi-feuilles/reels UvA et `.xlsm` OOXML macro-compatible sont golden-backed; workbooks metadata-only AuroraNIR/Foss XDS refuses explicitement. Restent `.xls` legacy OLE, un vrai `.xlsm` avec macros si besoin de metadata VBA, plus de fixtures multi-feuilles reelles et les cas ou Excel convertit les longueurs d'onde en dates. |
| ASD FieldSpec | partiel | Revisions 1/6/7/8 primary spectra couvertes; restent v3/v4/v5 eventuelles, blocs internes secondary/dependent/reference/calibration, audit/signatures et compagnons calibration `.ILL/.REF/.RAW` separes. |
| ASD calibration | bloqué | Obtenir un jeu redistribuable `.asd` + `.ILL/.REF/.RAW`; les samples `.asd` actuels ne contiennent pas les compagnons calibration, et le `.REF` present dans `samples/avantes/` est Avantes, pas ASD. |
| Avantes AvaSoft 6/7 binaire | partiel | Ajouter fixtures `.ABS` et autres modes binaires legacy puis comparaison `lightr`; le `.IRR` present est un export ASCII couvert par Avantes ASCII, pas une preuve du binaire legacy. |
| Avantes AvaSoft 8 binaire | partiel | `.Raw8` et `.IRR8` sont couverts par fixtures/goldens/tests semantiques (`AVS84`, modes 0/4, calibration `.IRR8` non appliquee); restent `.RWD8/.ABS8/.TRM8/.RFL8/.RIR8/.RMN8/.RMD8`, multi-subfile AVS8 et calibration irradiance complete pour `.IRR8`. |
| Bruker OPUS natif | partiel | Tout le corpus commite `samples/bruker_opus/` est golden-backed et les fixtures cross-reader restantes ont des tests semantiques directs: spectral-cockpit/opusreader2, pierreroudier/opusreader, brukeropus MIT, SpectroChemPy et cran soil.spec AfSIS/MPA. Les axes OPUS `MIN` sont maintenant types `time` quand rencontres. Restent OPUS 5/6 legacy archives, blocs 2D/imaging et conformance full-array automatisee contre lecteurs externes. |
| Bruker Tango / MPA / Matrix | partiel | AfSIS Bruker MPA `icr_*.0` reels committes (cran/soil.spec). Reste Bruker Tango FT-NIR dedie et metadata MPA/Matrix complete. |
| ENVI / hyperspectral cubes | partiel | ENVI Standard `.hdr` et entree directe `.img/.dat`, AVIRIS/Indian Pines `.lan/.spc/.GIS` et le cube MATLAB local-only `indian_pines_corrected.mat` sont charges en spectres par pixel; restent ERDAS LAN generique, NEON/Specim/HySpex/Headwall, HDF5 cubes et API masque/extraction. |
| FGI HDF5 + XML | partiel | Sidecar XML synthetique mappe vers HDF5 et provenance double; reste a valider une paire FGI reelle et le schema XML complet. |
| MFR Sun Photometer | partiel | Le `.OUT` synthetique valide le parseur texte; le MFRSR NetCDF ARM local est decode en 4,320 enregistrements x 7 filtres avec signaux hemispheric/diffuse/direct/alltime/ratio, QC NetCDF et sidecar YAML de plages suspectes/incorrectes. Restent un dump MFR-7/MFRSR redistribuable, un mapping ARM plus large (`_FillValue`, calibration, filtres) et comparaison ACT/xarray. |
| Microtops Sun Photometer | partiel | MAN NetCDF reel committe et teste (PANGAEA MSM114/2, CC-BY-4.0). Le reader tente une decouverte generique `aot_<nm>`, mais le payload MSM114/2 reste lu via fallback SHA-256 car `hdf5-reader` ne resout pas encore ce layout NetCDF4/HDF5. Les 7 exports AERONET MAN ASCII locaux `.lev10/.lev15/.lev20` sont testes avec AOD et AOD-STD; les signaux primaires AOT sont types `aerosol_optical_thickness`, tandis que `aot_std` reste une incertitude. Restent un vrai `.TXT` legacy redistribuable et un lecteur MAN NetCDF generique sans fallback. |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | partiel | Les 12 samples Ocean Optics committes sont golden-backed: textes SpectraSuite/OceanView/Jaz/JazIrrad/CRAIC/CSV/Master.Transmission, ProcSpec Linux/Windows types transmittance et white-reference type reflectance via XML core processor / `yUnits`, JCAMP LINK via `jcamp-dx` et `.spc` OceanView route Galactic. Restent QE Pro/Maya/Apex, vrai `.spc` Ocean non-Galactic, typage des Jaz/textes generiques sans metadata explicite et rapports reference `lightr`/`pavo`. |
| PP Systems UniSpec SC | partiel | Le `.SPT` synthetique est verrouille par golden et test semantique sur axe `nm`, metadata header, `dn_white`/`dn_target` raw et reflectance. Il manque une acquisition terrain reelle pour valider headers, units et metadata UniSpec SC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| PP Systems UniSpec DC | partiel | Le `.SPU` synthetique est verrouille par golden et test semantique sur axe `nm`, metadata header, `channel_a_dn`/`channel_b_dn` raw et reflectance. Il manque une acquisition terrain reelle pour valider les deux canaux et metadata UniSpec DC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| SVC / GER SIG | partiel | Les 15 fixtures committes sont golden-backed avec assertions semantiques directes pour SVC laptop, SVC PDA Acer clean/white-reference, matched-overlap-corrected, deux BAD declares, GER 3700 PDA et BEO HR-1024i field. Restent les firmware HR-1024i >=3.0, la promotion GPS/date/unites et les comparaisons automatisees `spectrolab`/`specdal`. |
| Spectral Evolution / PSR | partiel | PSR DN brett + PSR-3500 grape leaf reels committes; le fichier DN-only broken-but-valid est teste comme deux signaux raw avec `sed_missing_reflectance_signal` / `missing_reflectance_signal`, et les champs GPS/date/time parseables sont promus en metadata canonique. Restent SR-3500 / SR-6500 firmware specifics, typage plus fin des unites et conformance `spectrolab`/`specdal`. |
| MODTRAN albedo | partiel | Le `.dat` synthetique valide l'axe-first; il manque une sortie MODTRAN redistribuable sous licence claire. |
| USGS SPECPR / PRISM / ECOSTRESS text | partiel | ASCII `.asc`, ECOSTRESS/ASTER `.spectrum.txt` et AREF single-column sont couverts; restent le binaire SPECPR et les axes vrais pour dumps AREF sans sidecar. |
| Thermo / Galactic GRAMS SPC | partiel | Golden coverage elargie au corpus IR/Raman/UV-vis/NIR/NMR-FID ouvert, avec tests semantiques directs pour multi-subfile generated-X, directory-backed `TXYXYS`, old ordered-Z limite et axes SPC minute/seconde types `time` sur `s_xy.spc` et `NMR_FID.SPC`. Restent new big-endian `0x4C`, vieux headers/logs complets et decision de scope finale pour NMR/FID. |
| Thermo Nicolet OMNIC | partiel | SPA/SPG/SRS TGA-GC sont verrouilles par goldens/tests semantiques sur le corpus committe, y compris matrice 2D, offsets et metadata `series_y_*`; les trois `.srs` locaux SpectroChemPy couvrent `tg_gc`, `rapid_scan_raw` et `rapid_scan_reprocessed`. Reste `.srsx` et davantage de variantes high-speed. |
| Perkin Elmer Spectrum / IR | partiel | Le `.sp` PEPE mono-spectre reel `specio` est golden-backed et teste semantiquement. Le statut reste partiel parce que la ligne inclut `.fsm` Spotlight imaging, detecte/refuse comme hors v1, et parce qu'il manque des variantes PE NIR/Lambda; une separation future `.sp` vs `.fsm` permettrait de promouvoir le sous-ensemble `.sp`. |
| Foss NIRSystems / WinISI natif | bloqué | Format ferme sans lecteur fiable ni fixture binaire de reference; les samples Foss actuels sont des exports CSV/texte et les `.nir` presents sont BUCHI NIRCal, donc ne debloquent pas `.NIR/.DA/.cal/.eqa`. |
| Metrohm Vision / Vision Air | partiel | Le CSV Vision Air synthetique est verrouille par golden et test semantique sur 50 records, axe `nm`, signal absorbance et cibles `protein`/`moisture`/`fat`. Il manque un export client reel, une comparaison reference et la base projet native reste fermee. |
| BUCHI NIRCal | partiel | Le chemin `.nir` lit spectra/wavenumbers/proprietes; les cibles non nulles sont validees localement sur `transpec_DEMO_cannabis.nir`, et les zeros restent numeriques des qu'une table de proprietes contient de vraies valeurs. Restent une fixture redistribuable avec cibles non nulles, `.cal` calibration-only et variantes NIRMaster. |
| Perten DA / Inframatic | bloqué | Pas de fixture spectrale native; le CSV actuel est un rapport cible-seule sans axe spectral. Un export CSV/Excel avec colonnes de longueurs d'onde serait traitable par les readers tabulaires. |
| JASCO JWS | partiel | Les fixtures OLE2 `DataInfo`/`Y-Data` FT/IR transmittance, FP-8300 fluorescence et CD-1500/J-1500 CD/HT/Abs sont verrouillees par goldens/tests semantiques et probe; l'export texte JASCO est couvert par `row-spectral-table`. Restent blocs V-series NIR distincts, variantes Raman NRS et streams alternatifs (`Data`, `Header`, `XdataValue`). |
| Shimadzu UVProbe | partiel | Le `.txt` UVProbe synthetique est verrouille par golden/test semantique sur axe `nm`, signal `sample_s000` et titre `Spectrum Data`; la registry teste aussi que `.spc` n'est pas revendique par extension seule. Restent vrai `.txt`, vrai `.spc` Shimadzu et comparaison convertisseur/`pyfasma-spc`. |
| VIAVI MicroNIR | partiel | Reels CSV/XLSX MicroNIR 1700 committes et verrouilles par tests de lecture + probe (UvA forensic). `.pri` natif reste hors atteinte. |
| Si-Ware NeoSpectra | partiel | Reels OSSL Woodwell + UvA forensic XLSX committes et verrouilles par tests de lecture + probe; le descripteur OSSL non spectral est refuse explicitement. Reste a couvrir un export NeoSpectra Scanner natif single-measurement. |
| Spectro Inc. SiWare API | partiel | JSON natif `measurement.wavelengths`/`measurement.absorbance` et CSV axis-first sont verrouilles par goldens/tests semantiques. Les fixtures restent synthetiques; il manque une reponse API reelle, des variantes de schema et une comparaison reference sur les predictions, unites et metadata optionnelles. |
| JCAMP-DX | partiel | XYDATA/ASDF/NTUPLES/LINK Ocean Optics sont couverts par goldens elargis, y compris fichiers top-level multi-blocs (`nist_sucrose_ir.jdx` -> 2 records) et NTUPLES FID a axe `time`. Restent `LINK` generaux, `PEAK TABLE` apres extension du modele sparse, et plus de variantes NTUPLES. |
| NetCDF NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique, Microtops MAN, ARM MFRSR local et SURFSPECALB local derive sont couverts; PyrNet et AOSMET sont des refus attendus non spectraux. Restent schemas NIRS reels generiques, QC NetCDF4/HDF5 plus robuste et validation ACT/xarray. |
| AnIML | partiel | Les `SeriesSet` spectraux synthetiques sont couverts avec valeurs explicites et axe uniforme `AutoIncrementedValueSet`; `Example3.animl` est un sample AnIML reel non spectral refuse comme attendu. Restent vrais AnIML spectraux, indices segmentes non-zero, validation XSD et conformance avec tooling AnIML. |
| Allotrope ASM | partiel | Les trois fixtures Benchling spectrales/endpoints sont couvertes; restent conversions vendeurs multiples, cas ASM hors plate-reader et validation contre tooling Allotrope. |
| Allotrope ADF | partiel | `samples_local/allotrope_adf/adfsee_example.adf` valide la detection ADF, les `/data-cubes` numeriques, les titres de cubes, l'axe temps `SecondTimeValue` type `time`, la scale secondaire `NanometerValue` et les mesures `AbsorbanceUnitValue`. Restent l'ontologie Allotrope complete, les exports vendeurs, la validation SDK et un fixture redistribuable CI. |
| HDF5 NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique et les refus non-spectraux sont couverts; il manque schemas reels avec metadata, axes complexes et groupes multi-signaux. |
| MATLAB MAT / RData | partiel | MAT v5/v7.3 simples, DSO academiques, prospectr `NIRsoil.RData` et cube Indian Pines local-only sont couverts; restent structures MAT/RData generiques, cubes MAT v7.3 et metadata/targets heterogenes. |
| Renishaw WDF | partiel | Les 15 fixtures spectrales versionnees couvrent single, map, line, depth/zscan, FocusTrack, time-series, StreamLine et interrupted; les deux fixtures `measurement_type=0` sont des refus attendus. Les `MAP ` PSET observes exposent maintenant inventaire + `dataRange` derive par record quand la longueur matche le nombre de spectres. Restent autres layouts `MAP `, unites/algorithmes derives autoritaires, conformance full-array et fixtures par modele InVia Qontor/Apollo. |
| Horiba LabSpec / JobinYvon | partiel | `.l6m` reel Gd₂O₃/AlN map decode en mode experimental et compare integralement contre l'export texte (intensites + coordonnees); les axes XML `eV` sont types `energy`. Restent `.l6s`, autres layouts LabSpec6 et metadata complete. |
| Princeton TriVista TVF | partiel | Corpus RosettaSciIO couvert et golden-backed, y compris single/multi-frame, time series, line/map, multi-spectrometer et Step-and-Glue. Aucun sample bloquant connu dans le corpus actuel; restent conformance full-array automatisee contre `rsciio.trivista`, hardware/objective metadata plus riche et decision de scope pour variantes hors corpus. |
| DigitalSurf MountainsMap | partiel | Fixtures RosettaSciIO spectre, multi-spectres, hyperspectral maps, surface et zlib compresse/non compresse golden-backed. Aucun sample bloquant connu dans le corpus actuel; restent conformance full-array contre `rsciio.digitalsurf`, metadata surfaces plus riche et decision de scope pour variantes MountainsMap hors corpus/branded AFM-Raman. |
| Hamamatsu HPD-TA IMG | partiel | Les fixtures HPD-TA 2D adjacentes sont couvertes; rester explicitement adjacent tant qu'aucun export spectral point-sample Hamamatsu n'est cible. |
| WiTec WIP / WID | partiel | `Sa4.wip` reel decode en 4410 spectres TDGraph `WIT_PR06`; restent layouts WiTec generaux, coordonnees physiques, conversion Raman-shift et export ASCII equivalent pour comparaison. |
| fNIRS neuroscience | pas fait | Domaine physiologie hors scope; rediriger vers SNIRF/MNE-NIRS. Aucun sample fNIRS n'est present; les `.hdr` actuels sont ENVI et ne doivent pas etre routes par extension seule. |

## Verification locale du corpus (2026-05-20)

Dernier sweep CLI apres mise a jour de la matrice. Les compteurs ci-dessous
portent sur les fichiers evaluables par le CLI: les `README`, licences, PDF,
archives brutes, sidecars de documentation et YAML de QC sont exclus du
denominateur.

| Corpus | OK | Refus attendus | Refus inattendus | Notes |
|---|---:|---:|---:|---|
| `samples/` | 245 | 20 | 0 | Les refus attendus sont des formats volontairement non-NIRS, des fixtures negatives, des sidecars seuls (`92AV3C.spc`, `92AV3GT.GIS`, header Microtops), des workbooks metadata-only accompagnateurs, des rapports cible-seule Foss/Perten ou des descripteurs non spectraux (`neospectra_ossl_column_names.csv`). |
| `samples_local/` | 15 | 5 | 0 | Lectures OK: Indian Pines MATLAB v5, BUCHI cannabis, ARM MFRSR NetCDF + sidecar QC YAML, ARM SURFSPECALB derive, Allotrope ADF adfsee, 3 OMNIC `.srs` locaux et 7 exports Microtops MAN ASCII `.lev*`. Refus attendus: `_gt.mat` sidecar, NOAA `.lev2`, ARM AOSMET et PP Systems indices non raw/derives. |

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
| Microtops Sun Photometer | `samples/microtops/microtops_arc_msm114_2.nc` + `_header.txt` | [PANGAEA 966645](https://doi.pangaea.de/10.1594/PANGAEA.966645) (republished from AERONET MAN) | CC-BY-4.0 | `partiel` (synthetique seul) -> `partiel` (NetCDF MAN reel teste, AOT type, fallback fixture apres tentative generique); legacy `.TXT` et MAN generique sans fallback toujours absents |
| NetCDF NIRS-adjacent | `samples/netcdf/pyrnet_to_l1a_output.nc` | [`hdeneke/PyrNet`](https://github.com/hdeneke/PyrNet) | academic share | refusal non-NIRS teste: pas d'axe spectral ni de canaux Microtops AOT |
| Consumer Physics SCiO | `samples/scio/scio_app_scan.csv`, `scio_calibration_plate_Polypen.csv`, `scio_scans_from_tech_support.csv` | [`kebasaa/SCIO-read`](https://github.com/kebasaa/SCIO-read) | GPL-3 | `fait`: `band*`, calibration axis-first et groupes `spectrum`/`wr_raw`/`sample_raw` testes; ajoute aussi `excel/scio_forensic_*.xlsx` UvA Figshare en complement |

#### Fixtures non-redistribuables (uniquement en local — `samples_local/`, gitignore)

| Format | Fichier | Source | Licence / raison non-commit | Effet |
|---|---|---|---|---|
| Hyperspectral cube (AVIRIS Indian Pines) | `samples_local/hyperspectral_cubes/indian_pines_corrected.mat` + `_gt.mat` | [EHU/Grupo de Inteligencia Computacional](http://www.ehu.eus/ccwintco/index.php/Hyperspectral_Remote_Sensing_Scenes) | "academic use" sans SPDX clair → en local seulement | reader MAT v5 local-only teste: 21,025 spectres x 200 bandes + cible `land_cover_class`; la version `92AV3C.lan` plus petite reste committee |
| Allotrope ADF adfsee | `samples_local/allotrope_adf/adfsee_example.adf` | [`allotrope-open-source/adfsee`](https://gitlab.com/allotrope-open-source/adfsee) | ADF/ontology terms Allotrope, garder local | reader ADF experimental teste: 4 records depuis 3 data-cubes numeriques; RDF minimal mappe titres, axe temps type `time`, scale secondaire nm et absorbance mAU |
| Thermo Nicolet OMNIC SRS locaux | `samples_local/nicolet_omnic/spectrochempy_TGA_demo.srs`, `spectrochempy_rapid_scan.srs`, `spectrochempy_rapid_scan_reprocessed.srs` | [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) | CeCILL-B mais fichiers volumineux -> local seulement | TGA_demo absorbance, rapid-scan brut interferogramme/index et rapid-scan reprocessé absorbance sont testes localement; `.srsx` reste absent |
| Microtops MAN ASCII Okeanos | `samples_local/microtops/aeronet_man_Okeanos_19_2_*.lev10/.lev15/.lev20` | AERONET Maritime Aerosol Network | AERONET MAN PI/coauthorship policy -> en local seulement | reader local teste: AOD valides types `aerosol_optical_thickness`, canaux `-999` omis, AOD-STD pour exports daily/series |
| BUCHI NIRCal cannabis | `samples_local/buchi_nircal/transpec_DEMO_cannabis.nir` | orellano-c/transpec_info | licence non clarifiee pour redistribution du fixture -> en local seulement | reader local teste: 105 spectres, axe 1501 wavenumbers et cibles non nulles `CBDA`/`THCA` |
| ARM MFRSR / ARM NetCDF adjacents | `samples_local/mfr/*.nc`, `samples_local/netcdf/*.nc` | DOE ARM / ARM test data | ARM Data Use Policy -> en local seulement | MFRSR b1 local decode en 4,320 observations x 7 filtres avec sidecar QC YAML; SURFSPECALB local decode en 986 lignes utiles x 6 filtres; AOSMET reste non spectral |
| PP Systems Arctic LTER indices | `samples_local/pp_systems/*.csv/.xlsx` | Arctic LTER / EDI | dataset local non committe | produit derive NDVI/EVI/PRI/etc.; ne ferme pas le manque de raw `.SPT/.SPU` |
| Microtops `.lev2` disambiguation | `samples_local/microtops/noaa_lauder_sonde_la20170315.lev2` | [NOAA GML Lauder](https://gml.noaa.gov/aftp/data/ozwv/WaterVapor/Lauder_LEV/) | US Gov public domain MAIS le fichier est en realite un radiosonde water vapour/ozone, pas un sun-photometer Microtops | aide locale a la disambiguation `.lev2`; non commit pour eviter confusion |

### Formats restant fermes (sweep sans resultat exploitable, apres 3 passages)

| Format | Pourquoi pas trouve |
|---|---|
| ASD calibration `.ILL/.REF/.RAW` | Distribution vendeur SDK uniquement; SPECCHIO partiel derriere login partenariat; aucun GitHub/Wayback/Mendeley sample. |
| Foss `.NIR/.DA/.cal/.eqa` natif | Format ferme, aucune fixture binaire publique trouvee (Wayback FOSS / NIR-Predictor demos checked). |
| Perten DA / Inframatic | Pas de fixture native ni CSV reel public (clients only). |
| Metrohm Vision Air / OMNIS NIR natif | Format ferme, seul l'export CSV est documente publiquement. |
| Allotrope ADF vendeur | Le sample `adfsee` local ferme le manque "aucun ADF"; restent les ADF instrumentaux vendeurs (Waters/Sciex/Agilent/etc.), l'ontologie complete, les unites et la validation SDK Allotrope. |
| MODTRAN albedo `.dat` reel | Distribution sous licence MODTRAN/ONTAR ($2400) ; MIT OCW pcmodwin/RIT tutorials ne shippent que des references USGS deja couvertes. |
| MFR-7 / MFRSR `.OUT` reel | ARM Data Center exige compte; `samples_local/mfr/` ferme localement un NetCDF ARM MFRSR b1, mais pas un `.OUT` MFR-7 redistribuable — non commit. |
| Microtops II `.TXT` reel | AERONET MAN demande co-authorship; `samples_local/microtops/` ferme localement les exports MAN ASCII `.lev*`, mais pas un `.TXT` legacy redistribuable — non commit. |
| PP Systems UniSpec `.SPT/.SPU` reel raw | Aucune fixture raw `.spu/.spt` publique; `samples_local/pp_systems/` contient seulement des indices derives Arctic LTER — non commit. |
| Bruker OPUS 5/6 legacy | Archives privees, pas de mirror public; OPUS 7/8 couvert via 4 lecteurs independants suffit. |
| Thermo OMNIC `.srsx` | Pas de fixture publique trouvee (S.T.Japan demo bibliotheques `.spg` derriere formulaire); le canal `.srs`, y compris rapid-scan local, est couvert experimentalement. |
| Shimadzu UVProbe `.spc` natif | Un seul candidat (`uri-t/shimadzu-spc-converter`) sans licence claire; aucune autre source apres sweep. |
| VIAVI MicroNIR `.pri` natif | Format projet binaire, customer-only; CSV/XLSX exports reels deja couverts via UvA forensic. |
| Si-Ware NeoSpectra Scanner natif single-measurement | Le pipeline OSSL ne publie que des matrices wide; pas de fixture "1 mesure par CSV" publique. |
| Specim IQ demo cube | Specim a discontinue le produit (page "end-of-life"); seul l'archive 7z Arabidopsis Zenodo 1345007 (123 MB) existe — trop gros, et le mix raw/processed n'est pas isole. |
| NEON AOP HDF5 reflectance tile | Tiles 1 km × 1 km demandent inscription neon.science (compte gratuit mais distribution conditionnelle); fichier minimum ~50 MB. |
| Horiba `.l6s` single-spectrum | Aucune fixture publique trouvee; seul `.l6m` (map) committe. |
| JASCO V-780 NIR / NRS Raman `.jws` variants | Aucun sample distinct du V-770 IR + V-series UV-Vis deja committes. |
