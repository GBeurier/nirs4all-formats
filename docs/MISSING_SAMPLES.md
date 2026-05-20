# Missing samples — exact list (format / instrument / version)

Inventory date: 2026-05-18, refreshed 2026-05-20 after the public-data sweep
(see `docs/FORMAT_MATRIX.md` § "Sweep d'echantillons publics (2026-05-20)"
for the full diff). Cross-referenced against `docs/FORMATS.md` §1–6.

For every format listed in `FORMATS.md`, this file states:

- **Status** — ✅ real open sample present · 🟡 only ASCII export / partial · ⚪ only synthetic placeholder · ❌ nothing at all
- **What's still missing** — exact file kind (vendor binary, calibration companion, specific firmware revision)
- **Why** — vendor SDK, credentialed cloud, paywalled archive, etc.
- **Where to look** if you ever get access to one

If the row is ✅ with nothing in "Still missing", the format is fully covered for v1.

For the decision-level explanation of untreated formats, sample blockers,
unknown layouts and deliberate refusal paths, see
[`FORMAT_GAPS.md`](FORMAT_GAPS.md).

---

## ❌ No sample of any kind

| # | Format | Instrument | Vendor | Version / variant | Status | Why missing | Where it might come from |
|---|---|---|---|---|---|---|---|
| 1 | Allotrope ADF `.adf` | (analytical instruments, pharma) | Allotrope Foundation | binary HDF5 + RDF triplestore (any version) | ❌ | Membership-gated. Real ADFs ship with Allotrope-conformant pharma instruments. No GitHub mirror, no public-archive sample. | Allotrope Foundation membership; pharma user with an Allotrope-conformant device (Waters Empower, Sciex OS, etc.). |

---

## 🟡 Only synthetic placeholder or ASCII-export workaround

These have a synthetic file matching the documented shape, but no real vendor binary. Replace as soon as a real export from a user is obtained.

| # | Format | Instrument | Vendor | Specific version / variant | Status | Why missing | Where it might come from |
|---|---|---|---|---|---|---|---|
| 2 | `.ILL` / `.REF` / `.RAW` | FieldSpec Pro / FS3 / FS4 / HandHeld calibration companions | ASD / Malvern Panalytical | all revisions | ⚪ | Vendor SDK only — never redistributed; SPECCHIO has partial readers behind login. | ASD instrument SDK; SPECCHIO partner agreement. |
| 3 | `.NIR` native | NIRSystems 5000 / 6500 / XDS / DA / DS | Foss | WinISI II `.NIR` / `.DA` / `.cal` / `.eqa` binary | ⚪ | Pure-binary vendor format, no open reader. | A Foss WinISI II / Vision DS owner; export to text is the practical v1 path. |
| 4 | DA / Inframatic native | DA 7250 / Inframatic 9500 | Perten (PerkinElmer) | binary feed-analyzer files | ⚪ | Customer-only. The agro-feed user community shares CSV reports, not binaries. | A Perten DA / Inframatic owner. |
| 5 | Vision project DB | NIRS XDS / DS2500 / Vision Air | Metrohm | native project file (post-2010 firmware) | ⚪ | Closed format; only the CSV/Excel export path is public. | A Metrohm Vision Air customer. |
| 6 | `.spc` (Shimadzu native) | UVProbe UV-1900 / UV-2700 | Shimadzu | proprietary `.spc` (NOT Galactic) | ⚪ | Header magic differs from Galactic; only experimental readers exist (`pyfasma-spc`). | A UVProbe user; vendor support for the binary spec. |
| 7 | `.pri` project | MicroNIR Pro / MicroNIR OnSite | VIAVI / JDSU | binary project container (post-2018 firmware) | ⚪ | Customer-only; no GitHub mirror. | A VIAVI MicroNIR Pro customer. |
| 7a | Real MicroNIR 1700 CSV / XLSX | MicroNIR 1700 EC/ES | VIAVI | real measurement export | ✅ | **Resolved 2026-05-20** — UvA forensic dataset (Figshare 21252300, CC-BY-4.0) ships `K_Avg_MicroNIR.xlsx` and `T_Avg_MicroNIR.xlsx`. | — |
| 8 | NeoSpectra Scanner single-measurement CSV | NeoSpectra Scanner / NeoSpectra Micro | Si-Ware | real customer **per-measurement** export (one file per scan) | ⚪ | Customer-only; cloud API behind credentials. | A Si-Ware NeoSpectra customer. |
| 8a | NeoSpectra wide-CSV / XLSX (real, multi-sample) | NeoSpectra Handheld | Si-Ware | real multi-sample database export | ✅ | **Resolved 2026-05-20** — OSSL Woodwell+KSSL slice (Zenodo 13122321) + UvA forensic `K_Avg_NeoSpectra.xlsx` (Figshare 21252300) now ship in `siware_neospectra/`, both CC-BY-4.0. | — |
| 9 | SiWare API JSON / CSV stream | NeoSpectra Cloud | Spectro Inc. | JSON Web API response payload | ⚪ | API is credential-gated. | A Spectro Inc. customer who can dump an HTTP response. |
| 10 | `.OUT` | Microtops II Sun Photometer / Aethalometer | Solar Light | real field acquisition (MFR-7 channel layout) | ⚪ | AERONET archive requires login; no GitHub mirror. | AERONET (NASA GSFC) registered access. |
| 10a | Microtops II MAN NetCDF | Microtops II handheld sun-photometer (Maritime Aerosol Network) | Solar Light / AERONET MAN | NetCDF export of cruise AOD + Ångström + water vapour | ✅ | **Resolved 2026-05-20** — PANGAEA 966645 (MSM114/2 cruise, RV Maria S. Merian), CC-BY-4.0, ships `microtops_arc_msm114_2.nc`. Legacy `.TXT` Microtops ASCII export is *still* an open gap. | — |
| 10b | Microtops II `.lev10/.lev15/.lev20` ASCII | Microtops II handheld sun-photometer | Solar Light / AERONET MAN | Raw cruise ASCII export with 8-channel AOD + Ångström + WV | 🟡 (local) | **Resolved 2026-05-20, kept local** — AERONET MAN Okeanos Explorer 2019 cruise zip placed in `samples_local/microtops/` (AERONET PI-coauthorship policy means we cannot redistribute). | — |
| 11a | MFR-7 / MFRSR real NetCDF | SGP E11 MFRSR 7-channel | YES Inc. / DOE ARM | b1-level NetCDF, 6 narrowband + broadband | 🟡 (local) | **Resolved 2026-05-20, kept local** — ARM-DOE/arm-test-data fixture in `samples_local/mfr/` (ARM Data Use Policy). | — |
| 11b | PP Systems UniSpec-DC real exports | Toolik Lake LTER plots | PP Systems | Multi-year vegetation indices (NDVI/EVI/PRI/etc.) | 🟡 (local) | **Resolved 2026-05-20, kept local** — Arctic LTER knb-lter-arc.10185.7 derived indices CSV+XLSX in `samples_local/pp_systems/`. Raw `.SPT/.SPU` still not publicly available. | A PP Systems UniSpec customer for the raw binary. |
| 25a | BUCHI NIRCal real `.nir` with non-null targets | NIRMaster B-30 / N-300 / N-500 / NIRMaster Pro | BUCHI / Buhler | Calibration-transfer file with real cannabinoid targets | 🟡 (local) | **Resolved 2026-05-20, kept local** — orellano-c/transpec_info `DEMO_file_cannabis.nir` (105 spectra × 3 reps × 35 samples) in `samples_local/buchi_nircal/`. | — |
| 11 | `.SPT` / `.SPU` | UniSpec SC / UniSpec DC | PP Systems | real field acquisition (SC = single channel, DC = dual) | ⚪ | No GitHub fixture in any open-source ecosystem. | A PP Systems UniSpec owner. |
| 12 | albedo `.dat` (real) | MODTRAN5 / MODTRAN6 albedo library | Spectral Sciences / AFRL | real albedo bands from a MODTRAN run | ⚪ | Distributed under MODTRAN license; not freely redistributable. | A MODTRAN license holder. |
| 13 | FGI HDF5 + XML pairing | (FGI lab spectrometers) | Finnish Geodetic Institute | real FGI-schema HDF5 | ⚪ | Schema is institutional; no public fixture. | A Finnish FGI / NLS researcher. |
| 14 | UVProbe `.txt` (real) | UV-1900 / UV-2700 | Shimadzu | real customer export | ⚪ | Synthetic only — the actual layout matches Shimadzu docs but no real export was found. | Any UVProbe user. |
| 15 | JASCO `.txt` Raman export (real) | NRS-4500 / NRS-7500 Raman | JASCO | real Raman text export | ⚪ | Synthetic JASCO V-770 export only — Raman path not exercised. | A JASCO NRS-series owner. |
| 16 | WiTec `.wip` / `.wid` binary | alpha300 / alpha500 confocal Raman | WiTec | binary project file (any firmware) | ✅ | **Resolved 2026-05-20** — Zenodo 7907659 (ZrO₂ Raman imaging, ODbL v1.0) ships `Sa1-Sa5.wip`; `Sa4.wip` (19 MB, smallest) is committed in `raman_witec/`. Detection magic now validated against a true vendor binary; native decoder still pending. | — |
| 17 | Horiba LabSpec `.l6m` mapping binary | LabRAM HR Evolution / LabRAM Odyssey / XploRA | Horiba (Jobin Yvon) | LabSpec 6 native binary | ✅ | **Resolved 2026-05-20** — `ccoverstreet/horiba-raman` (MIT) ships `AlN_Gd2O3_indepth.l6m`, paired with its ASCII twin for reverse-engineering. | — |
| 17a | Horiba LabSpec `.l6s` single-spectrum binary | LabRAM HR Evolution / LabRAM Odyssey / XploRA | Horiba (Jobin Yvon) | LabSpec 6 single-spectrum binary | 🟡 | No open reader and no `.l6s` fixture yet; only `.l6m` (map) is covered. | A Horiba LabRAM customer with a "Save As Spectrum" `.l6s` file. |

---

## ✅ Real samples present but version coverage is partial

These have at least one real sample, but specific instrument or firmware revisions are still missing.

| # | Format | What is covered | Specific instrument / version that is still missing | Why |
|---|---|---|---|---|
| 18 | ASD `.asd` | v6, v7 (lab + field spectroscopy variants), v8 | **No v3/v4/v5 (legacy FieldSpec 1/FS3 firmware)** — those revisions still circulate in archival data | Legacy archives are private; revisions 3-5 dates back to 2003-2010 deployments. |
| 19 | Bruker OPUS native | OPUS 7.x / 8.x / new-data (post-2020) `.0` / `.0000` / `.001` / `.1` across **four independent reader projects** (opusreader2 GPL-3, opusreader GPL-3, brukeropus MIT, cran/soil.spec AfSIS Bruker MPA) | **No OPUS 5.x or 6.x files** — older infrared archives from 2000s. **No Bruker Tango (FT-NIR) demo file specifically** — same OPUS binary, but Tango-specific blocks. | Older OPUS archives are private; Tango owners are typically pharma/agro customers. AfSIS Bruker MPA samples (Africa Soil Information Services, 2014) now ship — these confirm the MPA family parses with the same code path. |
| 20 | Galactic `.spc` | Old + new header, LSB/MSB byte order, -XY/-XYY/-XYXY layouts, NIR/FTIR/Raman/NMR/MS/UV-Vis flavours | **No Thermo GRAMS .spc multi-channel (≥3 channels)** — covered analytically by `m_xyxy.spc` but not by a real instrument fixture. | Multi-channel instruments are niche. |
| 21 | Thermo Nicolet OMNIC | `.spa` single, `.spg` group (4.7 MB nh4y), `.srs` time-series (GC_Demo 5.7 MB + TGAIR 2.6 MB) | **No `.srsx`** (extended series, newer OMNIC ≥9.7) | `.srsx` files are larger; no small fixture available. SpectroChemPy notes this format is "tricky". |
| 22 | Perkin Elmer | `.sp` (single spectrum) | **`.fsm` Spotlight imaging** (50 MB available in `specio` but explicitly skipped). No PE Lambda 1050 NIR-specific blocks. | `.fsm` is 50 MB — too large for fixture; explicitly out of scope for v1 anyway per FORMATS.md §2. |
| 23 | JASCO `.jws` | UV-Vis fluorescence (V-series), CD/HT/Abs (CD spectrometer), IR (V-770) | **No JASCO V-780 NIR-specific blocks**, **no JASCO NRS-series Raman binary** | JASCO V-780 has slightly different binary layout; Raman is a separate `.jws` flavour. |
| 24 | Foss WinISI / DA / DS3 | Text exports (WinISI / DS3 CSV — synthetic) + **real Foss XDS Monochromator XM-1000 and NIRSYSTEM-5000 CSV exports** (sensAIfood Cordoba, CC-BY-4.0, **added 2026-05-20**) | **No real WinISI II `.NIR` text export** specifically — covered formats are the XDS / NIRSYSTEM-5000 wide CSV variant. Native `.NIR` binary remains pure-binary and unavailable. | The WinISI text export is the same wide-CSV layout (different metadata header); a real WinISI II export would round out the directory but is functionally equivalent. |
| 25 | BUCHI NIRCal | `.nir` calibration-transfer file (plant tissue) | **No `.cal` calibration-only files**, **no NIRMaster B-30/N-300 vendor variants** | These ship only with the NIRMaster firmware. |
| 26 | Avantes AvaSoft 6/7 | `.TRM`, `.ABS`, `.ROH`, `.DRK`, `.REF` (mostly transmittance) | **No `.IRR` v6/v7 (legacy absolute irradiance)**, **no `.RMN` Raman v6/v7** | These per-mode variants exist in vendor docs but no open fixture. |
| 27 | Avantes AvaSoft 8 | `.Raw8`, `.IRR8` | **No `.RWD8` (raw dark)**, **no `.ABS8` / `.TRM8` / `.RFL8` / `.RIR8` / `.RMN8` / `.RMD8`** explicitly — `lightr` does not ship them either | The AvaSoft 8 manual documents all suffixes but only a subset are in `lightr`. |
| 28 | Ocean Optics | SpectraSuite / OceanView / `.ProcSpec` / Jaz / OceanOptics-flavour `.spc` / period-decimal `.jdx` | **No Ocean Optics QE Pro firmware exports**, **no Maya/Apex pro `.txt` exports** | Newer Ocean Insight handhelds may have firmware-specific text formats. |
| 29 | Spectral Evolution `.sed` | PSR (Brett's DN) — working + intentionally-broken + **PSR-3500 grape-leaf reflectance** (serbinsh/R-FieldSpectra) | **No SR-3500 / SR-6500 firmware specifics** — covered models are PSR / PSR-3500 only | Other Spectral Evolution models have minor header drift. |
| 30 | SVC / GER `.sig` | Acer leaf (SVC HR-1024) + Serbin BNL (SVC laptop + SVC moc) + **GER 3700 PDA raw acquisition** (`serbinsh_gr070214_003.sig`) + **HR-1024i Barrow Environmental Observatory phenology field** (`serbinsh_BEO_CakeEater_Pheno_026_resamp.sig`) | **No SVC HR-1024i firmware ≥3.0 specifically** | GER PDA was retired 2009; the new `serbinsh_gr070214_003.sig` closes the long-standing "no raw GER 3700" gap. |
| 31 | USGS SPECPR | ASCII export (asphalt from splib06a) | **No binary `SPECPR` records** (52 MB `splib06a` exists but too large for fixture) | Binary archive is huge; ASCII export is the documented v1 path anyway. |
| 32 | ENVI SLI | Synthetic 50×200 library + ENVI mini-cube (CubeScope demo) + ECOSTRESS/ASTER/USGS ASCII references + **Real USGS splib06a and splib07 ENVI libraries convolved to AVIRIS-1995** (via `capstone-coal/pycoal` — GPL-2 wrapper / USGS data public domain) | Closed for v1 — every documented USGS Spectral Library variant is now covered alongside the L3Harris-style synthetic | — |
| 33 | Hyperspectral cubes | AVIRIS 92AV3C (`.lan` + `.spc` + `.GIS`) | **No NEON AOP HDF5 reflectance tile** (smallest is ~50 MB on data.neonscience.org), **no Specim IQ scene**, **no HySpex / Headwall cube**, **no AVIRIS-NG real cube** (only legacy AVIRIS) | All these live on cloud archives behind registered access, not on GitHub. AVIRIS-NG cubes are typically 4–8 GB. |
| 34 | Renishaw `.wdf` | 17 files covering all acquisition modes (sp / line / map / depth / zscan / streamline / focustrack / timeseries / interrupted) | **No `.wdf` from InVia Qontor or Apollo specifically** (the rsciio fixtures are anonymized — instrument model is not in the metadata) | Vendor doesn't ship per-model fixtures. |
| 35 | DigitalSurf `.sur` / `.pro` | Surface, spectral map, spectrum, RGB | **No AFM-Raman combo `.sur`** specifically (the spectral_map *is* AFM-Raman generic, but no Bruker / NanoSurf / Park branded fixture) | Vendor-branded fixtures are private. |

---

## Summary (2026-05-20 refresh)

- **165** open sample files now cover **38 of 47** format directories with at
  least one real, verified upstream sample (+4 directories vs the 2026-05-18
  snapshot: Foss WinISI text exports, real NeoSpectra, real MicroNIR, and the
  Horiba `.l6m` + WiTec `.wip` binaries).
- **8** directories now carry only synthetic placeholders (Microtops, MFR,
  PP Systems, MODTRAN, Metrohm, Shimadzu, Perten, AnIML/FGI/SiWare API/JASCO
  Raman). Every one of them is a closed-vendor format where the *documented
  text/CSV export* is the v1 path anyway (so the parser will work as soon as a
  user contributes a real export).
- **1** directory (`allotrope_adf/`) has nothing at all — no realistic open
  fixture exists, and the Allotrope ADF binary is not even a v1 priority per
  `FORMATS.md` §3.
- **Critical missing pieces without a documented text fallback**: ASD `.ILL` /
  `.REF` / `.RAW` calibration triplet, Foss `.NIR/.DA` natifs, Perten DA /
  Inframatic native, Metrohm Vision Air native, VIAVI `.pri` project, Horiba
  `.l6s` single-spectrum binary, Bruker OPUS 5/6 legacy, Thermo OMNIC `.srsx`,
  Shimadzu UVProbe `.spc` native. The 2026-05-20 sweep specifically unblocked
  Foss text exports, NeoSpectra (real), VIAVI MicroNIR (real), Horiba `.l6m`,
  WiTec `.wip` and Excel handheld exports — see `FORMAT_MATRIX.md` for sources
  and licences.

If you can supply a real binary for any row in the 🟡 or ⚪ tables above, drop it into the matching subdirectory and update both that directory's README and the row's status here.
