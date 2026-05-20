# Thermo / Galactic GRAMS `.spc`

De-facto interchange format. Multiple binary variants — **old vs. new header**, **LSB vs. MSB byte order**, and several data layouts (`-XY`, `-XYY`, `-XYXY`) for single-spectrum, common-X multi-spectrum, or independent-X multi-spectrum files. Test fixtures should cover every combination.

**Extension collision warning**: `.spc` is also used by OceanView (Ocean Optics), Shimadzu UVProbe, and Renishaw — all unrelated binary formats. Dispatch must use magic-byte sniffing, not the extension.

## Samples

### From [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) — MIT licensed

| File | Size | Variant | Domain |
|---|---|---|---|
| `nir.spc` | 57 KB | (sniff) | NIR |
| `Ft-ir.spc` | 8 KB | (sniff) | FT-IR |
| `RAMAN.SPC` | 15 KB | (sniff) | Raman |
| `raman-sion.spc` | ? | (sniff) | Raman |
| `RUBY18.SPC` | 4 KB | new header, single spectrum | UV-Vis |
| `MERC.SPC` | 14 KB | (sniff) | Hg lamp |
| `m_xyxy.spc` | 49 KB | **-XYXY** multi-spectrum, independent X axes | (test) |
| `m_evenz.spc` | 24 KB | multi-spectrum, common Z | (test) |
| `m_ordz.spc` | 35 KB | multi-spectrum, ordered Z | (test) |
| `s_evenx.spc` | 8 KB | single spectrum, even X (no X array stored) | (test) |
| `s_xy.spc` | 5 KB | single spectrum, **-XY** layout | (test) |
| `test_input.spc` | 17 KB | (test) | (test) |
| `NMR_FID.SPC` | 129 KB | NMR free induction decay | NMR |
| `resolutionPro.spc` | ? | ResolutionPro variant | (test) |
| `NDR0002.SPC` | ? | (test) | (test) |

`NMR_FID.SPC` is kept as a collision/adjacent-domain fixture but is not part of
the NIRS golden-conformance set.

### From [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) — CeCILL-B

| File | Size | Notes |
|---|---|---|
| `BENZENE.SPC` | 8 KB | Benzene IR (real chemistry reference) |
| `DRUG_SAMPLE.SPC` | 90 KB | Drug NIR sample (realistic pharma use case) |

### Spec reference

`spc_format_spec.pdf` — Galactic SPC v4 format spec PDF (from cheminfo/spc-parser/docs). Authoritative reference for header layout, flags, and block structure.

## Parser hints

- Magic: first byte is the `FTFLGS` byte, second byte is the `FVERSN` byte. New header `FVERSN = 0x4B`, old header `FVERSN = 0x4D` or `0x4E`. Choose layout based on `FVERSN`.
- LSB vs MSB byte order: determined by `FVERSN` value. The format spec PDF in this directory documents both.
- Data layouts:
  - `-XY`: single spectrum, explicit (x, y) pairs
  - `-XYY`: multi-spectrum sharing one X axis, then multiple Y arrays
  - `-XYXY`: multi-spectrum with independent X axes per spectrum (sub-files)
- Reference readers:
  - Python: [`spc-spectra`](https://github.com/nick-macro/spc-spectra), [`rohanisaac/spc`](https://github.com/rohanisaac/spc), `specio`, `spectrochempy`
  - JS: [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) (most actively maintained)
  - C++: `xylib`
