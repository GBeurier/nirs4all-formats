# Spectral Evolution / PSR `.sed`

Best-documented field-spectrometer ASCII format. Header carries instrument/date/GPS; the data block is whitespace-separated columns (wavelength, reference, target, reflectance %).

## Samples

### From [`meireles/spectrolab`](https://github.com/meireles/spectrolab/tree/master/inst/extdata/psr_DN_brett) (GPL-3, R package)

The `psr_DN_brett` directory hosts Brett's PSR DN test pair — one workable and one deliberately broken to exercise error paths.

| File | Size | Notes |
|---|---|---|
| `1566060_09506_working.sed` | 95 KB | Working PSR (Spectral Evolution) spectrum. |
| `1566060_15025_not_working.sed` | 76 KB | Companion broken-but-valid DN-only file; parsed with `missing_reflectance_signal` quality flag. |

### From [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra/tree/master/inst/extdata) (GPL-3, R package)

| File | Size | Notes |
|---|---|---|
| `serbinsh_cvars_grape_leaf.sed` | 95 KB | Real Spectral Evolution **PSR-3500** grape-leaf reflectance acquisition (Brookhaven National Lab). Different firmware variant than the `psr_DN_brett` pair — exercises slight header drift between PSR firmwares. |

## Parser hints

- ASCII with CRLF line terminators.
- Header section is `key: value` lines until `Data:` (or similar marker).
- Multi-column data block: `Wvl, Rad. (Ref.), Rad. (Target), Reflect.`
- Reference readers:
  - R: [`spectrolab::read_spectra(format="sed")`](https://github.com/meireles/spectrolab)
  - Python: [`specdal`](https://github.com/EnSpec/SpecDAL)
