# Demo sample provenance & licenses

These are the "try a sample" fixtures bundled with the in-browser demo. Every
file is **permissively licensed** (CC0 / MIT / BSD-3-Clause) so it can be
redistributed on the public GitHub Pages deploy. GPL-licensed conformance
fixtures from the main `samples/` tree are deliberately **not** included here —
the demo (like the rest of this MIT crate) keeps clear of GPL material.

| File | Format | Source | License |
|---|---|---|---|
| `synthetic_nirs.csv` | delimited-text | Generated for this project | CC0 |
| `synthetic_nirs.h5` | hdf5-nirs | Generated for this project | CC0 |
| `synthetic.dpt` | bruker-dpt | Generated for this project | CC0 |
| `PE1800.DX` | jcamp-dx | IUPAC JCAMP-DX.org official test data, via [`nzhagen/jcamp`](https://github.com/nzhagen/jcamp) | MIT |
| `nir.spc` | galactic-spc | [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) | MIT |
| `fieldspec_vnir.asd` | asd-fieldspec | [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader) (`v8sample00001.asd`, ASD revision 8) | MIT |
| `opus_ftir.0` | bruker-opus | [`joshduran/brukeropus`](https://github.com/joshduran/brukeropus) (`examples/brukeropus_file.0`) | MIT |
| `spectra.sp` | perkin-elmer-sp | [`paris-saclay-cds/specio`](https://github.com/paris-saclay-cds/specio) | BSD-3-Clause |

Full provenance for every fixture in the repository lives in the per-format
`samples/<family>/README.md` files.
