# Horiba LabSpec / JobinYvon

Status: experimental.

The Horiba reader covers the committed Raman LabSpec fixtures that are useful
for adjacent spectroscopy disambiguation and ML workflow compatibility:

- JobinYvon/LabSpec LSX XML exports with `<LSX_Data>`, `<LSX_Tree>` and
  `<LSX_Matrix>` payloads;
- LabSpec text exports with `#` metadata headers and either two-column,
  series-row or map-row numeric sections;
- one experimental LabSpec 6 `.l6m` binary map layout validated against the
  paired `labspec6_Gd2O3_AlN_map.txt` export.

Native binary support is deliberately narrow. The `.l6m` path scans the
LabSpec6 container for the main `Intens` float32 payload, the `Spectr` axis
payload and the matching 2D spatial axes. Unknown LabSpec6 layouts, calibration
files and `.l6s` variants remain pending until redistributable samples are
available.

## Normalization

The reader emits one `SpectralRecord` per spectrum or map pixel. The signal is
named `intensity`, uses `SignalType::RawCounts`, and keeps the native spectral
axis order.

Axis units are normalized conservatively:

- `nm` -> wavelength axis;
- `1/cm`, `cm-1` and corrupted `cm-�` headers -> wavenumber axis with unit
  `cm-1`;
- `eV` -> index axis with warning `horiba_unsupported_axis_kind_energy` until
  the core model grows an explicit energy axis kind.

For XML and text maps, spatial coordinates are stored in metadata as
`spatial_x`, `spatial_y` and unit fields. Text exports that omit the spectral
axis unit default to `cm-1` with warning
`horiba_labspec_text_axis_unit_inferred`.

The LabSpec6 binary path emits warning
`horiba_labspec6_binary_experimental` on every record because it is validated on
one public `.l6m` fixture only. For that fixture, the Rust test compares all 72
binary spectra against the paired text export: intensities match exactly,
spectral axes match within text-rounding tolerance, and `spatial_x` /
`spatial_y` use the same x-slowest/y-fastest map order as the text export.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/raman_horiba/jobinyvon_test_spec.xml` | 1 | wavelength, `nm`, 34 points | LSX XML single spectrum |
| `samples/raman_horiba/jobinyvon_test_spec_3s_cm-1.xml` | 1 | wavenumber, `cm-1`, 34 points | LSX XML single spectrum |
| `samples/raman_horiba/jobinyvon_test_spec_3s_eV.xml` | 1 | index, `eV`, 34 points | LSX XML energy-axis fallback |
| `samples/raman_horiba/jobinyvon_test_spec_range.xml` | 1 | wavelength, `nm`, 105 points | LSX XML range export |
| `samples/raman_horiba/jobinyvon_test_linescan.xml` | 3 | wavelength, `nm`, 34 points | LSX XML linescan, one record per point |
| `samples/raman_horiba/jobinyvon_test_map_x3-y2.xml` | 6 | wavelength, `nm`, 34 points | LSX XML 3 x 2 map |
| `samples/raman_horiba/labspec_532nm_Si.txt` | 1 | wavenumber, `cm-1`, 1024 points | Two-column text export |
| `samples/raman_horiba/labspec_Activation.txt` | 532 | wavenumber, `cm-1`, 1024 points | Legacy LabRam series-row text export |
| `samples/raman_horiba/labspec_SMC1_Initial.txt` | 1 | wavenumber, `cm-1`, 1024 points | Two-column text export |
| `samples/raman_horiba/labspec_lasertest1.txt` | 3 | wavenumber, `cm-1`, 1024 points | LabSpec series-row text export |
| `samples/raman_horiba/labspec_serie190214.txt` | 168 | wavenumber, `cm-1`, 1024 points | Time series-row text export |
| `samples/raman_horiba/labspec_LiNbWO6_pol.txt` | 1 | wavenumber, `cm-1`, 1024 points | Two-column text export |
| `samples/raman_horiba/labspec6_Gd2O3_AlN_map.txt` | 72 | wavenumber, `cm-1`, 498 points | LabSpec 6 map-row text export |
| `samples/raman_horiba/AlN_Gd2O3_indepth.l6m` | 72 | wavenumber, `cm-1`, 498 points | Experimental LabSpec6 binary map; intensity and spatial coordinates are compared against the paired text export |

## Reference Readers

Reference comparison targets are:

- `rsciio.jobinyvon` for JobinYvon XML;
- `spectrochempy.read_labspec()` for LabSpec text;
- `ccoverstreet/horiba-raman` for LabSpec 6 mapping text and the paired `.l6m`
  fixture.

Those comparisons are planned as isolated conformance jobs; the Rust runtime
does not import GPL reference-reader code.
