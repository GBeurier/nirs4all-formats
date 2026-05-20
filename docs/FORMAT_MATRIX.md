# Matrice compacte des formats

Colonnes de pilotage:

- `Variants`: sous-formats, versions, layouts ou modes explicitement suivis.
- `Validés`: parsing et metadata suffisants pour le variant, avec sample/test/doc.
- `Partiels`: parser utile mais incomplet pour ce variant (metadata, calibration,
  conformance, axes ou cibles incomplets).
- `Planifiés`: variant identifié et actionnable, mais pas encore codé.
- `Bloqués`: variant identifié mais bloqué par sample/spec/licence/référence.
- `Couverture NIRS`: lecture produit de la ligne. `diffusable` signifie que les
  variants principaux en activité sont couverts; `diffusable ciblé` signifie
  diffusable si le périmètre est annoncé explicitement; `utile incomplet`
  demande encore du code avant communication forte; `non viable` demande
  d'abord des samples/specs ou un parser significatif.
- `Impact manque`: gravité métier du manque restant. `aucun` ou `mineur`
  n'empêche pas de diffuser si le périmètre est clair; `moyen` demande un
  complément utile mais les spectres principaux sont conservés; `grave`
  signifie qu'un variant actif, une calibration essentielle ou une partie
  significative des spectres manque; `bloquant` signifie qu'on ne peut pas
  revendiquer le format; `hors périmètre` signifie volontairement adjacent ou
  hors cible.
- `Popularité`: fréquence attendue en NIRS terrain/industrie/recherche, pas
  seulement le nombre de samples disponibles.
- `Priorité`: effort projet conseillé. `P0` conditionne la valeur NIRS
  industrielle ou terrain; `P1` ajoute une couverture forte; `P2` est utile ou
  adjacent; `P3` peut attendre.
- `Manque critique`: ce qui empêche de considérer la ligne comme pleinement
  couverte ou ce qu'il faut faire ensuite.

Règle de lecture: `Popularité` élevée + `Impact manque` `grave`/`bloquant` +
`Priorité` P0/P1 indique où chercher des fichiers originaux ou coder en premier.
`Couverture NIRS = diffusable` avec impact `aucun`/`mineur`/`moyen` indique un
périmètre publiable si les limites sont annoncées.

Tri: la matrice principale est triée par `Priorité`, puis `Impact manque`, puis
`Popularité`. Les tableaux de suivi sont triés par leur colonne de pilotage
visible (`Suivi actuel`, `Corpus` ou `Format`).

| Nom | Vendeur | Extensions | Variants | Validés | Partiels | Planifiés | Bloqués | Couverture NIRS | Impact manque | Popularité | Priorité | Manque critique | Lib référence |
|---|---|---|---:|---:|---:|---:|---:|---|---|---|---|---|---|
| Foss NIRSystems / WinISI natif | Foss | `.NIR`, `.DA`, `.cal`, `.eqa` | 4 | 0 | 0 | 0 | 4 | non viable | bloquant | tres courant industrie | P0 sourcer | Format industriel clé, aucune fixture native fiable. | aucune fiable |
| Perten DA / Inframatic | Perten / PerkinElmer | binaire vendeur, `.csv` | 2 | 0 | 0 | 0 | 2 | non viable | bloquant | tres courant industrie | P0 sourcer | Format industriel clé, aucun sample spectral natif/export exploitable. | export CSV/Excel vendeur |
| ASD calibration | ASD / Malvern Panalytical | `.ILL`, `.REF`, `.RAW` | 3 | 0 | 0 | 0 | 3 | non viable | bloquant | specialise | P1 sourcer | Samples compagnons absents; utile mais pas indispensable au reader `.asd`. | SPECCHIO, asdreader |
| PP Systems UniSpec DC | PP Systems | `.SPU` | 1 | 0 | 1 | 0 | 0 | non viable | bloquant | specialise terrain | P1 sourcer | Parser synthetic seulement; acquisition terrain deux canaux nécessaire. | SPECCHIO, parseurs ad hoc |
| PP Systems UniSpec SC | PP Systems | `.SPT` | 1 | 0 | 1 | 0 | 0 | non viable | bloquant | specialise terrain | P1 sourcer | Parser synthetic seulement; acquisition terrain réelle nécessaire. | SPECCHIO, parseurs ad hoc |
| Avantes AvaSoft 8 binaire | Avantes | `.Raw8`, `.IRR8`, `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8` | 9 | 1 | 1 | 0 | 7 | diffusable ciblé | grave | courant | P1 sourcer | Beaucoup de suffixes AVS8 sans fixture; `.IRR8` reste calibration partielle. | lightr, manuel AvaSoft |
| Metrohm Vision / Vision Air | Metrohm | `.csv`, `.xlsx`, base projet native | 3 | 1 | 0 | 1 | 1 | diffusable ciblé | grave | courant industrie | P1 sourcer | CSV ok; base native/exports réels client à obtenir. | parseur texte, pandas, readxl |
| HDF5 NIRS generique | Vendor-neutral | `.h5`, `.hdf5` | 3 | 1 | 0 | 2 | 0 | diffusable ciblé | grave | specialise recherche | P1 coder | Schéma canonique ok; HDF5 spectraux réels hétérogènes à coder. | h5py, hdf5-reader, tables |
| Spectro Inc. SiWare API | Spectro Inc. | `.json`, `.csv` | 3 | 2 | 0 | 0 | 1 | utile incomplet | grave | specialise | P1 sourcer | Fixtures synthétiques; réponse API réelle nécessaire avant forte diffusion. | JSON/CSV standard |
| ASD FieldSpec | ASD / Malvern Panalytical | `.asd` | 8 | 4 | 0 | 3 | 1 | diffusable | moyen | courant terrain | P1 sourcer | Revisions principales ok; chercher v3-v5 et blocs calibration internes. | asdreader, prospectr, spectrolab, specdal, pyASDReader |
| Avantes AvaSoft 6/7 binaire | Avantes | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` | 5 | 4 | 0 | 0 | 1 | diffusable ciblé | moyen | courant | P1 sourcer | `.ABS` binaire manque; périmètre actuel assez utile. | lightr |
| BUCHI NIRCal | BUCHI / Buhler | `.nir`, export JCAMP-DX | 4 | 1 | 1 | 1 | 1 | diffusable ciblé | moyen | courant industrie | P1 sourcer | `.nir` utile; cibles non nulles redistribuables et variantes NIRMaster manquent. | prospectr::read_nircal |
| JCAMP-DX | Vendor-neutral / IUPAC | `.jdx`, `.dx`, `.jcm`, `.jcamp` | 5 | 3 | 1 | 1 | 0 | diffusable ciblé | moyen | courant échange | P1 coder | XYDATA/NTUPLES ok; LINK général et PEAK TABLE à coder. | jcamp, SpectroChemPy, nmrglue, ChemoSpec, hyperSpec |
| Si-Ware NeoSpectra | Si-Ware | `.csv`, `.xlsx` | 3 | 2 | 0 | 0 | 1 | diffusable | mineur | courant handheld | P1 sourcer | Matrices réelles ok; single-measurement Scanner absent. | parseur texte, openpyxl |
| Spectral Evolution / PSR | Spectral Evolution | `.sed` | 4 | 3 | 1 | 0 | 0 | diffusable | mineur | courant terrain | P1 compléter | Couverture terrain utile; compléter unités/conformance SR variants. | spectrolab, specdal |
| SVC / GER SIG | Spectra Vista / GER | `.sig` | 6 | 5 | 1 | 0 | 0 | diffusable | mineur | courant terrain | P1 compléter | Très utile terrain; compléter radiance/conformance spectrolab/specdal. | spectrolab, specdal |
| VIAVI MicroNIR | VIAVI / JDSU | `.csv`, `.xlsx`, `.pri` | 3 | 2 | 0 | 0 | 1 | diffusable | mineur | courant handheld | P1 sourcer | Exports réels ok; `.pri` natif customer-only. | parseur texte, openpyxl |
| Allotrope ADF | Allotrope Foundation | `.adf` | 4 | 0 | 2 | 0 | 2 | non viable | bloquant | emergent industrie | P2 compléter | ADF local partiel; SDK/ontologie/fixtures vendeurs manquent. | Allotrope SDK, adfsee |
| Horiba LabSpec / JobinYvon | Horiba | `.xml`, `.txt`, `.l6s`, `.l6m` | 5 | 2 | 1 | 0 | 2 | adjacent ciblé | grave | courant Raman | P2 sourcer | XML/TXT ok; `.l6m` expérimental, `.l6s`/layouts LabSpec6 absents. | RosettaSciIO, SpectroChemPy, horiba-raman |
| WiTec WIP / WID | WiTec | `.wip`, `.wid`, `.txt` | 5 | 1 | 1 | 1 | 2 | adjacent ciblé | grave | courant Raman | P2 coder | Un layout WIP ok; Raman-shift/coordonnées/layouts généraux à coder/sourcer. | pynxtools-raman, hySpc.read.Witec, LabberI2A WIPfile |
| AnIML | IUPAC / ASTM | `.animl` | 5 | 2 | 0 | 2 | 1 | utile incomplet | grave | emergent/niche | P2 sourcer | Vrais AnIML spectraux/XSD/conformance manquent. | animl-python, validateurs XML |
| FGI HDF5 + XML | FGI | `.h5`, `.hdf5`, `.xml` | 2 | 1 | 0 | 0 | 1 | diffusable ciblé | grave | niche | P2 sourcer | Paire réelle FGI absente; synthetic uniquement. | h5py, hdf5r, rhdf5, lxml |
| Bruker OPUS natif | Bruker | `.0`, `.1`, `.001`, `.0000`, sans extension fixe | 5 | 2 | 1 | 0 | 2 | diffusable | moyen | tres courant | P2 compléter | OPUS 7/8 et MPA ok; OPUS 5/6 legacy et imaging restent secondaires. | opusreader2, hyperSpec.utils, brukeropusreader, brukeropus, opusFC, SpectroChemPy |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | Ocean Optics / Ocean Insight | `.txt`, `.csv`, `.jaz`, `.JazIrrad`, `.Master.Transmission`, `.ProcSpec`, `.jdx`, `.spc` | 11 | 8 | 0 | 3 | 0 | diffusable | moyen | tres courant | P2 compléter | Large couverture active; ajouter QE Pro/Maya/Apex si samples. | lightr, pavo |
| Thermo / Galactic GRAMS SPC | Thermo / Galactic | `.spc`, `.SPC` | 6 | 3 | 1 | 1 | 1 | diffusable | moyen | tres courant | P2 compléter | New/old LSB utiles; BE et vieux logs restent secondaires. | spc-spectra, rohanisaac/spc, specio, SpectroChemPy, xylib, spc-parser |
| Thermo Nicolet OMNIC | Thermo Nicolet | `.spa`, `.spg`, `.srs`, `.srsx` | 5 | 3 | 1 | 0 | 1 | diffusable | moyen | tres courant | P2 compléter | SPA/SPG/SRS utiles; `.srsx` absent, axe secondaire SRS à enrichir. | SpectroChemPy, spa-on-python |
| Bruker Tango / MPA / Matrix | Bruker | OPUS natif | 3 | 1 | 0 | 2 | 0 | diffusable ciblé | moyen | courant | P2 sourcer | MPA couvert; chercher Tango/Matrix dédiés pour marketing vendor. | opusreader2, SpectroChemPy |
| ENVI / hyperspectral cubes | ENVI / Specim / HySpex / Headwall / NEON / AVIRIS | `.dat`, `.img` + `.hdr`, HDF5, `.lan`, `.mat` | 7 | 3 | 1 | 1 | 2 | diffusable ciblé | moyen | courant HSI | P2 sourcer | Cubes ENVI/AVIRIS ok; Specim/HySpex/NEON/HDF5 restent à sourcer. | spectral, rasterio, scipy |
| JASCO JWS | JASCO | `.jws`, `.txt` | 7 | 4 | 0 | 0 | 3 | diffusable | moyen | courant lab | P2 sourcer | Variants NIR/Raman JWS absents; principaux streams publics couverts. | jws2txt, jwsProcessor |
| MATLAB MAT / RData | MATLAB / R ecosystem | `.mat`, `.MAT`, `.RData` | 6 | 5 | 1 | 0 | 0 | diffusable | moyen | courant recherche | P2 compléter | Couverture utile ML; structures arbitraires à élargir. | scipy, hdf5-reader, R serialization, prospectr |
| Perkin Elmer Spectrum / IR | PerkinElmer | `.sp`, `.fsm` | 2 | 1 | 0 | 0 | 1 | diffusable ciblé | moyen | courant | P2 sourcer | `.sp` ok; `.fsm` imaging hors périmètre v1. | specio |
| Renishaw WDF | Renishaw | `.wdf` | 12 | 9 | 1 | 0 | 2 | adjacent diffusable | moyen | courant Raman | P2 compléter | Raman adjacent très couvert; compléter MAP layouts/conformance. | RosettaSciIO, SpectroChemPy |
| Shimadzu UVProbe | Shimadzu | `.spc`, `.txt` | 2 | 1 | 0 | 0 | 1 | diffusable ciblé | moyen | courant lab | P2 sourcer | `.txt` ok; `.spc` natif manquant. | pyfasma-spc, convertisseur Shimadzu |
| Allotrope ASM | Allotrope / Benchling | `.json` | 3 | 2 | 0 | 1 | 0 | diffusable ciblé | moyen | emergent industrie | P2 sourcer | Benchling ok; conversions vendeurs à obtenir. | Benchling allotropy |
| NetCDF NIRS generique | Vendor-neutral | `.nc`, `.cdf` | 5 | 3 | 1 | 1 | 0 | diffusable ciblé | moyen | specialise recherche | P2 compléter | Schémas dédiés ok; generic NetCDF spectral réel à élargir. | netcdf-reader, xarray, netcdf, ARM ACT |
| MFR Sun Photometer | Solar Light / YES Inc. | `.OUT`, `.nc` local | 3 | 2 | 0 | 0 | 1 | diffusable ciblé | moyen | niche | P2 sourcer | `.OUT` réel redistribuable absent; NetCDF ARM local seulement. | SPECCHIO, parseurs ad hoc, xarray, ARM ACT |
| Microtops Sun Photometer | Solar Light | `.TXT`, `.nc`, `.lev10/.lev15/.lev20` | 4 | 2 | 1 | 0 | 1 | diffusable ciblé | moyen | niche | P2 sourcer | MAN NetCDF/ASCII ok; `.TXT` legacy réel et NetCDF générique restent incomplets. | parseurs ad hoc, xarray |
| Excel spectral | Generique / lab | `.xlsx`, `.xlsm`, `.xls` | 3 | 2 | 0 | 0 | 1 | diffusable | mineur | courant | P2 sourcer | `.xls` legacy OLE manque; non bloquant pour diffusion moderne. | calamine, openpyxl, pandas, readxl |
| USGS SPECPR / PRISM / ECOSTRESS text | USGS / JHU / ECOSTRESS | `SPECPR`, `.asc`, `.txt`, `.spectrum.txt` | 4 | 3 | 0 | 0 | 1 | diffusable | mineur | courant datasets | P2 sourcer | Textes ok; binaire SPECPR manque mais peu bloquant v1. | convertisseur USGS |
| ENVI Spectral Library | L3Harris / ENVI | `.sli` + `.hdr` | 3 | 2 | 0 | 0 | 1 | diffusable | mineur | specialise | P3 diffuser | `.slb` non fixture; faible impact NIRS. | spectral, RStoolbox, pysptools |
| DigitalSurf MountainsMap | DigitalSurf | `.sur`, `.pro` | 5 | 5 | 0 | 0 | 0 | adjacent diffusable | mineur | niche adjacent | P3 diffuser | Aucun sample bloquant connu; AFM/Raman adjacent. | RosettaSciIO |
| Princeton TriVista TVF | Princeton Instruments | `.tvf` | 8 | 8 | 0 | 0 | 0 | adjacent diffusable | mineur | niche Raman | P3 diffuser | Aucun sample bloquant connu; Raman adjacent. | RosettaSciIO |
| Foss / WinISI / DS exports | Foss | `.txt`, `.csv` | 2 | 2 | 0 | 0 | 0 | diffusable | aucun | tres courant industrie | P3 diffuser | Aucun; ne remplace pas le natif Foss. | parseur texte |
| Tables axe-first | Generique / exports instrument | `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU` | 8 | 8 | 0 | 0 | 0 | diffusable | aucun | tres courant | P3 diffuser | Aucun; couvre beaucoup d'exports vendors. | pandas, read.table |
| Tables spectrales delimitees | Generique | `.csv`, `.tsv`, `.txt` | 3 | 3 | 0 | 0 | 0 | diffusable | aucun | tres courant | P3 diffuser | Aucun; base utile pour imports externes. | pandas, read.table, nirs4all CSVLoader |
| Avantes ASCII | Avantes | `.ttt`, `.trt`, `.tit`, `.tat`, `.IRR`, `.txt` | 6 | 6 | 0 | 0 | 0 | diffusable | aucun | courant | P3 diffuser | Aucun; bon chemin recommandé quand export disponible. | pandas, read.table |
| Bruker OPUS DPT | Bruker | `.dpt` | 1 | 1 | 0 | 0 | 0 | diffusable | aucun | courant | P3 diffuser | Aucun; export ASCII seulement. | pandas, read.table, lightr |
| Consumer Physics SCiO | Consumer Physics | `.csv` (developer app) | 3 | 3 | 0 | 0 | 0 | diffusable | aucun | courant handheld | P3 diffuser | Aucun; handheld NIR utile. | kebasaa/SCIO-read |
| Matrices spectrales | Generique / Foss / Metrohm / VIAVI | `.csv`, `.txt` | 3 | 3 | 0 | 0 | 0 | diffusable | aucun | courant | P3 diffuser | Aucun; utile pour ML et exports wide. | pandas, read.table |
| NumPy | Python / NumPy | `.npy`, `.npz` | 2 | 2 | 0 | 0 | 0 | diffusable | aucun | courant data | P3 diffuser | Aucun; utile bindings Python. | numpy |
| Parquet | Apache / generique | `.parquet` | 1 | 1 | 0 | 0 | 0 | diffusable | aucun | courant data | P3 diffuser | Aucun; format de distribution interne utile. | pyarrow, fastparquet, nirs4all ParquetLoader |
| IDL / ENVI texte | IDL / ENVI | `.txt` | 1 | 1 | 0 | 0 | 0 | diffusable | aucun | specialise | P3 diffuser | Aucun. | parseur texte |
| EMSA/MAS MSA | ISO / EMSA | `.msa` | 3 | 3 | 0 | 0 | 0 | adjacent diffusable | aucun | adjacent | P3 diffuser | Aucun; surtout microscopie/spectro adjacent. | RosettaSciIO |
| Hamamatsu HPD-TA IMG | Hamamatsu | `.img` | 2 | 2 | 0 | 0 | 0 | adjacent | hors périmètre | niche adjacent | P3 surveiller | Hors point-spectra NIRS; garder comme disambiguation. | RosettaSciIO |
| MODTRAN albedo | Spectral Sciences / AFRL | `.dat` | 1 | 0 | 1 | 0 | 0 | non viable | hors périmètre | niche | P3 sourcer | Non coeur NIRS; sample réel redistribuable absent. | parseur texte |
| ANDI / NetCDF MS | ASTM / vendor-neutral | `.cdf`, `.nc` | 1 | 1 | 0 | 0 | 0 | adjacent | hors périmètre | adjacent | P3 surveiller | Refus non-NIRS utile pour disambiguation. | pyteomics, PyMassSpec, pyOpenMS |
| mzML / mzMLb | HUPO PSI / MS vendors | `.mzML`, `.mzMLb` | 2 | 1 | 0 | 0 | 1 | adjacent | hors périmètre | adjacent | P3 surveiller | Refus non-NIRS; `.mzMLb` seulement documenté. | pyteomics, pymzML, pyOpenMS |
| fNIRS neuroscience | NIRx / SNIRF ecosystem | `.snirf`, `.nirs`, `.wl1`, `.wl2`, `.hdr` | 5 | 0 | 0 | 0 | 5 | hors-scope | hors périmètre | hors-scope | P3 hors-scope | Physiologie non ciblée par nirs4all-io spectroscopy. | MNE-NIRS, SNIRF |

## Notes pour les lignes non finalisées

Les lignes `Couverture NIRS = diffusable` peuvent rester listees quand il
existe encore des variants secondaires a sourcer, coder ou completer, tant que
l'`Impact manque` reste `mineur` ou `moyen`. La note indique le manque concret:
sample, metadata, calibration, conformance, variant non code ou périmètre hors
NIRS.

| Nom | Suivi actuel | Note / manque |
|---|---|---|
| Foss NIRSystems / WinISI natif | bloqué | Format ferme sans lecteur fiable ni fixture binaire de reference; les samples Foss actuels sont des exports CSV/texte et les `.nir` presents sont BUCHI NIRCal, donc ne debloquent pas `.NIR/.DA/.cal/.eqa`. |
| Perten DA / Inframatic | bloqué | Pas de fixture spectrale native; le CSV actuel est un rapport cible-seule sans axe spectral. Un export CSV/Excel avec colonnes de longueurs d'onde serait traitable par les readers tabulaires. |
| ASD calibration | bloqué | Obtenir un jeu redistribuable `.asd` + `.ILL/.REF/.RAW`; les samples `.asd` actuels ne contiennent pas les compagnons calibration, et le `.REF` present dans `samples/avantes/` est Avantes, pas ASD. |
| fNIRS neuroscience | pas fait | Domaine physiologie hors scope; rediriger vers SNIRF/MNE-NIRS. Aucun sample fNIRS n'est present; les `.hdr` actuels sont ENVI et ne doivent pas etre routes par extension seule. |
| PP Systems UniSpec DC | partiel | Le `.SPU` synthetique est verrouille par golden et test semantique sur axe `nm`, metadata header, `channel_a_dn`/`channel_b_dn` raw et reflectance. Il manque une acquisition terrain reelle pour valider les deux canaux et metadata UniSpec DC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| PP Systems UniSpec SC | partiel | Le `.SPT` synthetique est verrouille par golden et test semantique sur axe `nm`, metadata header, `dn_white`/`dn_target` raw et reflectance. Il manque une acquisition terrain reelle pour valider headers, units et metadata UniSpec SC. Les indices Arctic LTER locaux sont des produits derives, pas des spectres raw UniSpec. |
| Avantes AvaSoft 8 binaire | partiel | `.Raw8` et `.IRR8` sont couverts par fixtures/goldens/tests semantiques et probe (`AVS84`, modes 0/4), avec date/heure SPC promues en `acquisition_start_date` / `acquisition_start_time`; restent `.RWD8/.ABS8/.TRM8/.RFL8/.RIR8/.RMN8/.RMD8`, multi-subfile AVS8 et calibration irradiance complete pour `.IRR8`. |
| Metrohm Vision / Vision Air | partiel | Le CSV Vision Air synthetique est verrouille par golden et test semantique sur 50 records, axe `nm`, signal absorbance et cibles `protein`/`moisture`/`fat`. Il manque un export client reel, une comparaison reference et la base projet native reste fermee. |
| HDF5 NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique et les refus non-spectraux sont couverts; il manque schemas reels avec metadata, axes complexes et groupes multi-signaux. |
| Spectro Inc. SiWare API | partiel | JSON natif `measurement.wavelengths`/`measurement.absorbance` et CSV axis-first sont verrouilles par goldens/tests semantiques. Les fixtures restent synthetiques; il manque une reponse API reelle, des variantes de schema et une comparaison reference sur les predictions, unites et metadata optionnelles. |
| ASD FieldSpec | partiel | Revisions 1/6/7/8 primary spectra couvertes; restent v3/v4/v5 eventuelles, blocs internes secondary/dependent/reference/calibration, audit/signatures et compagnons calibration `.ILL/.REF/.RAW` separes. |
| Avantes AvaSoft 6/7 binaire | partiel | Deux fixtures `.TRM` et les modes `.ROH/.DRK/.REF` sont golden-backed avec tests semantiques et probes verrouilles pour chaque suffixe disponible. Restent `.ABS` et autres modes binaires legacy puis comparaison `lightr`; le `.IRR` present est un export ASCII couvert par Avantes ASCII, pas une preuve du binaire legacy. |
| BUCHI NIRCal | partiel | Le chemin `.nir` lit spectra/wavenumbers/proprietes; les cibles non nulles sont validees localement sur `transpec_DEMO_cannabis.nir`, et les zeros restent numeriques des qu'une table de proprietes contient de vraies valeurs. Restent une fixture redistribuable avec cibles non nulles, `.cal` calibration-only et variantes NIRMaster. |
| JCAMP-DX | partiel | XYDATA/ASDF/NTUPLES/LINK Ocean Optics sont couverts par goldens elargis, y compris fichiers top-level multi-blocs (`nist_sucrose_ir.jdx` -> 2 records) et NTUPLES FID a axe `time`. Restent `LINK` generaux, `PEAK TABLE` apres extension du modele sparse, et plus de variantes NTUPLES. |
| Si-Ware NeoSpectra | partiel | Reels OSSL Woodwell + UvA forensic XLSX committes et verrouilles par tests de lecture + probe; le descripteur OSSL non spectral est refuse explicitement. Reste a couvrir un export NeoSpectra Scanner natif single-measurement. |
| Spectral Evolution / PSR | partiel | PSR DN brett + PSR-3500 grape leaf reels committes; le fichier DN-only broken-but-valid est teste comme deux signaux raw avec `sed_missing_reflectance_signal` / `missing_reflectance_signal`, et les champs GPS/date/time parseables sont promus en metadata canonique. Restent SR-3500 / SR-6500 firmware specifics, typage plus fin des unites et conformance `spectrolab`/`specdal`. |
| SVC / GER SIG | partiel | Les 15 fixtures committes sont golden-backed avec assertions semantiques directes pour SVC laptop, SVC PDA Acer clean/white-reference, matched-overlap-corrected, deux BAD declares, GER 3700 PDA et BEO HR-1024i field. Le lecteur promeut maintenant date/heure, GPS SVC `ddmm.mmmmN` / `dddmm.mmmmW`, GPS time, unites source et flags `overlap_removed`/`resampled_export`. Restent les firmware HR-1024i >=3.0, l'unite physique radiance quand elle est fournie par le vendeur et les comparaisons automatisees `spectrolab`/`specdal`. |
| VIAVI MicroNIR | partiel | Reels CSV/XLSX MicroNIR 1700 committes et verrouilles par tests de lecture + probe (UvA forensic). `.pri` natif reste hors atteinte. |
| Allotrope ADF | partiel | `samples_local/allotrope_adf/adfsee_example.adf` valide la detection ADF, les `/data-cubes` numeriques, les titres de cubes, l'axe temps `SecondTimeValue` type `time`, la scale secondaire `NanometerValue` et les mesures `AbsorbanceUnitValue`. Restent l'ontologie Allotrope complete, les exports vendeurs, la validation SDK et un fixture redistribuable CI. |
| Horiba LabSpec / JobinYvon | partiel | `.l6m` reel Gd₂O₃/AlN map decode en mode experimental et compare integralement contre l'export texte (intensites + coordonnees); les axes XML `eV` sont types `energy`, et les branches XML range/linescan sont verrouillees par tests semantiques. Restent `.l6s`, autres layouts LabSpec6 et metadata complete. |
| WiTec WIP / WID | partiel | `Sa4.wip` reel decode en 4410 spectres TDGraph `WIT_PR06`, avec validation stricte `LineValid` booleenne et metadata diagnostique: 4950 slots physiques, 49 lignes valides, 6 lignes invalides, index physique et calibration `FreePolynom`. Restent layouts WiTec generaux, coordonnees physiques, conversion Raman-shift et export ASCII equivalent pour comparaison. |
| AnIML | partiel | Les `SeriesSet` spectraux synthetiques sont couverts avec valeurs explicites et axe uniforme `AutoIncrementedValueSet`; `Example3.animl` est un sample AnIML reel non spectral refuse comme attendu. Restent vrais AnIML spectraux, indices segmentes non-zero, validation XSD et conformance avec tooling AnIML. |
| FGI HDF5 + XML | partiel | Sidecar XML synthetique mappe vers HDF5 et provenance double; reste a valider une paire FGI reelle et le schema XML complet. |
| Bruker OPUS natif | partiel | Tout le corpus commite `samples/bruker_opus/` est golden-backed et les fixtures cross-reader restantes ont des tests semantiques directs: spectral-cockpit/opusreader2, pierreroudier/opusreader, brukeropus MIT, SpectroChemPy et cran soil.spec AfSIS/MPA. Les axes OPUS `MIN` sont maintenant types `time` quand rencontres. Restent OPUS 5/6 legacy archives, blocs 2D/imaging et conformance full-array automatisee contre lecteurs externes. |
| Ocean Optics SpectraSuite / OceanView / Jaz / CRAIC | partiel | Les 12 samples Ocean Optics committes sont golden-backed: textes SpectraSuite/OceanView/Jaz/JazIrrad/CRAIC/CSV/Master.Transmission, ProcSpec Linux/Windows types transmittance et white-reference type reflectance via XML core processor / `yUnits`, JCAMP LINK via `jcamp-dx` et `.spc` OceanView route Galactic. Restent QE Pro/Maya/Apex, vrai `.spc` Ocean non-Galactic, typage des Jaz/textes generiques sans metadata explicite et rapports reference `lightr`/`pavo`. |
| Thermo / Galactic GRAMS SPC | partiel | Golden coverage elargie au corpus IR/Raman/UV-vis/NIR/NMR-FID ouvert, avec tests semantiques directs pour multi-subfile generated-X, directory-backed `TXYXYS`, old ordered-Z limite et axes SPC minute/seconde types `time` sur `s_xy.spc` et `NMR_FID.SPC`. Restent new big-endian `0x4C`, vieux headers/logs complets et decision de périmètre finale pour NMR/FID. |
| Thermo Nicolet OMNIC | partiel | SPA/SPG/SRS TGA-GC sont verrouilles par goldens/tests semantiques sur le corpus committe, y compris matrice 2D, offsets et metadata `series_y_*`; les trois `.srs` locaux SpectroChemPy couvrent `tg_gc`, `rapid_scan_raw` et `rapid_scan_reprocessed`. Reste `.srsx` et davantage de variantes high-speed. |
| Bruker Tango / MPA / Matrix | partiel | AfSIS Bruker MPA `icr_*.0` reels committes (cran/soil.spec). Reste Bruker Tango FT-NIR dedie et metadata MPA/Matrix complete. |
| ENVI / hyperspectral cubes | partiel | ENVI Standard `.hdr` et entree directe `.img/.dat` sont charges en spectres par pixel avec `map info` parse, unite spatiale normalisee, projection/reference/pixel-size et ordre `row_slowest_x_fastest`; AVIRIS/Indian Pines `.lan/.spc/.GIS` et le cube MATLAB local-only `indian_pines_corrected.mat` sont aussi couverts. Restent ERDAS LAN generique, NEON/Specim/HySpex/Headwall, HDF5 cubes et API masque/extraction. |
| JASCO JWS | partiel | Les fixtures OLE2 `DataInfo`/`Y-Data` FT/IR transmittance, FP-8300 fluorescence et CD-1500/J-1500 CD/HT/Abs sont verrouillees par goldens/tests semantiques et probe; l'export texte JASCO est couvert par `row-spectral-table`. Restent blocs V-series NIR distincts, variantes Raman NRS et streams alternatifs (`Data`, `Header`, `XdataValue`). |
| MATLAB MAT / RData | partiel | MAT v5/v7.3 simples, DSO academiques, prospectr `NIRsoil.RData` et cube Indian Pines local-only sont couverts; restent structures MAT/RData generiques, cubes MAT v7.3 et metadata/targets heterogenes. |
| Perkin Elmer Spectrum / IR | partiel | Le `.sp` PEPE mono-spectre reel `specio` est golden-backed et teste semantiquement. Le statut reste partiel parce que la ligne inclut `.fsm` Spotlight imaging, detecte/refuse comme hors v1, et parce qu'il manque des variantes PE NIR/Lambda; une separation future `.sp` vs `.fsm` permettrait de promouvoir le périmètre `.sp`. |
| Renishaw WDF | partiel | Les 15 fixtures spectrales versionnees couvrent single, map, line, depth/zscan, FocusTrack, time-series, StreamLine et interrupted; les deux fixtures `measurement_type=0` sont des refus attendus. Les `MAP ` PSET observes exposent maintenant inventaire + `dataRange` derive par record quand la longueur matche le nombre de spectres, et les fixtures map/depth `dataRange` sont golden-backed. Restent autres layouts `MAP `, unites/algorithmes derives autoritaires, conformance full-array et fixtures par modele InVia Qontor/Apollo. |
| Shimadzu UVProbe | partiel | Le `.txt` UVProbe synthetique est verrouille par golden/test semantique sur axe `nm`, signal `sample_s000` et titre `Spectrum Data`; la registry teste aussi que `.spc` n'est pas revendique par extension seule. Restent vrai `.txt`, vrai `.spc` Shimadzu et comparaison convertisseur/`pyfasma-spc`. |
| Allotrope ASM | partiel | Les trois fixtures Benchling spectrales/endpoints sont couvertes; restent conversions vendeurs multiples, cas ASM hors plate-reader et validation contre tooling Allotrope. |
| NetCDF NIRS generique | partiel | Le schema `spectra+wavelengths` synthetique, Microtops MAN, ARM MFRSR local et SURFSPECALB local derive sont couverts; PyrNet et AOSMET sont des refus attendus non spectraux. Restent schemas NIRS reels generiques, QC NetCDF4/HDF5 plus robuste et validation ACT/xarray. |
| MFR Sun Photometer | partiel | Le `.OUT` synthetique valide le parseur texte; le MFRSR NetCDF ARM local est decode en 4,320 enregistrements x 7 filtres avec signaux hemispheric/diffuse/direct/alltime/ratio, QC NetCDF et sidecar YAML de plages suspectes/incorrectes. Restent un dump MFR-7/MFRSR redistribuable, un mapping ARM plus large (`_FillValue`, calibration, filtres) et comparaison ACT/xarray. |
| Microtops Sun Photometer | partiel | MAN NetCDF reel committe et teste (PANGAEA MSM114/2, CC-BY-4.0). Le reader tente une decouverte generique `aot_<nm>`, mais le payload MSM114/2 reste lu via fallback SHA-256 car `hdf5-reader` ne resout pas encore ce layout NetCDF4/HDF5. Les 7 exports AERONET MAN ASCII locaux `.lev10/.lev15/.lev20` sont testes avec AOD et AOD-STD; les signaux primaires AOT sont types `aerosol_optical_thickness`, et `aot_std` est type `uncertainty`. Restent un vrai `.TXT` legacy redistribuable et un lecteur MAN NetCDF generique sans fallback. |
| Excel spectral | partiel | `.xlsx` synthetique/multi-feuilles/reels UvA et `.xlsm` OOXML macro-compatible sont golden-backed; workbooks metadata-only AuroraNIR/Foss XDS refuses explicitement. Restent `.xls` legacy OLE, un vrai `.xlsm` avec macros si besoin de metadata VBA, plus de fixtures multi-feuilles reelles et les cas ou Excel convertit les longueurs d'onde en dates. |
| USGS SPECPR / PRISM / ECOSTRESS text | partiel | ASCII `.asc`, ECOSTRESS/ASTER `.spectrum.txt` et AREF single-column sont couverts; restent le binaire SPECPR et les axes vrais pour dumps AREF sans sidecar. |
| DigitalSurf MountainsMap | partiel | Fixtures RosettaSciIO spectre, multi-spectres, hyperspectral maps, surface et zlib compresse/non compresse golden-backed. Les maps exposent maintenant `map_x_index`/`map_y_index` et `map_axis_order`; les surfaces exposent `spatial_y_index`, unites X/Y et `surface_axis_order`. Aucun sample bloquant connu; restent conformance full-array contre `rsciio.digitalsurf`, metadata objet/commentaire plus riche et decision de périmètre pour variantes MountainsMap hors corpus/branded AFM-Raman. |
| Princeton TriVista TVF | partiel | Corpus RosettaSciIO couvert et golden-backed, y compris single/multi-frame, time series, line/map, multi-spectrometer et Step-and-Glue. L'axe spectral est derive de `xDim/Calibration`, `xDim@Length` et `Frame@xDim` sont valides, les metadata detector/spectrometer numerotees sont promues, et les unites spatiales absentes restent explicites (`unknown`) au lieu d'etre inventees. Aucun sample bloquant connu; restent conformance full-array automatisee contre `rsciio.trivista`, objective/hardware-branch metadata plus riche et decision de périmètre pour variantes hors corpus. |
| Hamamatsu HPD-TA IMG | partiel | Les fixtures HPD-TA 2D adjacentes sont couvertes, avec axes Y calibres temporels exposes en metadata `time` et axes detecteur non calibres conserves en `index`. Rester explicitement adjacent tant qu'aucun export spectral point-sample Hamamatsu n'est cible. |
| MODTRAN albedo | partiel | Le `.dat` synthetique valide l'axe-first; il manque une sortie MODTRAN redistribuable sous licence claire. |

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
| AVIRIS / hyperspectral cubes | `samples/hyperspectral_cubes/92AV3C.lan`, `92AV3C.spc`, `92AV3GT.GIS` | Public Indian Pines / AVIRIS fixture already mirrored locally | dataset terms to confirm before release | `partiel` (`92AV3C` ERDAS LAN decode experimental) |
| Excel spectral | `samples/excel/scio_forensic_P_avg.xlsx`, `nirone_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — Consumer Physics SCiO + Spectral Engines NIRone 2.0 | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (vrais XLSX vendeurs handheld) |
| Foss / WinISI / DS exports | `samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv`, `foss_xds_barleyground_sensAIfood.csv` (+metadata) | [Zenodo 16759587](https://zenodo.org/records/16759587) — sensAIfood Univ. Cordoba (Foss XDS XM-1000 + NIRSYSTEM-5000) | CC-BY-4.0 | `partiel` → `fait` |
| Horiba LabSpec / JobinYvon | `samples/raman_horiba/AlN_Gd2O3_indepth.l6m` | [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) | MIT | `partiel` (XML/TXT seul) → `partiel` (`.l6m` decode experimental) |
| Si-Ware NeoSpectra | `samples/siware_neospectra/neospectra_ossl_column_names.csv`, `neospectra_ossl_50samples_slice.csv`, `neospectra_forensic_K_avg.xlsx` | [Zenodo 13122321 OSSL](https://zenodo.org/records/13122321) + [Figshare 21252300 UvA forensic](https://doi.org/10.21942/uva.21252300) | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (vrais clients OSSL + forensique) |
| Tables spectrales delimitees (handheld) | `samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv` (+metadata) | [Zenodo 15838272](https://zenodo.org/records/15838272) — sensAIfood Grainit (AuroraNIR 950-1650 nm) | CC-BY-4.0 | bonus handheld miniaturise |
| VIAVI MicroNIR | `samples/viavi_micronir/micronir_forensic_K_avg.xlsx`, `micronir_forensic_T_avg.xlsx` | [Figshare 21252300](https://doi.org/10.21942/uva.21252300) — MicroNIR 1700 forensique UvA | CC-BY-4.0 | `partiel` (synthetique seul) → `partiel` (CSV/XLSX reels) |
| WiTec WIP / WID | `samples/raman_witec/Sa4.wip` | [Zenodo 7907659](https://zenodo.org/records/7907659) — analyse Raman ZrO₂ | ODbL v1.0 | `partiel` (ASCII seul) → `partiel` (`WIT_PR06` TDGraph decode experimental) |

### Sweep d'echantillons publics (2026-05-20 — second passage)

Apres le premier passage, recherche etendue sur PANGAEA, GitLab Allotrope,
github.com/pierreroudier/opusreader, github.com/joshduran/brukeropus,
github.com/cran/soil.spec, github.com/serbinsh/R-FieldSpectra,
github.com/capstone-coal/pycoal, github.com/hdeneke/PyrNet,
github.com/kebasaa/SCIO-read, ehu.eus/ccwintco (Indian Pines), NOAA Lauder.

#### Nouveaux fixtures committes (second passage)

| Format | Fichier ajoute | Source | Licence | Effet matrice |
|---|---|---|---|---|
| Bruker OPUS natif (cross-reader) | `samples/bruker_opus/brukeropus_file.0`, `opusreader_test_spectra.0`, `icr_087266_B2.0`, `icr_087273_G3.0` | [`joshduran/brukeropus`](https://github.com/joshduran/brukeropus) (MIT), [`pierreroudier/opusreader`](https://github.com/pierreroudier/opusreader) (GPL-3), [`cran/soil.spec`](https://github.com/cran/soil.spec) AfSIS (GPL-2/3) | mixte (MIT + GPL) | reste `partiel` mais couverture cross-vendor elargie |
| Consumer Physics SCiO | `samples/scio/scio_app_scan.csv`, `scio_calibration_plate_Polypen.csv`, `scio_scans_from_tech_support.csv` | [`kebasaa/SCIO-read`](https://github.com/kebasaa/SCIO-read) | GPL-3 | `fait`: `band*`, calibration axis-first et groupes `spectrum`/`wr_raw`/`sample_raw` testes; ajoute aussi `excel/scio_forensic_*.xlsx` UvA Figshare en complement |
| ENVI Spectral Library | `samples/envi_sli/usgs_splib06a_aviris95_envi.sli|hdr` + `usgs_splib07_aviris95_envi.sli|hdr` | [`capstone-coal/pycoal`](https://github.com/capstone-coal/pycoal) | GPL-2 (wrapper) + USGS public domain (data) | `partiel` → `fait` |
| Microtops Sun Photometer | `samples/microtops/microtops_arc_msm114_2.nc` + `_header.txt` | [PANGAEA 966645](https://doi.pangaea.de/10.1594/PANGAEA.966645) (republished from AERONET MAN) | CC-BY-4.0 | `partiel` (synthetique seul) -> `partiel` (NetCDF MAN reel teste, AOT type, fallback fixture apres tentative generique); legacy `.TXT` et MAN generique sans fallback toujours absents |
| NetCDF NIRS-adjacent | `samples/netcdf/pyrnet_to_l1a_output.nc` | [`hdeneke/PyrNet`](https://github.com/hdeneke/PyrNet) | academic share | refusal non-NIRS teste: pas d'axe spectral ni de canaux Microtops AOT |
| Spectral Evolution / PSR | `samples/spectral_evolution/serbinsh_cvars_grape_leaf.sed` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | reste `partiel`, PSR-3500 firmware variant ajoute |
| SVC / GER SIG | `samples/svc_ger/serbinsh_gr070214_003.sig`, `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra) | GPL-3 | GER 3700 PDA + HR-1024i Barrow firmware variants ajoutees |

#### Fixtures non-redistribuables (uniquement en local — `samples_local/`, gitignore)

| Format | Fichier | Source | Licence / raison non-commit | Effet |
|---|---|---|---|---|
| Allotrope ADF adfsee | `samples_local/allotrope_adf/adfsee_example.adf` | [`allotrope-open-source/adfsee`](https://gitlab.com/allotrope-open-source/adfsee) | ADF/ontology terms Allotrope, garder local | reader ADF experimental teste: 4 records depuis 3 data-cubes numeriques; RDF minimal mappe titres, axe temps type `time`, scale secondaire nm et absorbance mAU |
| ARM MFRSR / ARM NetCDF adjacents | `samples_local/mfr/*.nc`, `samples_local/netcdf/*.nc` | DOE ARM / ARM test data | ARM Data Use Policy -> en local seulement | MFRSR b1 local decode en 4,320 observations x 7 filtres avec sidecar QC YAML; SURFSPECALB local decode en 986 lignes utiles x 6 filtres; AOSMET reste non spectral |
| BUCHI NIRCal cannabis | `samples_local/buchi_nircal/transpec_DEMO_cannabis.nir` | orellano-c/transpec_info | licence non clarifiee pour redistribution du fixture -> en local seulement | reader local teste: 105 spectres, axe 1501 wavenumbers et cibles non nulles `CBDA`/`THCA` |
| Hyperspectral cube (AVIRIS Indian Pines) | `samples_local/hyperspectral_cubes/indian_pines_corrected.mat` + `_gt.mat` | [EHU/Grupo de Inteligencia Computacional](http://www.ehu.eus/ccwintco/index.php/Hyperspectral_Remote_Sensing_Scenes) | "academic use" sans SPDX clair → en local seulement | reader MAT v5 local-only teste: 21,025 spectres x 200 bandes + cible `land_cover_class`; la version `92AV3C.lan` plus petite reste committee |
| Microtops `.lev2` disambiguation | `samples_local/microtops/noaa_lauder_sonde_la20170315.lev2` | [NOAA GML Lauder](https://gml.noaa.gov/aftp/data/ozwv/WaterVapor/Lauder_LEV/) | US Gov public domain MAIS le fichier est en realite un radiosonde water vapour/ozone, pas un sun-photometer Microtops | aide locale a la disambiguation `.lev2`; non commit pour eviter confusion |
| Microtops MAN ASCII Okeanos | `samples_local/microtops/aeronet_man_Okeanos_19_2_*.lev10/.lev15/.lev20` | AERONET Maritime Aerosol Network | AERONET MAN PI/coauthorship policy -> en local seulement | reader local teste: AOD valides types `aerosol_optical_thickness`, canaux `-999` omis, AOD-STD pour exports daily/series |
| PP Systems Arctic LTER indices | `samples_local/pp_systems/*.csv/.xlsx` | Arctic LTER / EDI | dataset local non committe | produit derive NDVI/EVI/PRI/etc.; ne ferme pas le manque de raw `.SPT/.SPU` |
| Thermo Nicolet OMNIC SRS locaux | `samples_local/nicolet_omnic/spectrochempy_TGA_demo.srs`, `spectrochempy_rapid_scan.srs`, `spectrochempy_rapid_scan_reprocessed.srs` | [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) | CeCILL-B mais fichiers volumineux -> local seulement | TGA_demo absorbance, rapid-scan brut interferogramme/index et rapid-scan reprocessé absorbance sont testes localement; `.srsx` reste absent |

### Formats restant fermes (sweep sans resultat exploitable, apres 3 passages)

| Format | Pourquoi pas trouve |
|---|---|
| Allotrope ADF vendeur | Le sample `adfsee` local ferme le manque "aucun ADF"; restent les ADF instrumentaux vendeurs (Waters/Sciex/Agilent/etc.), l'ontologie complete, les unites et la validation SDK Allotrope. |
| ASD calibration `.ILL/.REF/.RAW` | Distribution vendeur SDK uniquement; SPECCHIO partiel derriere login partenariat; aucun GitHub/Wayback/Mendeley sample. |
| Bruker OPUS 5/6 legacy | Archives privees, pas de mirror public; OPUS 7/8 couvert via 4 lecteurs independants suffit. |
| Foss `.NIR/.DA/.cal/.eqa` natif | Format ferme, aucune fixture binaire publique trouvee (Wayback FOSS / NIR-Predictor demos checked). |
| Horiba `.l6s` single-spectrum | Aucune fixture publique trouvee; seul `.l6m` (map) committe. |
| JASCO V-780 NIR / NRS Raman `.jws` variants | Aucun sample distinct du V-770 IR + V-series UV-Vis deja committes. |
| Metrohm Vision Air / OMNIS NIR natif | Format ferme, seul l'export CSV est documente publiquement. |
| MFR-7 / MFRSR `.OUT` reel | ARM Data Center exige compte; `samples_local/mfr/` ferme localement un NetCDF ARM MFRSR b1, mais pas un `.OUT` MFR-7 redistribuable — non commit. |
| Microtops II `.TXT` reel | AERONET MAN demande co-authorship; `samples_local/microtops/` ferme localement les exports MAN ASCII `.lev*`, mais pas un `.TXT` legacy redistribuable — non commit. |
| MODTRAN albedo `.dat` reel | Distribution sous licence MODTRAN/ONTAR ($2400) ; MIT OCW pcmodwin/RIT tutorials ne shippent que des references USGS deja couvertes. |
| NEON AOP HDF5 reflectance tile | Tiles 1 km × 1 km demandent inscription neon.science (compte gratuit mais distribution conditionnelle); fichier minimum ~50 MB. |
| Perten DA / Inframatic | Pas de fixture native ni CSV reel public (clients only). |
| PP Systems UniSpec `.SPT/.SPU` reel raw | Aucune fixture raw `.spu/.spt` publique; `samples_local/pp_systems/` contient seulement des indices derives Arctic LTER — non commit. |
| Shimadzu UVProbe `.spc` natif | Un seul candidat (`uri-t/shimadzu-spc-converter`) sans licence claire; aucune autre source apres sweep. |
| Si-Ware NeoSpectra Scanner natif single-measurement | Le pipeline OSSL ne publie que des matrices wide; pas de fixture "1 mesure par CSV" publique. |
| Specim IQ demo cube | Specim a discontinue le produit (page "end-of-life"); seul l'archive 7z Arabidopsis Zenodo 1345007 (123 MB) existe — trop gros, et le mix raw/processed n'est pas isole. |
| Thermo OMNIC `.srsx` | Pas de fixture publique trouvee (S.T.Japan demo bibliotheques `.spg` derriere formulaire); le canal `.srs`, y compris rapid-scan local, est couvert experimentalement. |
| VIAVI MicroNIR `.pri` natif | Format projet binaire, customer-only; CSV/XLSX exports reels deja couverts via UvA forensic. |
