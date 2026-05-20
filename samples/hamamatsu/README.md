# Hamamatsu streak camera `.img`

Binary streak-camera output (high-speed time-resolved luminescence / Raman / fluorescence). Acquisition modes encoded in the file: focus, operate, photon-counting, shading, custom-X-axis.

## Samples

All from [`hyperspy/rosettasciio@main/rsciio/tests/data/hamamatsu/`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/hamamatsu) — GPL-3.0.

| File | Mode |
|---|---|
| `focus_mode.img` | Focus mode acquisition. |
| `operate_mode.img` | Standard operate mode. |
| `photon_counting.img` | Photon counting mode. |
| `shading_file.img` | Shading-correction reference. |
| `xaxis_other.img` | Non-default X axis. |

## Parser hints

- Reference reader: [`rsciio.hamamatsu`](https://hyperspy.org/rosettasciio/).
- The streak-camera image is 2-D (time × wavelength) — semantically a hyperspectral 2-D map, not a single spectrum.
- For NIRS pipelines this format is **fringe** (high-speed luminescence rather than continuous NIR sweep); listed for completeness as a "refuse cleanly with pointer to rsciio" candidate.
