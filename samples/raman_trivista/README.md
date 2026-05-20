# Princeton Instruments / TriVista `.tvf` (Raman)

Binary spectroscopy file from the Princeton Instruments / Princeton Acton / TriVista line of triple-stage Raman spectrometers.

## Samples

All from [`hyperspy/rosettasciio@main/rsciio/tests/data/trivista/`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/trivista) — GPL-3.0.

| File | Mode |
|---|---|
| `spec_1s_1acc_1frame_average.tvf` | Single spectrum, 1 s, 1 acc, 1 frame average. |
| `spec_3s_1acc_2frames_average.tvf` | 3 s, 2 frames averaged. |
| `spec_3s_2acc_1frame_average.tvf` | 3 s, 2 accumulations. |
| `spec_3s_2acc_1frame_no_average.tvf` | 3 s, 2 accumulations, no average. |
| `spec_multiple_spectrometers.tvf` | Multi-spectrometer triple-stage configuration. |
| `spec_step_and_glue.tvf` | Step-and-glue grating mode. |
| `spec_timeseries_2x1s_delta3s.tvf` | Time series. |
| `linescan.tvf` | Line scan. |
| `map.tvf` | XY map. |

## Parser hints

- Reference reader: [`rsciio.trivista`](https://hyperspy.org/rosettasciio/) — production-quality.
- Current native reader: XML frame payloads, wavelength `ValueArray`, X/Y
  navigation from `InfoSerialized`, timestamps and Step-and-Glue child
  documents are covered.
- `xDim@Length`, `Frame@xDim`, calibration label/unit/display/type/laser wave,
  detector fields and numbered spectrometer groups are asserted by semantic
  tests and goldens.
- The RosettaSciIO X/Y navigation groups do not carry a unit item; the native
  reader therefore records `spatial_*_unit = unknown` for those fixtures.
- Step-and-glue / multi-spectrometer files contain disjoint X axes that the
  parser must keep separate (similar to `.spc` -XYXY layout).
