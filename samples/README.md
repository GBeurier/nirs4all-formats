# Sample fixtures for `nirs_loader`

This directory holds at least one representative file per format listed in
[`docs/FORMATS.md`](../docs/FORMATS.md), used as ingestion fixtures for the
parsers and as regression test data.

## Provenance and licensing

Every sample is either:

1. **Downloaded from an open-source repository** — provenance and license are
   documented in each subdirectory's `README.md`. The dominant upstream
   sources are:
   - [`ropensci/lightr`](https://github.com/ropensci/lightr) — Avantes, Ocean
     Optics, Bruker DPT, Jaz (**GPL-2**).
   - [`spectrolab`](https://github.com/meireles/spectrolab) — Spectral
     Evolution `.sed`, SVC/GER `.sig` (**GPL-3**).
   - [`spectral-cockpit/opusreader2`](https://github.com/spectral-cockpit/opusreader2) —
     Bruker OPUS native (GPL-3 R package).
   - [`spectrochempy/spectrochempy_data`](https://github.com/spectrochempy/spectrochempy_data) —
     Nicolet OMNIC `.SPA`/`.SPG`, Galactic `.SPC`, OPUS, MATLAB datasets (**CeCILL-B**).
   - [`pierreroudier/asdreader`](https://github.com/pierreroudier/asdreader) —
     ASD `.asd` (GPL-3).
   - [`KaiTastic/pyASDReader`](https://github.com/KaiTastic/pyASDReader) —
     ASD v6/v7/v8 (**MIT**).
   - [`l-ramirez-lopez/prospectr`](https://github.com/l-ramirez-lopez/prospectr) —
     BUCHI NIRCal `.nir`, prospectr `NIRsoil` (MIT).
   - [`nzhagen/jcamp`](https://github.com/nzhagen/jcamp) — IUPAC JCAMP-DX
     official test suite + IR/UV-Vis spectra library (**MIT**).
   - [`cheminfo/spc-parser`](https://github.com/cheminfo/spc-parser) — Galactic
     `.spc` variants (**MIT**).
   - [`paris-saclay-cds/specio`](https://github.com/paris-saclay-cds/specio) —
     Perkin Elmer `.sp` (**BSD-3-Clause**).
   - [`odoluca/jasco_jws_reader`](https://github.com/odoluca/jasco_jws_reader),
     [`gnezd/Jasco_jws`](https://github.com/gnezd/Jasco_jws) — JASCO `.jws`.
   - [`ns-bak/splib06.library`](https://github.com/ns-bak/splib06.library) —
     USGS SPECPR ASCII export (USGS public domain).
   - [Eigenvector](https://eigenvector.com/data/) — Corn, NIR shootout 2002
     benchmark `.mat` files (non-commercial research redistribution).
   - [NIST WebBook](https://webbook.nist.gov/) — IR JCAMP-DX (U.S. Government
     public domain).
   - [`apache/parquet-testing`](https://github.com/apache/parquet-testing),
     [`h5py/h5py`](https://github.com/h5py/h5py),
     [`Unidata/netcdf-c`](https://github.com/Unidata/netcdf-c) — generic format
     fixtures (Apache-2.0 / BSD-3).
   - [`hyperspy/rosettasciio`](https://github.com/hyperspy/rosettasciio) —
     Renishaw `.wdf`, JobinYvon/Horiba XML, Princeton TriVista `.tvf`, DigitalSurf
     `.sur`/`.pro`, Hamamatsu streak `.img`, EMSA/MAS ISO 22029 `.msa` (**GPL-3**).
   - [`pymzml/pymzML`](https://github.com/pymzml/pymzML) — mzML (**MIT**).
   - [`PyMassSpec/PyMassSpec`](https://github.com/PyMassSpec/PyMassSpec) —
     ANDI MS `.cdf` (**GPLv2** — data only, no reader code vendored).
   - [`spectralpython/sample-data`](https://github.com/spectralpython/sample-data) —
     AVIRIS 92AV3C hyperspectral cube (academic use, no SPDX in repo).
   - [`Benchling-Open-Source/allotropy`](https://github.com/Benchling-Open-Source/allotropy) —
     Allotrope **ASM** JSON instances (**MIT**).
   - [`FAIRmat-NFDI/pynxtools-raman`](https://github.com/FAIRmat-NFDI/pynxtools-raman) —
     WiTec WIP ASCII export (**Apache-2.0**).
   - [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) —
     LabSpec 6 mapping export (**MIT**).
2. **Generated locally** with [`scripts/gen_synthetic.py`](#) (CC-0) for the
   handful of formats where no permissively-licensed open fixture could be
   located (Microtops, PP Systems, Si-Ware NeoSpectra, VIAVI MicroNIR,
   Foss WinISI text export, MODTRAN albedo, Metrohm Vision Air CSV export,
   AnIML, FGI HDF5+XML, Shimadzu text export, JASCO text export). Synthetic
   files contain a documented 50-sample × 200-band realistic NIRS shape
   (1100–2500 nm absorbance with three Gaussian peaks at typical NIR
   bands).

Per-format `README.md` files document each file individually.

## Inventory

Legend: ✅ real open-source sample(s) · 🟡 partial (synthetic + reference
ASCII / lacking native binary) · ⚪ generated synthetic only.

| Subdirectory | Format | Status | Files | Open sample sources |
|---|---|---|---|---|
| `asd/` | ASD `.asd` (multiple revisions) | ✅ | 6 | asdreader, prospectr, pyASDReader (v6/v7/v8 + v7 field) |
| `avantes/` | Avantes AvaSoft 6/7 binaries, AvaSoft 8 binaries, ASCII exports | ✅ | 12 | lightr |
| `bruker_dpt/` | Bruker OPUS `.dpt` text export | ✅ | 2 | lightr + synthetic |
| `bruker_opus/` | Bruker OPUS native | ✅ | 7 | opusreader2, brukeropus, spectrochempy_data |
| `buchi_nircal/` | BUCHI NIRCal `.nir` | ✅ | 1 | prospectr |
| `csv_tsv/` | CSV / TSV / IDL-ENVI text | ⚪ | 4 | synthetic |
| `envi_sli/` | ENVI Spectral Library + cube + USGS/ECOSTRESS/ASTER ASCII | 🟡 | 9 | synthetic SLI + CubeScope-demo cube + spectralpython USGS/ECOSTRESS + ASTER-Spectral-Library |
| `excel/` | Excel `.xlsx` | ⚪ | 1 | synthetic |
| `fgi/` | FGI HDF5 + XML | ⚪ | 2 | synthetic |
| `foss_winisi/` | Foss WinISI text export / DS3 CSV report | 🟡 | 2 | synthetic (no open native `.NIR` sample exists) |
| `galactic_spc/` | Thermo / Galactic GRAMS `.spc` | ✅ | 16 + spec PDF | cheminfo/spc-parser, spectrochempy_data |
| `hdf5/` | Generic HDF5 | ✅ | 2 | h5py + synthetic |
| `jasco/` | JASCO `.jws` + text export | ✅ | 4 | jasco_jws_reader, gnezd/Jasco_jws + synthetic |
| `jcamp_dx/` | JCAMP-DX (all encodings) | ✅ | 21 | nzhagen/jcamp (IUPAC official suite + IR library) + NIST WebBook |
| `matlab/` | MATLAB `.mat` / `.RData` | ✅ | 7 | Eigenvector Corn + NIR Shootout 2002, spectrochempy_data, prospectr + synthetic |
| `metrohm/` | Metrohm Vision Air CSV export | ⚪ | 1 | synthetic |
| `microtops/` | Microtops sun photometer `.TXT` | ⚪ | 1 | synthetic |
| `modtran/` | MODTRAN5 albedo `.dat` | ⚪ | 1 | synthetic |
| `netcdf/` | NetCDF (ANDI-adjacent) | ✅ | 3 | netcdf-c, xarray-data + synthetic |
| `nicolet_omnic/` | Thermo Nicolet OMNIC `.spa` / `.spg` / `.srs` | ✅ | 3 | spectrochempy, spectrochempy_data |
| `mfr/` | MFR Sun Photometer `.OUT` | ⚪ | 1 | synthetic |
| `perten/` | Perten DA / Inframatic CSV report | ⚪ | 1 | synthetic |
| `siware_api/` | Spectro Inc. SiWare API JSON / CSV | ⚪ | 2 | synthetic |
| `numpy/` | NumPy `.npy` / `.npz` | ⚪ | 2 | synthetic |
| `ocean_optics/` | Ocean Optics SpectraSuite / OceanView / ProcSpec / Jaz | ✅ | 12 | lightr |
| `parquet/` | Parquet | ✅ | 2 | apache/parquet-testing + synthetic |
| `perkin_elmer/` | Perkin Elmer `.sp` | ✅ | 1 | specio |
| `pp_systems/` | PP Systems UniSpec SC `.SPT` / DC `.SPU` | ⚪ | 2 | synthetic |
| `shimadzu/` | Shimadzu UVProbe text export | ⚪ | 1 | synthetic |
| `siware_neospectra/` | Si-Ware NeoSpectra CSV | ⚪ | 1 | synthetic |
| `specpr/` | USGS SPECPR | ✅ | 1 | ns-bak/splib06.library (USGS public domain) |
| `spectral_evolution/` | Spectral Evolution PSR `.sed` | ✅ | 2 | spectrolab |
| `svc_ger/` | SVC HR-1024 / GER 3700 `.sig` | ✅ | 13 | spectrolab |
| `viavi_micronir/` | VIAVI MicroNIR CSV | ⚪ | 1 | synthetic |
| `animl/` | AnIML XML | ✅ | 2 | KE-UniLiv/animl-ontology Example3 + synthetic |
| `allotrope_asm/` | Allotrope ASM JSON | ✅ | 4 | Benchling-Open-Source/allotropy |
| `allotrope_adf/` | Allotrope ADF binary | ❌ | 0 | **No public sample found** — see directory README |
| `hyperspectral_cubes/` | AVIRIS / generic hyperspectral cubes | ✅ | 4 | spectralpython/sample-data (academic use) |
| `raman_renishaw/` | Renishaw `.wdf` (Raman) | ✅ | 17 | rosettasciio + spectrochempy_data |
| `raman_horiba/` | Horiba LabSpec / JobinYvon (XML + text) | ✅ | 13 | rosettasciio + spectrochempy_data + ccoverstreet/horiba-raman |
| `raman_trivista/` | Princeton TriVista `.tvf` (Raman) | ✅ | 9 | rosettasciio |
| `raman_witec/` | WiTec ASCII export (`.wip` binary unavailable) | 🟡 | 1 | FAIRmat-NFDI/pynxtools-raman |
| `digitalsurf/` | DigitalSurf `.sur` / `.pro` (AFM-Raman) | ✅ | 5 | rosettasciio |
| `hamamatsu/` | Hamamatsu streak `.img` | ✅ | 5 | rosettasciio |
| `msa_iso22029/` | EMSA / MAS `.msa` (ISO 22029) | ✅ | 11 | rosettasciio |
| `mzml/` | mzML / mzMLb (MS) | ✅ | 3 | pymzml |
| `andi_ms/` | ANDI MS `.cdf` (chromatography) | ✅ | 1 | PyMassSpec |

**Totals**: 47 directories · 270 fixture files · 32 directories with at
least one real open-source sample · 14 directories synthetic-only · 1
directory with no available sample (`allotrope_adf/`).

## Known gaps (no permissively-licensed fixture exists)

Tracked here for honesty — none of these have a real open sample I could
verify; they all carry a synthetic placeholder for shape testing.

| Format | Why not found | Mitigation |
|---|---|---|
| ASD `.ILL` / `.REF` / `.RAW` companion files | Vendor SDK distribution only; SPECCHIO has partial support behind login. | Reverse-engineer from the SDK once a real workflow needs them; otherwise route to "vendor SDK only" with a clear error. |
| Foss NIRSystems / WinISI `.NIR` / `.DA` / `.cal` / `.eqa` native | Pure-binary vendor format; no open reader exists. | Ingest WinISI / DA1650 / DS2500 / DS3 text exports only (synthetic in `foss_winisi/`). |
| Metrohm Vision Air native (`.viscv`, project DB) | Closed; only the CSV export workflow is public. | Synthetic Vision Air CSV in `metrohm/`. |
| Microtops `.TXT` real samples | AERONET hosts the data behind login; no GitHub mirror found. | Synthetic in `microtops/`. |
| PP Systems UniSpec `.SPT` / `.SPU` real | No GitHub fixture found. | Synthetic in `pp_systems/`. |
| Shimadzu UVProbe native `.spc` (different from Galactic) | Proprietary; experimental readers only. | Synthetic ASCII export in `shimadzu/`. |
| VIAVI MicroNIR `.pri` project / real CSV exports | Customer-only. | Synthetic CSV in `viavi_micronir/`. |
| Si-Ware NeoSpectra Scanner CSV (real) | Customer-only. | Synthetic CSV in `siware_neospectra/`. |
| Spectro Inc. SiWare API JSON (real) | Cloud API behind credentials. | Synthetic in `siware_api/`. |
| MFR Sun Photometer real `.OUT` | AERONET-archived behind login. | Synthetic in `mfr/`. |
| Perten DA / Inframatic real CSV | No GitHub fixture found. | Synthetic in `perten/`. |
| MODTRAN5 albedo `.dat` real | Distributed with MODTRAN license. | Synthetic in `modtran/`. |
| FGI HDF5 + XML real | Schema is FGI-owned; no public fixture. | Synthetic in `fgi/`. |
| Allotrope ADF `.adf` | Heavy SDK stack; pharma-specific. **Not even attempted — listed in FORMATS.md but explicitly *not a v1 priority*.** | Document as "requires Allotrope SDK". |
| Perkin Elmer `.fsm` (imaging) | Real fixture exists (50 MB in `specio`) but **explicitly out of scope for v1** per FORMATS.md. | Skip. |
| Hyperspectral imaging cubes (Specim/HySpex/Headwall/AVIRIS-NG/NEON AOP) | Out of scope per FORMATS.md §4. | Two reference cubes (`cubescope-mini-cube`, `92AV3C.spc`) live in `envi_sli/` for refusal-path tests. |
| Renishaw WDF / WiTec WIP / Horiba LabSpec | Out of scope per FORMATS.md §5 (UV-Vis/Raman look-alikes). | None. |

**Gaps that are not blockers**: every truly proprietary format above already
has the *vendor's documented text/CSV export* as the v1 path (FORMATS.md
explicitly captures this), so the parser will work as soon as the
synthetic-shape fixture is replaced with one real export from a user. The
ASD `.ILL/.REF/.RAW` calibration triplet is the only purely-binary gap with
no documented text fallback.

## Mapping to `docs/FORMATS.md`

| FORMATS.md section | Where it lands here |
|---|---|
| §1 Field / portable spectroradiometers | `asd/`, `avantes/`, `bruker_dpt/`, `bruker_opus/`, `envi_sli/`, `fgi/`, `excel/`, `microtops/`, `ocean_optics/`, `pp_systems/`, `spectral_evolution/`, `svc_ger/`, `modtran/`, `csv_tsv/`, `specpr/` |
| §2 Benchtop / industrial / FT-NIR | `bruker_opus/`, `galactic_spc/`, `nicolet_omnic/`, `perkin_elmer/`, `foss_winisi/`, `metrohm/`, `buchi_nircal/`, `avantes/`, `jasco/`, `shimadzu/`, `viavi_micronir/`, `siware_neospectra/` |
| §3 Standardized / vendor-neutral | `jcamp_dx/`, `netcdf/`, `animl/`, `csv_tsv/`, `parquet/`, `hdf5/` |
| §4 Hyperspectral imaging (out-of-scope) | `envi_sli/cubescope-mini-cube.{hdr,img}`, `envi_sli/92AV3C.spc` |
| §5 Adjacent useful formats | `matlab/`, `numpy/`, `excel/` |

## Maintenance

When adding a new sample:

1. Place it in the matching subdirectory (create one if a new format family
   appears in `FORMATS.md`).
2. Update the subdirectory's `README.md` with file size, source URL, license,
   and any parser hints.
3. Update the inventory table above (status, file count, sources).
4. If the source is not permissively licensed, get explicit written
   permission and document it in the per-format README.

If you regenerate the synthetic fixtures, run `scripts/gen_synthetic.py`
(deterministic — `np.random.seed(42)`). Output is committed; the script is
documentation for *how* the fixtures were built, not a runtime dependency.

## What to do when a real sample is later found

Many "synthetic-only" directories above are placeholders for formats that
**do** have real binary samples behind vendor SDKs or in non-public datasets.
When a real fixture is obtained:

1. Verify the license / redistribution terms first.
2. Add the real file alongside the synthetic; keep both — the synthetic is
   still a useful "minimal shape" reference for unit tests.
3. Update the directory status from ⚪ to ✅ in the inventory above.

## Disambiguation reminders

Several extensions are shared across vendors and **must not** be dispatched
by extension alone. The parser should sniff the magic bytes / header first:

- `.spc` — Galactic (`galactic_spc/`) vs. OceanView (`ocean_optics/OceanOptics.spc`) vs. Shimadzu vs. Renishaw.
- `.nir` — BUCHI NIRCal (`buchi_nircal/`) vs. Foss NIRSystems (`foss_winisi/`, no open binary sample).
- `.sig` — SVC PDA vs. SVC laptop vs. GER 3700 (all in `svc_ger/`; same family but different header conventions).
- `.dat` — ENVI cube (`envi_sli/`) vs. MODTRAN albedo (`modtran/`).
- `.spa` — Thermo Nicolet (`nicolet_omnic/`) vs. occasional namespace collisions.
- `.nc` / `.cdf` — NIRS NetCDF export vs. ANDI MS vs. climate data (`netcdf/`).
- `.0`, `.0000`, `.001` — Bruker OPUS naming (`bruker_opus/`), but some ASD files also use sequential `.NNN` extensions (see `asd/3L9257.000`).

These collisions are why §7 of `FORMATS.md` mandates magic-byte sniffing
first; extension is only a tie-breaker.
