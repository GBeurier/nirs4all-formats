# Renishaw `.wdf` (Raman / SERS)

OLE-style binary container (`WDF1` magic). Holds spectra + map / line / depth / streamline / time-series acquisitions + extensive instrument metadata.

## Samples

### From [`hyperspy/rosettasciio`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/renishaw) — GPL-3.0

Test fixtures used by `rsciio.renishaw`. Small (<5 MB total), each exercising a different acquisition mode.

| File | Mode | Size |
|---|---|---|
| `renishaw_test_spectrum.wdf` | Single point spectrum | 244 KB |
| `renishaw_test_linescan.wdf` | Linescan | 324 KB |
| `renishaw_test_map.wdf` | XY map | 325 KB |
| `renishaw_test_map2.wdf` | XY map (variant) | 2.0 MB |
| `renishaw_test_timeseries.wdf` | Time series | 264 KB |
| `renishaw_test_zscan.wdf` | Z scan (depth) | 231 KB |
| `renishaw_test_streamline.wdf` | Streamline mapping | 3.6 MB |
| `renishaw_test_focustrack.wdf` | Focus tracking | 265 KB |
| `renishaw_test_focustrack_invariant.wdf` | Focus tracking (invariant) | 119 KB |
| `renishaw_test_exptime10_acc1.wdf` | Variable exposure time | 255 KB |
| `renishaw_test_undefined.wdf` | Undefined / minimal | 13 KB |
| `interrupted_acquisition.wdf` | Interrupted scan (negative test) | 294 KB |

### From [`spectrochempy/spectrochempy_data@master/testdata/ramandata/wire/`](https://github.com/spectrochempy/spectrochempy_data/tree/master/testdata/ramandata/wire) — MIT

Real-world Renishaw acquisitions, larger and richer than the rsciio fixtures.

| File | Mode | Size |
|---|---|---|
| `wire_sp.wdf` | Single spectrum | 74 KB |
| `wire_depth.wdf` | Depth profile | 236 KB |
| `wire_line.wdf` | Line scan | 1.2 MB |
| `wire_Streamline.wdf` | Streamline mapping | 3.7 MB |
| `wire_undefined.wdf` | Undefined / minimal | 13 KB |

## Parser hints

- Magic: bytes 0-3 are `57 44 46 31` (`WDF1`).
- Structure: chunk-based, similar in spirit to RIFF — each block has a 16-byte header + payload. Block types include `DATA`, `XLST`, `YLST`, `ORGN` (origin metadata), `WMAP` (map info), `WHTL` (white-light image), `WXIS` (X-axis info), `WXDA` (X-axis data), `WXDB`, etc.
- Current native reader: single-spectrum subset only (`measurement_type=1`,
  `count=1`) through `DATA`, `XLST` and `YLST`. Map/line/depth/time-series
  fixtures are kept as next-step reverse-engineering targets and negative
  coverage.
- Reference reader:
  - Python: [`rsciio.renishaw`](https://hyperspy.org/rosettasciio/) (most active — supports all map modes), [`py-wdf-reader`](https://github.com/alchem0x2A/py-wdf-reader) / [`renishawWiRE`](https://pypi.org/project/renishawWiRE/) (older but documented).
- Sample-count: a single `.wdf` from a map mode can contain tens of thousands of spectra — load lazily.
