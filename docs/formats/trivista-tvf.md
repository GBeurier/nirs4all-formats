# Princeton TriVista TVF

Status: experimental / partial.

The TriVista reader covers the committed RosettaSciIO `.tvf` Raman fixtures:

- XML `XmlMain` / `Document` containers with ASCII `Frame` payloads;
- non-uniform wavelength axes from `xDim/Calibration@ValueArray`;
- one `SpectralRecord` per frame for single spectra, time series, line scans
  and maps;
- `InfoSerialized` X/Y navigation groups mapped to `spatial_x`,
  `spatial_y`, frame indices and `um` units;
- Windows FILETIME-style frame timestamps mapped to `time_filetime_100ns` and
  `elapsed_time_seconds`;
- Step-and-Glue files emit the primary glued spectrum plus each child document
  as separate records, preserving the source windows.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/raman_trivista/spec_1s_1acc_1frame_average.tvf` | 1 | wavelength, `nm`, 1024 points | Single averaged spectrum |
| `samples/raman_trivista/spec_3s_1acc_2frames_average.tvf` | 2 | wavelength, `nm`, 1024 points | Two frame records |
| `samples/raman_trivista/spec_3s_2acc_1frame_average.tvf` | 1 | wavelength, `nm`, 1024 points | Two accumulations, averaged |
| `samples/raman_trivista/spec_3s_2acc_1frame_no_average.tvf` | 1 | wavelength, `nm`, 1024 points | Two accumulations, not averaged |
| `samples/raman_trivista/spec_multiple_spectrometers.tvf` | 1 | wavelength, `nm`, 1024 points | Multi-spectrometer setup metadata |
| `samples/raman_trivista/spec_step_and_glue.tvf` | 20 | wavelength, `nm`, 18000 or 1024 points | Glued primary plus 19 child windows |
| `samples/raman_trivista/spec_timeseries_2x1s_delta3s.tvf` | 2 | wavelength, `nm`, 1024 points | Timestamp-derived elapsed time |
| `samples/raman_trivista/linescan.tvf` | 21 | wavelength, `nm`, 97 points | X line scan navigation |
| `samples/raman_trivista/map.tvf` | 81 | wavelength, `nm`, 1024 points | 9 by 9 XY map navigation |

## Binary Notes

Despite the historical "binary spectroscopy file" wording, these fixtures are
XML documents. Spectral values live in semicolon-separated `Frame` text nodes.
The spectral axis is stored as a pipe-separated `ValueArray` whose first field
declares the point count.

The `InfoSerialized` attribute is escaped XML. The current reader decodes the
top-level `Experiment`, `Detector`, `Calibration`, `X-Axis` and `Y-Axis`
groups. Hardware filtering and richer spectrometer/objective metadata are left
to a later conformance pass.

## Remaining Gaps

The committed RosettaSciIO corpus is fully covered by goldens. The remaining
partial status is not a sample blocker: it is a conformance and metadata task,
mainly full-array comparison against `rsciio.trivista`, richer hardware /
objective metadata and an explicit scope decision for variants outside this
fixture corpus.

## Reference Readers

Layout and fixture behavior are cross-checked against `rsciio.trivista` 0.13.0.
RosettaSciIO defaults to the glued spectrum for Step-and-Glue; `nirs4all-io`
also emits child documents so low-level consumers can inspect source windows.
