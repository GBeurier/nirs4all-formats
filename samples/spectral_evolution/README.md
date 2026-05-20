# Spectral Evolution / PSR `.sed`

Best-documented field-spectrometer ASCII format. Header carries instrument/date/GPS; the data block is whitespace-separated columns (wavelength, reference, target, reflectance %).

## Samples

From [`meireles/spectrolab`](https://github.com/meireles/spectrolab/tree/master/inst/extdata/psr_DN_brett) (GPL-3, R package). The `psr_DN_brett` directory hosts Brett's PSR DN test pair — one workable and one deliberately broken to exercise error paths.

| File | Size | Notes |
|---|---|---|
| `1566060_09506_working.sed` | 95 KB | Working PSR (Spectral Evolution) spectrum. |
| `1566060_15025_not_working.sed` | 76 KB | Companion broken-but-valid file for negative-path tests. |

## Parser hints

- ASCII with CRLF line terminators.
- Header section is `key: value` lines until `Data:` (or similar marker).
- Multi-column data block: `Wvl, Rad. (Ref.), Rad. (Target), Reflect.`
- Reference readers:
  - R: [`spectrolab::read_spectra(format="sed")`](https://github.com/meireles/spectrolab)
  - Python: [`specdal`](https://github.com/EnSpec/SpecDAL)
