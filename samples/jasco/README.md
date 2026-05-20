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

- `.jws` is OLE2 — open with `olefile` (Python) or `cfb` (Rust).
- Streams of interest in committed fixtures: `DataInfo`, `Y-Data`, `BaseInfo`,
  `ModuleInfo`, `SampleInfo`, `UserInfo` and `MeasParam`.
- Current native labels:
  - `243.jws`: `transmittance` (`%T`) from FT/IR metadata and ordinate scale.
  - `sample_fluorescence.jws`: `fluorescence` from `FP-8300` metadata.
  - `sample_CD_HT_Abs.jws`: `cd` (`mdeg`), `ht` (`V`) and `absorbance`
    (`dOD`) from `CD-1500` / `J-1500` metadata.
- The three committed `.jws` fixtures are covered by semantic tests, probes and
  golden summaries.
- Other JASCO variants may use streams such as `Data`, `Header`, `XdataValue`,
  etc. (varies by JASCO firmware).
- Reference readers:
  - Python: [`jws2txt`](https://pypi.org/project/jws2txt/), [`odoluca/jasco_jws_reader`](https://github.com/odoluca/jasco_jws_reader). Coverage is partial.
- ASCII text export is the safe fallback for any JASCO instrument.
