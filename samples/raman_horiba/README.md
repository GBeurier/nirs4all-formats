# Horiba LabSpec / JobinYvon Raman

Two output paths:

1. **JobinYvon XML** — Modern LabSpec 6 / LabRAM exports an XML wrapper around the spectral data, with explicit unit metadata.
2. **LabSpec text** — Tab/whitespace-separated ASCII (the legacy / "Save As Spectrum" path), with optional per-pixel mapping coordinates.

The native binary formats `.l6s` (single) / `.l6m` (map) are **not openly readable** in
the general case; one `.l6m` fixture is now committed (see below) thanks to the
`ccoverstreet/horiba-raman` repository.

## Samples

### JobinYvon XML (from [`hyperspy/rosettasciio`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/jobinyvon) — GPL-3.0)

| File | Notes |
|---|---|
| `jobinyvon_test_spec.xml` | Single spectrum, default units. |
| `jobinyvon_test_spec_3s_cm-1.xml` | 3 s integration, X axis in cm⁻¹. |
| `jobinyvon_test_spec_3s_eV.xml` | Same spectrum, X axis in eV. |
| `jobinyvon_test_spec_range.xml` | Range / step-and-glue (multiple gratings). |
| `jobinyvon_test_linescan.xml` | Linescan. |
| `jobinyvon_test_map_x3-y2.xml` | XY map (3×2). |

### LabSpec text exports (from [`spectrochempy/spectrochempy_data@master/testdata/ramandata/labspec/`](https://github.com/spectrochempy/spectrochempy_data/tree/master/testdata/ramandata/labspec) — MIT)

| File | Notes |
|---|---|
| `labspec_532nm_Si.txt` | Silicon 200 µm calibration spectrum @ 532 nm laser. |
| `labspec_Activation.txt` | Sample activation series. |
| `labspec_SMC1_Initial.txt` | "SMC1 initial" — catalysis sample. |
| `labspec_lasertest1.txt` | Laser power test. |
| `labspec_serie190214.txt` | Time series. |
| `labspec_LiNbWO6_pol.txt` | LiNbWO₆ polarized Raman (subset of the 0°/45°/90° H/V matrix). |

### LabSpec 6 mapping export (from [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) — MIT)

| File | Notes |
|---|---|
| `labspec6_Gd2O3_AlN_map.txt` | Gd₂O₃ in AlN substrate, 2D Raman map export (real material). |

### LabSpec 6 native binary (from [`ccoverstreet/horiba-raman@main/src/example/`](https://github.com/ccoverstreet/horiba-raman/blob/main/src/example/AlN_Gd2O3_indepth.l6m) — MIT)

| File | Size | Notes |
|---|---|---|
| `AlN_Gd2O3_indepth.l6m` | 174 KB | Real LabSpec 6 in-depth Raman map of Gd₂O₃ in AlN substrate (binary `.l6m`). Pairs with the `labspec6_Gd2O3_AlN_map.txt` ASCII export above so the binary path can be validated against a known-good text equivalent. |

## Parser hints

- JobinYvon XML: well-formed LSX XML with `<LSX_Data>`, `<LSX_Tree>`, axis
  descriptors under ID `0x7B697861`, and ordinate rows under
  `<LSX_Matrix>/<LSX_Row>`. Axis labels include `Intens`, `Spectr`, `X` and
  `Y`; unit nodes carry `Cnt`, `Cnt/sec`, `nm`, `1/cm`, `eV` or spatial
  micrometre units.
- LabSpec text: leading `#`-comment lines (Title, Acq. time, Laser, Grating,
  Date, `AxisType[n]`, `AxisUnit[n]`, ...), then either two columns, a first
  axis row followed by series rows, or an axis row followed by `X Y values...`
  map rows. Locale: `.` decimal in EN, sometimes `,` decimal in FR/DE LabSpec.
- Reference readers:
  - Python: `rsciio.jobinyvon` (production-quality XML), `spectrochempy.read_labspec()` (text), [`ccoverstreet/horiba-raman`](https://github.com/ccoverstreet/horiba-raman) (mapping text).
- Native `.l6s` / `.l6m`: no documented broad open reader. The committed
  `AlN_Gd2O3_indepth.l6m` now validates the narrow native `.l6m` map decoder
  against its `labspec6_Gd2O3_AlN_map.txt` ASCII twin; other LabSpec6 binary
  layouts still need matching samples before they can be claimed.
