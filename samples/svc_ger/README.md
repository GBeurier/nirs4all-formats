# SVC HR-1024(i) / GER 3700 `.sig`

ASCII with two header conventions (PDA portable vs. laptop firmware). Header section ends at `data=`; the data block is whitespace-separated wavelength + reference + target + reflectance.

**Note**: GER 3700 and SVC share the `.sig` extension and a similar layout, but firmware-specific metadata fields differ. Some bad-file examples are intentionally broken to test error handling.

## Samples

All from [`meireles/spectrolab`](https://github.com/meireles/spectrolab/tree/master/inst/extdata) (GPL-3, R package).

### Acer leaf example (`Acer_example/`)

Sugar maple leaf spectra collected with an SVC HR-1024(i).

| File | Notes |
|---|---|
| `ACPL_D2_P1_T_1_000.sig` | Top of leaf, scan 1 |
| `ACPL_D2_P1_T_1_WR_000.sig` | White reference paired with the above |
| `ACPL_D2_P1_T_2_000.sig` | Top of leaf, scan 2 |
| `ACPL_D2_P1_B_1_001.sig` | Bottom of leaf, scan 1 |
| `ACPL_D2_P1_B_2_001.sig` | Bottom of leaf, scan 2 |
| `ACPL_D2_P1_M_1_000.sig` | Middle of leaf, scan 1 |
| `ACPL_D2_P1_M_2_000.sig` | Middle of leaf, scan 2 |
| `ACPL_F3_P2_B_1_000.sig` | Different plot, bottom scan |
| `ACPL_D2_P1_B_1_000_BAD.sig` | **Deliberately malformed** — for negative tests |
| `3_6_PANVI_2_T_1_001_BAD.sig` | **Deliberately malformed** — different SVC variant |

### Serbin BNL example (`svc_raw_and_overlap_matched_serbin/`)

| File | Variant |
|---|---|
| `BNL13001_000_laptop.sig` | Laptop firmware variant |
| `BNL13002_000_laptop.sig` | Laptop firmware variant |
| `BNL13001_000_moc.sig` | "moc" (matched overlap corrected) export |

### From [`serbinsh/R-FieldSpectra`](https://github.com/serbinsh/R-FieldSpectra/tree/master/inst/extdata) (GPL-3, R package)

| File | Size | Variant / notes |
|---|---|---|
| `serbinsh_gr070214_003.sig` | 33 KB | Raw GER 3700 PDA-firmware acquisition (filename pattern matches the GER 1500/3700 `<base>_<NNN>.sig` convention). Closes the prior "no raw GER 3700 PDA sample" gap. |
| `serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` | 72 KB | SVC HR-1024i field acquisition (BEO/Cake-Eater phenology campaign, Barrow Environmental Observatory) — `_resamp` indicates spectrolab-style resampling has already been applied. Complements `BNL13001_000_*` for an HR-1024(i) firmware comparison. |

## Parser hints

- ASCII, CRLF.
- Detect convention by inspecting header keys: PDA writes a different subset of metadata than laptop firmware.
- The two `*_BAD.sig` fixtures are accepted as parseable text but are marked
  with `declared_bad_fixture` / `svc_sig_declared_bad_fixture` so validation
  reports can separate them from clean acquisitions.
- Reference readers: [`spectrolab`](https://github.com/meireles/spectrolab), [`specdal`](https://github.com/EnSpec/SpecDAL).
