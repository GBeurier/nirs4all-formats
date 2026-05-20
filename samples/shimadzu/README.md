# Shimadzu UVProbe `.spc` / `.txt`

`.spc` is Shimadzu's proprietary container — **same extension as Galactic SPC, different binary format**. Must be disambiguated by header magic. v1 strategy: rely on the CSV/TXT export workflow.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_uvprobe.txt` | Generated locally | CC-0 | Mock UVProbe `.txt` export — `"Spectrum Data"` header + 2-column (wavelength, sample) CSV. |

## Parser hints

- Native Shimadzu `.spc`: only experimental readers exist ([`pyfasma-spc`](https://pypi.org/project/pyfasma-spc/) is one of the rare candidates). Refuse with a clear error and recommend the UVProbe export.
- `.txt` export has a fixed header (`"Spectrum Data"` followed by `"Wavelength nm","Sample <id>"` column row), then 2-column data.
- Quote handling: UVProbe wraps column names in `"…"` even when not needed by RFC 4180. A tolerant CSV reader is required.
