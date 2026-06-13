# Export Files to Retrieve

Generated on 2026-05-26 from `docs/FORMAT_MATRIX.md`, `docs/MISSING_SAMPLES.md`, and the pages under `docs/formats/`. For each batch, request the original file if possible, a readable export of the same scan, the instrument model, software version, measurement mode, and a few control values. Keep structural metadata even when the data must be anonymized.

## Missing original files to retrieve

| Priority | Format / machine | Original files to request | Useful companion export | Why |
|---|---|---|---|---|
| P0 | Foss NIRSystems / WinISI / ISIscan | Native `.NIR`, `.DA`, `.cal`, `.eqa` | CSV/TXT from the same set + WinISI/ISIscan version | Key industrial format, no usable native binary. |
| P0 | Perten DA / Inframatic | Native vendor spectral file, not just a target report | CSV/XLSX with wavelength columns | Key industrial format, no usable native/export spectral sample. |
| P1 | ASD FieldSpec calibration | Complete `.asd` sets + `.ILL`, `.REF`, `.RAW` | ASCII with white/dark/reference | Unlocks calibration companions. |
| P1 | ASD FieldSpec legacy | v3/v4/v5 `.asd` revisions and files with internal reference/calibration/audit/signature blocks | ASCII export if available | Recent revisions pass; older firmware remains to confirm. |
| P1 | Avantes AvaSoft 8 | `.RWD8`, `.ABS8`, `.TRM8`, `.RFL8`, `.RIR8`, `.RMN8`, `.RMD8`, multi-subfile set, complete `.IRR8` | Readable AvaSoft export | Many active suffixes have no fixture. |
| P1 | Avantes AvaSoft 6/7 | Legacy binary `.ABS`, other non-ASCII binary modes | Readable AvaSoft export | Main gap in the legacy reader. |
| P1 | BUCHI NIRCal / NIRMaster | `.nir` with non-null targets, `.cal`, NIRMaster/NIRFlex variants | JCAMP-DX or CSV from the same project | Useful reader, but no public target-rich fixture. |
| P1 | Generic HDF5 NIRS | Real `.h5`/`.hdf5` with spectra/axes/metadata/targets, nested groups, transposed matrices, multi-signals | Schema description or export script | Real metadata-rich schemas are needed. |
| P1 | JCAMP-DX spectral | `.jdx`, `.dx`, `.jcm`, `.jcamp` with `LINK`, `PEAK TABLE`, `PEAK ASSIGNMENTS`, `NTUPLES` | Vendor export if possible | Basic coverage is OK; real multi-block/peak cases need validation. |
| P1 | Metrohm Vision / Vision Air / OMNIS NIR | Native database/project if possible, real Vision Air exports | CSV/XLSX with spectral axis | Synthetic CSV only; native database is closed. |
| P1 | PP Systems UniSpec SC | Raw field acquisition `.SPT` | Optional text export | Parser validated only on synthetic data. |
| P1 | PP Systems UniSpec DC | Raw two-channel field acquisition `.SPU` | Optional text export | Two-channel parser validated only on synthetic data. |
| P1 | Si-Ware NeoSpectra Scanner | Single-measurement export per scan, CSV/XLSX or app format | Cloud export if available | Real matrices are OK; one-measurement-per-file format is absent. |
| P1 | Spectral Evolution / PSR / SR | `.sed` from SR-3500/SR-6500/recent firmware, reflectance/radiance/DN | Export or `spectrolab`/`specdal` reference dump | SR variants and conformance need expansion. |
| P1 | Spectro Inc. SiWare API | Real API JSON responses + associated CSV | API field documentation | Current fixtures are synthetic. |
| P1 | SVC / GER SIG | `.sig` HR-1024i firmware >= 3.0, calibrated radiance files, historical GER | Comparable `spectrolab` exports | Improves physical units and byte-level conformance. |
| P1 | VIAVI MicroNIR | Native `.pri` project | CSV/XLSX from the same scan | Real exports are OK; native format is customer-only. |
| P2 | Vendor Allotrope ADF | Instrumental `.adf` from Waters/Sciex/Agilent/Bruker or others | Equivalent export + units/ontology | Local ADF is partial; SDK/vendor validation is missing. |
| P2 | Allotrope ASM | ASM JSON from multiple vendor conversions | Source instrument export | Benchling covered; industrial diversity needs validation. |
| P2 | AnIML | Real spectral `.animl`, XSD/conformance, multiple `SeriesSet` | Source export if possible | Current spectral examples are synthetic or non-spectral. |
| P2 | Bruker OPUS legacy | OPUS 5/6 `.0`, `.1`, `.001`, `.0000`, files without extension, 2D/imaging blocks | DPT/CSV from the same scan | OPUS 7/8 and MPA OK; legacy/imaging remain to cover. |
| P2 | Bruker Tango / Matrix | Native OPUS files from Tango FT-NIR and Matrix | DPT/CSV from the same scan | MPA covered; dedicated Tango/Matrix fixtures missing. |
| P2 | ENVI / hyperspectral cubes | `.hdr` + `.dat`/`.img` sets from Specim, HySpex, Headwall, Specim IQ; NEON AOP HDF5 | Sensor metadata, ROI, calibration | ENVI/AVIRIS OK; field HSI families need sourcing. |
| P2 | FGI HDF5 + XML | Real `.h5`/`.hdf5` pair + `.xml` sidecar | Complete XML schema | Current mapping is synthetic only. |
| P2 | Horiba LabSpec / JobinYvon | `.l6s` single-spectrum, other LabSpec6 `.l6m` | Matching text/XML export | `.l6m` map is experimental; single-spectrum absent. |
| P2 | JASCO JWS | `.jws` V-780/V-series NIR and NRS Raman, `Data`, `Header`, `XdataValue` streams | JASCO text export | Lab/NIR/Raman variants absent. |
| P2 | Spectral MATLAB MAT / RData | Real `.mat` v5/v7.3 and `.RData` with heterogeneous structures, cubes, targets, metadata | Generation script if possible | Arbitrary structures need broader coverage. |
| P2 | MFR-7 / MFRSR | Redistributable real `.OUT`, ARM NetCDF with calibration, `_FillValue`, filters, QC | YAML/QC if available | ARM NetCDF local only; redistributable `.OUT` absent. |
| P2 | Microtops II / MAN | Legacy Microtops II `.TXT`, redistributable MAN ASCII/NetCDF exports, complete header | AERONET/MAN documentation | MAN local OK; public `.TXT` absent. |
| P2 | Generic NetCDF NIRS | Real spectral `.nc`/`.cdf` with wavelengths, spectra, metadata, QC, multi-signals | Schema notes | Dedicated schemas OK; genericity needs strengthening. |
| P2 | Ocean Optics / Ocean Insight | QE Pro, Maya, Apex exports; real non-Galactic Ocean `.spc` | OceanView/SpectraSuite export from the same scan | Recent devices have no fixture. |
| P2 | PerkinElmer Spectrum / Lambda / Spotlight | `.sp` NIR/Lambda, `.fsm` Spotlight imaging | CSV/TXT from the same scan | `.sp` single-spectrum OK; imaging and NIR/Lambda variants need sourcing. |
| P2 | Renishaw WDF | `.wdf` InVia Qontor/Apollo, other `MAP` layouts, maps/depth/time-series | Equivalent CSV/ASCII | Strong coverage but incomplete layouts/conformance. |
| P2 | Shimadzu UVProbe | Real native Shimadzu `.spc` and real UVProbe `.txt` | Compared export | Current `.txt` is synthetic; native `.spc` missing. |
| P2 | Specim IQ / field cubes | Usable reduced Specim IQ archive, identified raw/processed data | Clear license + metadata | Possible source, but currently too large/not isolated. |
| P2 | Thermo / Galactic GRAMS SPC | New big-endian `.spc`, old headers/logs, atypical multi-subfile files | Export or reference read | LSB variants OK; BE/old logs missing. |
| P2 | Thermo Nicolet OMNIC | `.srsx`, other high-speed/rapid-scan `.srs`, `.spa/.spg` variants | ASCII export | SPA/SPG/SRS useful; `.srsx` absent. |
| P2 | WiTec WIP / WID | `.wip`, `.wid` with varied layouts | ASCII export from the same project | One map layout OK; general layouts need sourcing. |
| P3 | ENVI Spectral Library legacy | `.slb` with `.hdr` | ENVI export if available | Closes a low-impact legacy variant. |
| P3 | Excel legacy | Spectral OLE `.xls`, real `.xlsm` with macros, real multi-sheet workbooks | CSV from the same workbook | Import robustness, non-blocking. |
| P3 | MODTRAN albedo | Redistributable MODTRAN/ONTAR `.dat` output | Clear license | Outside core NIRS; no real sample. |
| P3 | USGS SPECPR | Original SPECPR binary, AREF dumps with verifiable axes | ASCII conversion | USGS/ECOSTRESS text files OK; binary absent. |

## Formats whose content goes beyond spectra alone

| Format | What the file can contain in addition to spectra | Preserve / verify during retrieval | Why it matters |
|---|---|---|---|
| ASD FieldSpec `.asd` + companions | Internal secondary/dependent/reference/calibration blocks, audit/signatures, `.ILL/.REF/.RAW` calibration | Primary file + companions, dark/reference timestamps, firmware version, calibration labels | Some data are not yet emitted as signals but must be inventoried. |
| Avantes AvaSoft 6/7/8 | Raw/sample/dark/reference/irradiance, irradiance calibration, multi-subfile, instrument/operator metadata | All files from the same scan and suffixes by mode | Rebuilding absorbance/transmittance/irradiance may require companion files. |
| BUCHI NIRCal `.nir` | Properties/targets, replicates, `Spectra Info`, project/spectrum GUID, device/serial, timestamps | Complete project with non-null targets and replicates | A `.nir` is a transfer/calibration, not just an X matrix. |
| Bruker OPUS | Multiple signals in one file: absorbance, reflectance, sample/reference, interferograms, phase, reports | Keep every block and a comparable DPT/CSV export | Multiple blocks can represent versions or treatments of the same scan. |
| JCAMP-DX | Multi-block `LINK`, `NTUPLES`, peak tables, assignments, FID/NMR, X checkpoints | Do not split files; keep all linked blocks | One file can contain multiple spectra or sparse/peak data instead of a simple curve. |
| Thermo / Galactic SPC | Single/common/independent-X layouts, multi-subfile, NIR/FTIR/Raman/NMR/MS, old headers | Raw original file + instrument/domain indication | The `.spc` extension is collision-prone and the layout changes data semantics. |
| Thermo Nicolet OMNIC `.spa/.spg/.srs/.srsx` | Spectrum groups, TGA/GC series, rapid-scan, secondary time/Y axes, raw/reprocessed | Complete series + ASCII export if possible | `.srs` files are matrices/series, not single spectra. |
| SVC / GER `.sig` | Reference/target/reflectance, overlap policy, factors, foreoptic, detector metadata, GPS, battery, errors | Raw non-resampled file + optional resampled export | Reflectance often depends on reference and overlap corrections. |
| Spectral Evolution `.sed` | DN reference/target, reflectance, GPS, instrument/foreoptic, batteries, integration times, dark mode | Original file with explicit units | Some files are DN-only or have inconsistent declared columns. |
| Generic HDF5 NIRS | Nested groups, multi-signals, shared axes, targets, global attributes, transposed matrices | Complete tree and schema description | HDF5 conventions vary strongly by laboratory/instrument. |
| FGI HDF5 + XML | HDF5 payload + XML metadata sidecar | Always provide the `.xml` + `.h5/.hdf5` pair | XML carries metadata and references the HDF5 payload. |
| NetCDF / ARM / Microtops / MFRSR | Time series, QC arrays, YAML sidecar, global attributes, multiple filters/channels | Complete NetCDF + associated QC/header files | Quality flags and axes often come from metadata or sidecars. |
| ENVI Standard / ERDAS LAN / HSI cubes | Image cube, axis sidecars, ground-truth `.GIS`, ROI/masks, spatial coordinates | All sidecars (`.hdr`, `.spc`, `.GIS`) + spatial context | Each pixel is a spectrum; labels/classes are in separate files. |
| MATLAB MAT / RData | Heterogeneous structures, X/y matrices, targets, labels, cubes, `_gt.mat` sidecars | Complete workspace + script/export describing variables | Variable names often indicate spectrum/axis/target roles. |
| Renishaw WDF | Spectra, maps, line/depth/time-series, white-light image metadata, `MAP` analysis blocks | Raw `.wdf` + CSV/ASCII export from the same mapping | Maps and derived analyses do not reduce to a curve. |
| Horiba LabSpec `.l6m/.l6s` / XML/TXT | Maps, line scans, spatial coordinates, energy/wavenumber/wavelength axis, instrument metadata | Binary + paired text/XML export | Binary/export comparison is needed to stabilize layouts. |
| WiTec `.wip/.wid` | Complete project: maps, line scans, images/navigation, TDGraph objects, physical coordinates | Raw project + ASCII export from the same project | The content is a project tree, not a flat spectrum file. |
| JASCO `.jws` | OLE2 streams `DataInfo`, `Y-Data`, `BaseInfo`, multi-channel CD/HT/Abs, fluorescence/IR/NIR/Raman | Complete OLE file + text export | Channels can have distinct semantic roles. |
| Ocean Optics / Ocean Insight | Text with vendor metadata, Jaz multichannel `W/I/P`, ProcSpec XML/ZIP, white-reference | Complete archive/file + acquisition mode | Columns alone are not always enough to type the signal. |
| Consumer Physics SCiO CSV | `spectrum`, `wr_raw`, `sample_raw` groups, device/sample metadata, targets | Complete CSV with preamble | An export may contain processed and raw/reference signals. |
| Allotrope ADF | HDF5 data cubes + RDF/triplestore, secondary axes, ontological units | Complete `.adf` + mapping/SDK if available | The ontology determines cube and unit meaning. |
| Allotrope ASM JSON | Data cubes, endpoint results, device/control settings, converter metadata | Complete JSON and source instrument | JSON can describe spectra, endpoints, and experimental context. |
| AnIML | XML with `SeriesSet`, axes, explicit or auto-incremented values, sample metadata | Complete XML + XSD/version | Multiple series can coexist in the same document. |
| DigitalSurf `.sur/.pro` | Multi-spectrum profiles, hyperspectral maps, surfaces, zlib compression, spatial axes | Complete file + exported object type | Some data are surfaces/profiles, not direct NIRS spectra. |
| Princeton TriVista `.tvf` | Multiple frames, time-series, maps, multi-spectrometer, Step-and-Glue, hardware metadata | Complete `.tvf` + acquisition notes | One file can contain navigation, frames, and multiple spectrometers. |
| Excel / XLSX / XLSM | Multiple sheets, metadata/targets, macros, axes converted to dates, wide matrices | Complete unconverted workbook | CSV conversion can lose sheets, types, and metadata. |
| NumPy `.npy/.npz` / Parquet | X matrices, axes, sample IDs, targets, schema metadata | Complete archive with all arrays/columns | Axis/target/sample roles depend on keys or columns. |
