# Thermo / Galactic GRAMS `.spc`

De-facto interchange format. Multiple binary variants — **old vs. new header**, **LSB vs. MSB byte order**, and several data layouts (`-XY`, `-XYY`, `-XYXY`) for single-spectrum, common-X multi-spectrum, or independent-X multi-spectrum files. Test fixtures should cover every combination.

**Extension collision warning**: `.spc` is also used by OceanView (Ocean Optics), Shimadzu UVProbe, and Renishaw — all unrelated binary formats. Dispatch must use magic-byte sniffing, not the extension.

## Samples

### From [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) — MIT licensed

| File | Size | Variant | Domain |
|---|---|---|---|
| `nir.spc` | 57 KB | new LSB, `multi_common_generated_x` | NIR |
| `Ft-ir.spc` | 8 KB | new LSB, `single_generated_x` | FT-IR |
| `RAMAN.SPC` | 15 KB | new LSB, `single_generated_x`, custom labels | Raman |
| `raman-sion.spc` | ? | new LSB, `multi_common_generated_x` | Raman |
| `RUBY18.SPC` | 4 KB | new LSB, `single_generated_x`, custom labels | UV-Vis |
| `MERC.SPC` | 14 KB | new LSB, `single_generated_x`, custom labels | Hg lamp |
| `m_xyxy.spc` | 49 KB | new LSB, **-XYXY** `multi_independent_xyxy` | (test) |
| `m_evenz.spc` | 24 KB | new LSB, `multi_common_generated_x`, common Z | (test) |
| `m_ordz.spc` | 35 KB | old LSB, `multi_common_generated_x`, ordered Z | (test) |
| `s_evenx.spc` | 8 KB | new LSB, `single_generated_x` | (test) |
| `s_xy.spc` | 5 KB | new LSB, **-XY** `single_explicit_x` | (test) |
| `test_input.spc` | 17 KB | new LSB, `single_generated_x` | (test) |
| `NMR_FID.SPC` | 129 KB | new LSB, `single_generated_x`, NMR free induction decay | NMR |
| `resolutionPro.spc` | ? | new LSB, `single_generated_x`, ResolutionPro variant | (test) |
| `NDR0002.SPC` | ? | new LSB, `single_generated_x`, custom labels | (test) |

`NMR_FID.SPC` is kept as a collision/adjacent-domain fixture but is not part of
the NIRS golden-conformance set.

### From [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) — CeCILL-B

| File | Size | Notes |
|---|---|---|
| `BENZENE.SPC` | 8 KB | new LSB, `single_generated_x`; benzene IR (real chemistry reference) |
| `DRUG_SAMPLE.SPC` | 90 KB | new LSB, `multi_independent_xyxy`; directory-backed mass-spectrum series |

### Spec reference

`spc_format_spec.pdf` — Galactic SPC v4 format spec PDF (from cheminfo/spc-parser/docs). Authoritative reference for header layout, flags, and block structure.

## Parser hints

- Magic: first byte is the `FTFLGS` byte, second byte is the `FVERSN` byte. New header `FVERSN = 0x4B`, old header `FVERSN = 0x4D` or `0x4E`. Choose layout based on `FVERSN`.
- LSB vs MSB byte order: determined by `FVERSN` value. The format spec PDF in this directory documents both.
- Data layouts:
  - `-XY`: single spectrum, explicit (x, y) pairs
  - `-XYY`: multi-spectrum sharing one X axis, then multiple Y arrays
  - `-XYXY`: multi-spectrum with independent X axes per spectrum (sub-files)
- The reader exposes the decoded layout as `metadata.galactic_spc.data_layout`
  (`single_generated_x`, `single_explicit_x`, `multi_common_generated_x`,
  `multi_common_explicit_x`, or `multi_independent_xyxy`).
- Reference readers:
  - Python: [`spc-spectra`](https://github.com/nick-macro/spc-spectra), [`rohanisaac/spc`](https://github.com/rohanisaac/spc), `specio`, `spectrochempy`
  - JS: [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) (most actively maintained)
  - C++: `xylib`
