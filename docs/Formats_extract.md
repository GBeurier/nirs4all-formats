# Export fichiers à récupérer

Généré le 2026-05-26 depuis `docs/FORMAT_MATRIX.md`, `docs/MISSING_SAMPLES.md` et les pages `docs/formats/`. Pour chaque lot, demander si possible le fichier original, un export lisible du même scan, le modèle instrument, la version logicielle, le mode de mesure et quelques valeurs de contrôle. Garder les métadonnées structurelles même si les données doivent être anonymisées.

## Fichiers originaux manquants à récupérer

| Priorité | Format / machine | Fichiers originaux à demander | Export compagnon utile | Pourquoi |
|---|---|---|---|---|
| P0 | Foss NIRSystems / WinISI / ISIscan | `.NIR`, `.DA`, `.cal`, `.eqa` natifs | CSV/TXT du même jeu + version WinISI/ISIscan | Format industriel clé, aucun binaire natif exploitable. |
| P0 | Perten DA / Inframatic | Fichier natif vendeur spectral, pas seulement rapport cible | CSV/XLSX avec colonnes longueurs d’onde | Format industriel clé, aucun sample spectral natif/export exploitable. |
| P1 | ASD FieldSpec calibration | Jeux complets `.asd` + `.ILL`, `.REF`, `.RAW` | ASCII avec white/dark/reference | Débloque les compagnons de calibration. |
| P1 | ASD FieldSpec legacy | `.asd` revisions v3/v4/v5 et fichiers avec blocs internes reference/calibration/audit/signature | Export ASCII si disponible | Les revisions récentes passent; les anciens firmwares restent à confirmer. |
| P1 | Avantes AvaSoft 8 | `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8`, jeu multi-subfile, `.IRR8` complet | Export AvaSoft lisible | Beaucoup de suffixes actifs sans fixture. |
| P1 | Avantes AvaSoft 6/7 | `.ABS` binaire legacy, autres modes binaires non ASCII | Export AvaSoft lisible | Trou principal du lecteur legacy. |
| P1 | BUCHI NIRCal / NIRMaster | `.nir` avec cibles non nulles, `.cal`, variants NIRMaster/NIRFlex | JCAMP-DX ou CSV du même projet | Reader utile mais fixture publique riche en cibles manquante. |
| P1 | HDF5 NIRS générique | `.h5`/`.hdf5` réels avec spectra/axes/metadata/targets, groupes imbriqués, matrices transposées, multi-signaux | Description schéma ou script d’export | Il faut des schémas réels metadata-rich. |
| P1 | JCAMP-DX spectral | `.jdx`, `.dx`, `.jcm`, `.jcamp` avec `LINK`, `PEAK TABLE`, `PEAK ASSIGNMENTS`, `NTUPLES` | Export vendeur si possible | Couverture de base OK; vrais cas multi-blocs/peaks à valider. |
| P1 | Metrohm Vision / Vision Air / OMNIS NIR | Base/projet natif si possible, exports Vision Air réels | CSV/XLSX avec axe spectral | CSV synthétique seulement; base native fermée. |
| P1 | PP Systems UniSpec SC | Acquisition brute `.SPT` terrain | Export texte éventuel | Parser validé seulement sur synthétique. |
| P1 | PP Systems UniSpec DC | Acquisition brute `.SPU` deux canaux terrain | Export texte éventuel | Parser deux canaux validé seulement sur synthétique. |
| P1 | Si-Ware NeoSpectra Scanner | Export single-measurement par scan, CSV/XLSX ou format app | Export cloud si disponible | Matrices réelles OK; format une mesure par fichier absent. |
| P1 | Spectral Evolution / PSR / SR | `.sed` SR-3500/SR-6500/firmwares récents, reflectance/radiance/DN | Export ou dump reference `spectrolab`/`specdal` | Variants SR et conformance à élargir. |
| P1 | Spectro Inc. SiWare API | Réponses API JSON réelles + CSV associé | Documentation champs API | Fixtures actuelles synthétiques. |
| P1 | SVC / GER SIG | `.sig` HR-1024i firmware >= 3.0, fichiers radiance calibrée, GER historiques | Exports `spectrolab` comparables | Améliore unités physiques et conformance byte-level. |
| P1 | VIAVI MicroNIR | Projet natif `.pri` | CSV/XLSX du même scan | Exports réels OK; natif customer-only. |
| P2 | Allotrope ADF vendeur | `.adf` instrumentaux Waters/Sciex/Agilent/Bruker ou autre | Export équivalent + unités/ontologie | ADF local partiel; manque validation SDK/vendor. |
| P2 | Allotrope ASM | JSON ASM issus de conversions vendeurs multiples | Export instrument source | Benchling couvert; diversité industrielle à valider. |
| P2 | AnIML | Vrais `.animl` spectraux, XSD/conformance, plusieurs `SeriesSet` | Export source si possible | Exemples spectraux actuels synthétiques ou non spectraux. |
| P2 | Bruker OPUS legacy | OPUS 5/6 `.0`, `.1`, `.001`, `.0000`, sans extension, blocs 2D/imaging | DPT/CSV du même scan | OPUS 7/8 et MPA OK; legacy/imaging restent à couvrir. |
| P2 | Bruker Tango / Matrix | OPUS natifs issus de Tango FT-NIR et Matrix | DPT/CSV du même scan | MPA couvert; manque fixtures dédiées Tango/Matrix. |
| P2 | ENVI / cubes hyperspectraux | Jeux `.hdr` + `.dat`/`.img` Specim, HySpex, Headwall, Specim IQ; NEON AOP HDF5 | Metadata capteur, ROI, calibration | ENVI/AVIRIS OK; familles HSI terrain à sourcer. |
| P2 | FGI HDF5 + XML | Paire réelle `.h5`/`.hdf5` + sidecar `.xml` | Schéma XML complet | Mapping actuel synthétique seulement. |
| P2 | Horiba LabSpec / JobinYvon | `.l6s` single-spectrum, autres `.l6m` LabSpec6 | Export texte/XML correspondant | `.l6m` map expérimental; single-spectrum absent. |
| P2 | JASCO JWS | `.jws` V-780/V-series NIR et NRS Raman, streams `Data`, `Header`, `XdataValue` | Export texte JASCO | Variants lab/NIR/Raman absents. |
| P2 | MATLAB MAT / RData spectraux | `.mat` v5/v7.3 et `.RData` réels avec structures hétérogènes, cubes, targets, metadata | Script de génération si possible | Structures arbitraires à élargir. |
| P2 | MFR-7 / MFRSR | `.OUT` réel redistribuable, NetCDF ARM avec calibration, `_FillValue`, filtres, QC | YAML/QC si disponible | NetCDF ARM local seulement; `.OUT` redistribuable absent. |
| P2 | Microtops II / MAN | `.TXT` legacy Microtops II, exports MAN ASCII/NetCDF redistribuables, header complet | Documentation AERONET/MAN | MAN local OK; `.TXT` public absent. |
| P2 | NetCDF NIRS générique | `.nc`/`.cdf` spectraux réels avec wavelengths, spectra, metadata, QC, multi-signaux | Notes schéma | Schémas dédiés OK; généricité à renforcer. |
| P2 | Ocean Optics / Ocean Insight | Exports QE Pro, Maya, Apex; vrai `.spc` Ocean non-Galactic | Export OceanView/SpectraSuite du même scan | Appareils récents sans fixture. |
| P2 | PerkinElmer Spectrum / Lambda / Spotlight | `.sp` NIR/Lambda, `.fsm` Spotlight imaging | CSV/TXT du même scan | `.sp` mono-spectre OK; imaging et variants NIR/Lambda à sourcer. |
| P2 | Renishaw WDF | `.wdf` InVia Qontor/Apollo, autres layouts `MAP`, maps/depth/time-series | CSV/ASCII équivalent | Couverture forte mais layouts/conformance incomplets. |
| P2 | Shimadzu UVProbe | Vrai `.spc` natif Shimadzu et vrai `.txt` UVProbe | Export comparé | `.txt` actuel synthétique; natif `.spc` manquant. |
| P2 | Specim IQ / cubes terrain | Archive Specim IQ réduite exploitable, raw/processed identifiés | Licence claire + metadata | Source possible mais trop grosse/non isolée actuellement. |
| P2 | Thermo / Galactic GRAMS SPC | `.spc` new big-endian, vieux headers/logs, multi-subfile atypiques | Export ou lecture reference | Variants LSB OK; BE/vieux logs manquants. |
| P2 | Thermo Nicolet OMNIC | `.srsx`, autres `.srs` high-speed/rapid-scan, variants `.spa/.spg` | Export ASCII | SPA/SPG/SRS utiles; `.srsx` absent. |
| P2 | WiTec WIP / WID | `.wip`, `.wid` de layouts variés | Export ASCII du même projet | Un layout map OK; layouts généraux à sourcer. |
| P3 | ENVI Spectral Library legacy | `.slb` accompagné de `.hdr` | Export ENVI si disponible | Ferme un variant legacy faible impact. |
| P3 | Excel legacy | `.xls` OLE spectral, vrai `.xlsm` avec macros, workbooks multi-feuilles réels | CSV du même workbook | Robustesse import, non bloquant. |
| P3 | MODTRAN albedo | Sortie `.dat` MODTRAN/ONTAR redistribuable | Licence claire | Hors cœur NIRS; sample réel absent. |
| P3 | USGS SPECPR | Binaire SPECPR original, dumps AREF avec axes vérifiables | Conversion ASCII | Textes USGS/ECOSTRESS OK; binaire absent. |

## Formats dont le contenu dépasse les seuls spectres

| Format | Ce que le fichier peut contenir en plus des spectres | À préserver / vérifier lors de la récupération | Pourquoi c’est important |
|---|---|---|---|
| ASD FieldSpec `.asd` + compagnons | Blocs internes secondary/dependent/reference/calibration, audit/signatures, calibration `.ILL/.REF/.RAW` | Fichier primaire + compagnons, timestamps dark/reference, version firmware, labels calibration | Certaines données ne sont pas encore émises comme signaux mais doivent être inventoriées. |
| Avantes AvaSoft 6/7/8 | Raw/sample/dark/reference/irradiance, calibration irradiance, multi-subfile, metadata instrument/operator | Tous les fichiers d’un même scan et suffixes par mode | Recomposer absorbance/transmittance/irradiance peut nécessiter les fichiers compagnons. |
| BUCHI NIRCal `.nir` | Propriétés/cibles, réplicats, `Spectra Info`, GUID projet/spectre, device/serial, timestamps | Projet complet avec cibles non nulles et réplicats | Un `.nir` est un transfert/calibration, pas seulement une matrice X. |
| Bruker OPUS | Plusieurs signaux dans un fichier: absorbance, reflectance, sample/reference, interferograms, phase, reports | Conserver tous les blocs et un export DPT/CSV comparable | Les blocs multiples peuvent représenter des versions ou traitements distincts du même scan. |
| JCAMP-DX | `LINK` multi-blocs, `NTUPLES`, tables de pics, assignments, FID/NMR, checkpoints X | Ne pas découper les fichiers; garder tous les blocs liés | Un seul fichier peut contenir plusieurs spectres ou des données sparse/peak plutôt qu’une courbe simple. |
| Thermo / Galactic SPC | Layouts single/common/independent-X, multi-subfile, NIR/FTIR/Raman/NMR/MS, anciens headers | Fichier brut original + indication instrument/domaine | L’extension `.spc` est collision-prone et le layout change le sens des données. |
| Thermo Nicolet OMNIC `.spa/.spg/.srs/.srsx` | Groupes de spectres, séries TGA/GC, rapid-scan, axes secondaires temps/Y, raw/reprocessed | Série complète + export ASCII si possible | Les `.srs` sont des matrices/séries, pas un spectre unique. |
| SVC / GER `.sig` | Reference/target/reflectance, overlap policy, factors, foreoptic, detector metadata, GPS, battery, errors | Fichier brut non resamplé + éventuel export resamplé | Le signal reflectance dépend souvent de la reference et de corrections de recouvrement. |
| Spectral Evolution `.sed` | DN reference/target, reflectance, GPS, instrument/foreoptic, batteries, integration times, dark mode | Fichier original avec unités explicites | Certains fichiers sont DN-only ou ont colonnes déclarées incohérentes. |
| HDF5 NIRS générique | Groupes imbriqués, multi-signaux, axes partagés, targets, attributs globaux, matrices transposées | Arborescence complète et description schéma | Les conventions HDF5 varient fortement selon laboratoire/instrument. |
| FGI HDF5 + XML | HDF5 payload + XML metadata sidecar | Toujours fournir la paire `.xml` + `.h5/.hdf5` | Le XML porte la metadata et référence le payload HDF5. |
| NetCDF / ARM / Microtops / MFRSR | Séries temporelles, QC arrays, sidecar YAML, global attributes, filtres/canaux multiples | NetCDF complet + fichiers QC/headers associés | Les flags qualité et axes viennent souvent de metadata ou sidecars. |
| ENVI Standard / ERDAS LAN / cubes HSI | Cube image, axis sidecars, ground-truth `.GIS`, ROI/masks, coordonnées spatiales | Tous les sidecars (`.hdr`, `.spc`, `.GIS`) + contexte spatial | Chaque pixel est un spectre; les labels/classes sont dans des fichiers séparés. |
| MATLAB MAT / RData | Structures hétérogènes, matrices X/y, targets, labels, cubes, sidecars `_gt.mat` | Workspace complet + script/export décrivant les variables | Les noms de variables donnent souvent le rôle spectre/axe/cible. |
| Renishaw WDF | Spectra, maps, line/depth/time-series, white-light image metadata, `MAP` analysis blocks | `.wdf` brut + export CSV/ASCII du même mapping | Les maps et analyses dérivées ne se réduisent pas à une courbe. |
| Horiba LabSpec `.l6m/.l6s` / XML/TXT | Maps, line scans, coordonnées spatiales, axe énergie/wavenumber/wavelength, metadata instrument | Binaire + export texte/XML appairé | La comparaison binaire/export est nécessaire pour stabiliser les layouts. |
| WiTec `.wip/.wid` | Projet complet: maps, line scans, images/navigation, objets TDGraph, coordonnées physiques | Projet brut + export ASCII du même projet | Le contenu est une arborescence projet, pas un fichier spectre plat. |
| JASCO `.jws` | Streams OLE2 `DataInfo`, `Y-Data`, `BaseInfo`, multi-channel CD/HT/Abs, fluorescence/IR/NIR/Raman | Fichier OLE complet + export texte | Les canaux peuvent avoir des rôles sémantiques distincts. |
| Ocean Optics / Ocean Insight | Textes avec metadata vendor, Jaz multichannel `W/I/P`, ProcSpec XML/ZIP, white-reference | Archive/fichier complet + mode acquisition | Les colonnes ne suffisent pas toujours à typer le signal. |
| Consumer Physics SCiO CSV | Groupes `spectrum`, `wr_raw`, `sample_raw`, metadata device/sample, targets | CSV complet avec preamble | Un export peut contenir signal traité et raw/reference. |
| Allotrope ADF | HDF5 data-cubes + RDF/triplestore, axes secondaires, unités ontologiques | `.adf` complet + mapping/SDK si disponible | L’ontologie détermine le sens des cubes et unités. |
| Allotrope ASM JSON | Data cubes, endpoint results, device/control settings, converter metadata | JSON complet et source instrument | Le JSON peut décrire spectra, endpoints et contexte expérimental. |
| AnIML | XML avec `SeriesSet`, axes, valeurs explicites ou auto-incrémentées, sample metadata | XML complet + XSD/version | Plusieurs séries peuvent cohabiter dans le même document. |
| DigitalSurf `.sur/.pro` | Profils multi-spectres, hyperspectral maps, surfaces, zlib compression, axes spatiaux | Fichier complet + type d’objet exporté | Certaines données sont surfaces/profils, pas spectres NIRS directs. |
| Princeton TriVista `.tvf` | Frames multiples, time-series, maps, multi-spectrometer, Step-and-Glue, hardware metadata | `.tvf` complet + notes acquisition | Un fichier peut contenir navigation, frames et plusieurs spectromètres. |
| Excel / XLSX / XLSM | Plusieurs feuilles, metadata/cibles, macros, axes convertis en dates, matrices wide | Workbook complet non converti | Une conversion CSV peut perdre feuilles, types et metadata. |
| NumPy `.npy/.npz` / Parquet | Matrices X, axes, sample IDs, targets, schema metadata | Archive complète avec tous les arrays/colonnes | Les rôles axe/cible/sample dépendent des clés ou colonnes. |
