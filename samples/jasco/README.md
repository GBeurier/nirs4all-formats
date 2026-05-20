# JASCO V-series / FT-IR `.jws`

OLE2 compound-document binary (Microsoft compound file format). Reverse-engineered. Mostly UV-Vis but also used in NIR mode.

## Samples

### From [`odoluca/jasco_jws_reader`](https://github.com/odoluca/jasco_jws_reader/tree/master/Sample%20JWS%20files) — GPL-3

| File | Mode | Size |
|---|---|---|
| `sample_fluorescence.jws` | Fluorescence | 7.5 KB |
| `sample_CD_HT_Abs.jws` | CD / HT / Abs (multi-channel) | 25 KB |

### From [`gnezd/Jasco_jws`](https://github.com/gnezd/Jasco_jws/tree/main/testdata)

| File | Notes |
|---|---|
| `243.jws` | Generic JASCO V-series IR spectrum, 36 KB. |

### Synthetic ASCII export

| File | Notes |
|---|---|
| `synthetic_jws_export.txt` | Mock JASCO V-770 NIR text export (synthetic — generated locally). Useful as a known-good reference for the ASCII export path. |

## Parser hints

- `.jws` is OLE2 — open with `olefile` (Python) or `compoundfiles` (Rust).
- Streams of interest: `Data`, `Header`, `XdataValue`, etc. (varies by JASCO firmware).
- Reference readers:
  - Python: [`jws2txt`](https://pypi.org/project/jws2txt/), [`odoluca/jasco_jws_reader`](https://github.com/odoluca/jasco_jws_reader). Coverage is partial.
- ASCII text export is the safe fallback for any JASCO instrument.
