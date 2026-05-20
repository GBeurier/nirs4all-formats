# Metrohm NIRS XDS / DS2500 / Vision / Vision Air

Native [Vision project DB](https://www.metrohm.com/cs_cz/service/software-center/vision.html) has no open reader. CSV/Excel exports are the practical path for v1.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_visionair.csv` | Generated locally | CC-0 | Mock Vision Air export with `;` separator, sample/protein/moisture/fat columns + spectral block. Matches the layout reported by users. |

## Parser hints

- Vision Air typically uses `;` as field separator (Excel-European convention).
- Header rows: a few title lines, then a column-name row, then sample rows.
- Reference / property values (protein, moisture, fat, …) live in the first few columns; expose them in `targets`, not metadata.
- For native Vision DB files: refuse to load and recommend the vendor's Vision Air "Export to CSV" workflow.
