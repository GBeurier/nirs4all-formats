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

## Parser hints

- ASCII, CRLF.
- Detect convention by inspecting header keys: PDA writes a different subset of metadata than laptop firmware.
- Reference readers: [`spectrolab`](https://github.com/meireles/spectrolab), [`specdal`](https://github.com/EnSpec/SpecDAL).
