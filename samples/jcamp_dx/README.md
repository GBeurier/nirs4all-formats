# JCAMP-DX (`.jdx`, `.dx`, `.jcm`, `.jcamp`)

[IUPAC standard](https://iupac.org/what-we-do/digital-standards/jcamp-dx/). The payload can use four different encodings — **AFFN**, **XYDATA**, **DIF/DUP**, or **NTUPLES** — and each existing reader covers a subset. Test fixtures here exercise every encoding so the parser can be regression-tested.

## Official JCAMP-DX.org test suite

These files are the **canonical IUPAC test fixtures** from [jcamp-dx.org](http://www.jcamp-dx.org/) mirrored in [`nzhagen/jcamp/data/jcamp-dx.org_official_test_data/`](https://github.com/nzhagen/jcamp/tree/master/data/jcamp-dx.org_official_test_data). MIT licensed.

| File | Encoding | Purpose |
|---|---|---|
| `BRUKAFFN.DX` | **AFFN** | Bruker IR — plain ASCII numbers, no compression |
| `BRUKDIF.DX` | **DIF** (difference) | Bruker IR — DIF compression |
| `BRUKPAC.DX` | **PAC** (packed) | Bruker IR — PAC compression |
| `BRUKSQZ.DX` | **SQZ** (squeezed) | Bruker IR — SQZ digit compression |
| `BRUKNTUP.DX` | **NTUPLES** | Bruker NMR/2D — NTUPLES block |
| `BRUKER1.JCM` | (Bruker .JCM variant) | Bruker proprietary JCAMP subset |
| `PE1800.DX` | PE/AFFN | Perkin Elmer 1800 IR — real-world AFFN |
| `LABCALC.DX` | LabCalc | Galactic LabCalc-style |
| `SPECFILE.DX` | XYDATA | Small generic test |
| `TEST32.DX` | various | 32-bit precision test |
| `TESTSPEC.DX` | XYDATA | Generic spectrum test |
| `TESTFID.DX` | NTUPLES | Free Induction Decay (NMR-style 2D) |

## NIST WebBook IR JCAMPs

NIST Chemistry WebBook IR spectra are distributed as JCAMP-DX. These are real-world chemistry references suitable for sanity checks. Public domain (U.S. Government work).

| File | Compound | CAS |
|---|---|---|
| `nist_water_ir.jdx` | Water | 7732-18-5 |
| `nist_ethanol_nist_ir.jdx` | Ethanol | 64-17-5 |
| `nist_methanol_ir.jdx` | Methanol | 67-56-1 |
| `nist_methane_ir.jdx` | Methane | 74-82-8 |
| `nist_glycerol_ir.jdx` | Glycerol | 56-81-5 |
| `nist_sucrose_ir.jdx` | Sucrose | 57-50-1 |

Fetch URL pattern: `https://webbook.nist.gov/cgi/cbook.cgi?JCAMP=C<cas_no_dashes>&Type=IR&Index=1`.

## IR small-molecule library (nzhagen/jcamp)

Single-compound IR spectra under [`nzhagen/jcamp/data/infrared_spectra/`](https://github.com/nzhagen/jcamp/tree/master/data/infrared_spectra). MIT.

| File | Compound |
|---|---|
| `ethanol_ir.jdx` | Ethanol |
| `acetone_ir.jdx` | Acetone |
| `carbon_dioxide_ir.jdx` | CO₂ |

## Parser hints

- Header tags begin with `##` (e.g. `##TITLE=`, `##JCAMP-DX=`, `##XUNITS=`, `##YUNITS=`, `##NPOINTS=`, `##FIRSTX=`, `##LASTX=`).
- Data block follows `##XYDATA=` or `##XYPOINTS=` or `##NTUPLES=` and is terminated by `##END=`.
- DIF/DUP/SQZ are digit-substitution encodings; AFFN is plain numbers. NTUPLES is for multi-dimensional data.
- Reference readers:
  - Python: [`jcamp`](https://github.com/nzhagen/jcamp) (covers AFFN/XYDATA/DIF/DUP), [`spectrochempy.read_jcamp()`](https://www.spectrochempy.fr/reference/generated/spectrochempy.read_jcamp.html), `nmrglue`
  - R: `ChemoSpec`, `hyperSpec`
- v1 priority — most format-friendly text format.
