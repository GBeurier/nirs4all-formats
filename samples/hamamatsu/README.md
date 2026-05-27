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
- The streak-camera image is 2-D (time or vertical CCD position × wavelength) — semantically a time-resolved / streak-camera signal, not a single point-sample NIR spectrum.
- `nirs4all-formats` loads these fixtures as one 2-D `y,x` record each, with the secondary time/CCD axis preserved in metadata and explicit adjacent-format warnings.
