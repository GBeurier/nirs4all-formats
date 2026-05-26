# Outputs utilisateur

Ce document décrit les sorties que `nirs4all-io` doit fournir aux utilisateurs
finaux. La bibliothèque lit des formats spectroscopiques hétérogènes; elle doit
donc exposer à la fois un modèle riche et des projections simples pour les
outils data science.

## Principes

- Le modèle Rust reste la source de vérité.
- Les bindings Python, R, WASM et futurs bindings ne réimplémentent pas les
  parseurs; ils exposent les mêmes données dans des formes idiomatiques.
- Les sorties canoniques doivent conserver les signaux, axes, dimensions,
  métadonnées, provenance et avertissements.
- Les projections tabulaires ou ML peuvent être pratiques mais sont
  potentiellement destructives; elles doivent être explicites.
- Un format contenant plusieurs signaux, blocs, pixels, séries temporelles ou
  versions traitées ne doit pas être réduit implicitement à une seule matrice.

## Niveau 1: modèle canonique non destructif

Ces sorties sont prioritaires car elles préservent l'information extraite du
fichier source.

| Output | Interfaces | Contenu attendu | Priorité |
|---|---|---|---|
| `SpectralRecordSet` | Python, R, WASM, futur C ABI | Ensemble de records homogènes ou hétérogènes issus d'un fichier ou lot. | P0 |
| `Vec<SpectralRecord>` | Rust | Représentation native déjà utilisée par le coeur. | P0 |
| `SpectralRecord` | Toutes | Un échantillon, pixel, acquisition, bloc ou unité logique. | P0 |
| `SpectralArray` | Toutes | Un signal nommé avec `values`, `axis`, `shape`, `dims`, `coords`, `unit`, `role` et `source`. | P0 |
| `metadata` | Toutes | Métadonnées instrument, acquisition, sample, vendor, sidecars et champs métier. | P0 |
| `provenance` | Toutes | Fichier source, format détecté, reader, hash, version, warnings et limites connues. | P0 |
| `quality_flags` | Toutes | Indications explicites sur conversion, données manquantes, axes suspects ou support partiel. | P1 |

Contrat attendu: cette couche ne resample pas, ne fusionne pas les signaux et ne
choisit pas un signal à la place de l'utilisateur. Elle doit pouvoir représenter
un spectre 1-D, un cube `[row, col, x]`, une série `[time, x]`, ou plusieurs
signaux tels que `raw_counts`, `white_reference`, `dark_reference`,
`absorbance` et `reflectance`.

## Niveau 2: sorties data science simples

Ces sorties servent aux utilisateurs qui veulent rapidement charger leurs
données dans un notebook, un script R ou un pipeline ML.

| Output | Interfaces | Forme | Usage |
|---|---|---|---|
| `X, axis` | Python, R, Rust | Matrice `n_samples x n_features` + axe spectral. | Prétraitement, régression, classification. |
| `wide DataFrame` | pandas, polars, R `data.frame`/tibble | Une ligne par sample/pixel; colonnes metadata + colonnes spectrales. | Usage métier, Excel-like, ML classique. |
| `long DataFrame` | pandas, R tidyverse | Colonnes `record_id`, `signal`, `x`, `value`, metadata. | Multi-signaux, visualisation, axes hétérogènes. |
| `targets` | Python, R, Rust | Table séparée ou colonnes dédiées. | Valeurs labo, classes, propriétés chimiques. |
| `sklearn.Bunch` | Python | `data`, `target`, `feature_names`, `metadata`. | Intégration scikit-learn. |
| `torch.TensorDataset` | Python | Tenseurs `float32` pour `X` et cible optionnelle. | Deep learning. |
| `SpectroDataset` | Python / nirs4all | Dataset compatible avec l'écosystème `nirs4all`. | Modélisation downstream. |

Règle importante: si les records n'ont pas le même axe spectral, la projection
wide ou `X` doit échouer avec un diagnostic clair. Le resampling doit rester une
étape explicite dans `nirs4all` ou dans le code utilisateur.

## Niveau 3: sorties pour données complexes

Certains formats du projet ne sont pas naturellement tabulaires. Les sorties
suivantes doivent éviter de perdre la structure.

| Output | Cas concernés | Forme recommandée |
|---|---|---|
| `xarray.DataArray` | Cubes hyperspectraux, maps Raman, séries temps-spectre. | Dims et coords nommées. |
| `xarray.Dataset` | Plusieurs signaux alignés ou partiellement alignés. | Variables par signal. |
| `ND array + dims + coords` | Rust, WASM, C ABI, bindings sans xarray. | Buffer typé + shape + coordonnées. |
| `pixel table` | ENVI, ERDAS, Specim, AVIRIS, maps. | `row`, `col`, metadata spatiale, signal choisi. |
| `multi-signal dataset` | Raw/dark/white/processed dans un même fichier. | Un signal par source, rôles explicites. |
| `record inventory` | JCAMP `LINK`, OPUS, SPC multi-subfile, projets HDF5. | Liste des blocs/signaux disponibles avant projection. |

Une interface utilisateur devrait permettre de choisir explicitement le signal:
par exemple `signal="absorbance"`, `signal="reflectance"` ou
`signal="raw_counts"`.

## Exports fichiers

La CLI et les bindings doivent pouvoir écrire des formats de sortie stables pour
les workflows hors code.

| Export | Priorité | Contenu | Usage |
|---|---|---|---|
| JSON lossless | P0 | `SpectralRecord[]` complet, metadata, provenance, warnings. | Transport stable CLI/bindings/tests. |
| CSV wide | P0 | Table simple avec metadata et colonnes spectrales. | Excel, outils métier, import rapide. |
| CSV long | P1 | Une ligne par point spectral et par signal. | Multi-signaux, axes non alignés, visualisation. |
| Parquet | P1 | Wide ou long avec schéma stable. | Gros volumes, data lake, Python/R/Polars. |
| Arrow IPC | P2 | Table en mémoire sérialisée. | Échange rapide Python/R/JS. |
| HDF5 ou Zarr | P2 | Données N-D, chunks, coords, metadata. | Cubes et séries volumineuses. |
| PNG quicklook | P3 | Courbes superposées, heatmap ou aperçu cube. | Contrôle qualité rapide. |
| Diagnostics JSON | P1 | Dimensions, axes, signaux, warnings, NaN, saturation, hashes. | Audit automatique et support utilisateur. |

## Bundle recommandé

Pour un export complet depuis la CLI, utiliser un dossier structuré:

```text
dataset.n4io/
  manifest.json
  records.json
  spectra_wide.csv
  spectra_long.csv
  spectra.parquet
  metadata.json
  targets.csv
  diagnostics.json
  quicklook.png
```

`manifest.json` doit décrire la version de `nirs4all-io`, le fichier source, le
format détecté, les commandes utilisées, le signal exporté, les options de
sélection de pixels/ROI et les checksums.

## Surfaces par interface

| Interface | Outputs à fournir |
|---|---|
| Rust | `open_path() -> Vec<SpectralRecord>`, `RecordSet`, projections contrôlées, exports JSON/CSV/Parquet. |
| Python | Dict brut, dataclasses `SpectralRecordSet`, `numpy`, `pandas`, `polars`, `xarray`, `sklearn`, `torch`, `SpectroDataset`. |
| R | List brut, `nirs4allio_dataset`, `matrix`, `data.frame`, tibble optionnel, extraction des targets. |
| WASM / JS | JSON lossless, typed arrays, metadata/provenance séparables. |
| CLI | `probe`, `read-json`, `convert`, `scan --json`, exports bundle. |

## Priorités produit

1. Stabiliser le trio P0: JSON lossless, wide DataFrame/CSV et `X, axis`.
2. Fournir un diagnostic clair quand une projection est impossible ou lossy.
3. Ajouter Parquet pour les volumes importants et les pipelines modernes.
4. Exposer les structures N-D via `xarray`/coords sans les aplatir par défaut.
5. Ajouter un export bundle pour faciliter les échanges avec des utilisateurs
   non développeurs.

