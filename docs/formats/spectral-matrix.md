# Spectral Matrix Exports

Status: experimental.

This reader covers text exports where each row is one complete spectrum and
the spectral axis is either given by numeric column headers or by a preceding
`Wavelengths:` block.

It complements:

- `csv_like`, which handles simple files whose first line is already the
  numeric-header table;
- `spectral_table`, which handles transposed exports with one spectral point
  per row.

## Supported Fixtures

| Fixture | Records | Axis | Targets |
|---|---:|---|---|
| `samples/foss_winisi/synthetic_winisi_export.txt` | 50 | `Wavelengths:` block, `nm` | `protein` |
| `samples/foss_winisi/foss_xds_barleyground_sensAIfood.csv` | 7 | numeric `,` headers, `nm` | `Moisture`, `Protein`, `Year` |
| `samples/foss_winisi/foss_xds_wheat2_sensAIfood.csv` | 2 | numeric `,` headers, `nm` | `Moisture`, `Protein`, `Year` |
| `samples/metrohm/synthetic_visionair.csv` | 50 | numeric `;` headers, `nm` | `protein`, `moisture`, `fat` |
| `samples/viavi_micronir/synthetic_micronir.csv` | 20 | numeric `,` headers after instrument metadata, `nm` | none |
| `samples/csv_tsv/auroranir_handheld_barley_sensAIfood.csv` | 86 | numeric `,` headers, `nm` | `Moisture`, `Protein`, `Year` |
| `samples/siware_neospectra/neospectra_ossl_50samples_slice.csv` | 24 | numeric `,` headers, `nm` | soil reference properties |

The reader emits one `SpectralRecord` per sample row with a single
`absorbance` signal. Non-spectral numeric columns are stored as `targets`.
Sample identifiers are stored as `metadata.sample_id`.

## Dispatch Boundaries

Target-only reports are intentionally not loaded as spectra. The committed
FOSS DS3 and Perten report fixtures contain properties but no spectral axis,
so the registry returns `UnsupportedFormat` until the core model has a
non-spectral report representation.
