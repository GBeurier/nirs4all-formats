# Hamamatsu IMG

Status: experimental adjacent-format reader.

Hamamatsu `.img` files from HPD-TA streak-camera systems are 2D
time-resolved images, not point-sample NIR spectra. `nirs4all-io` still loads
the committed fixtures because the core record model can represent a signal
with dimensions `y,x`: `x` is wavelength or pixel position, while `y` is a
secondary time or detector-position axis stored in metadata. Calibrated
time-resolved Y axes are tagged as `time`; uncalibrated detector-position axes
remain `index`.

The reader covers:

- `IM` little-endian headers;
- 8-bit, 16-bit and 32-bit unsigned payloads;
- UTF-8 HPD-TA comment sections;
- calibration tables referenced by `#offset,count`;
- RosettaSciIO-compatible X-axis reversal to ascending wavelength order;
- focus, operate, photon-counting, shading-reference and uncalibrated-X modes.

## Supported Fixtures

| Fixture | Records | Signal Shape | Axis | Notes |
|---|---:|---|---|---|
| `samples/hamamatsu/focus_mode.img` | 1 | `512 x 672` | wavelength, `nm` | Y axis is vertical CCD position in pixels |
| `samples/hamamatsu/operate_mode.img` | 1 | `512 x 672` | wavelength, `nm` | Y axis is time in `us`, typed `time` |
| `samples/hamamatsu/photon_counting.img` | 1 | `512 x 672` | wavelength, `nm` | Photon-counting acquisition, Y axis in `ns`, typed `time` |
| `samples/hamamatsu/shading_file.img` | 1 | `512 x 672` | wavelength, `nm` | Shading/reference style acquisition, Y axis in `ps`, typed `time` |
| `samples/hamamatsu/xaxis_other.img` | 1 | `508 x 672` | pixel index, `px` | Uncalibrated X and Y axes |

All files emit warnings identifying them as streak-camera 2D signals. Files
with time axes use `hamamatsu_img_secondary_time_axis_in_metadata`; focus-style
files use `hamamatsu_img_y_axis_is_detector_position`.

## Binary Notes

The fixed header is 64 bytes:

```text
0   char[2]  "IM"
2   i16le    comment_length
4   i16le    image_width_px
6   i16le    image_height_lines
8   i16le    offset_x
10  i16le    offset_y
12  i16le    file_type: 0=bit8, 1=compressed, 2=bit16, 3=bit32
14  i32le    num_images_in_channel
18  i16le    num_additional_channels
20  i16le    channel_number
22  f64le    timestamp
30  char[4]  marker
34  char[30] additional_info
64  utf8     comment, length = comment_length
```

The payload follows the comment and stores `width * height` unsigned values.
Calibration tables referenced in the comment live later in the file as float32
arrays. The current reader refuses the unused `file_type=1` compressed variant.

## Reference Readers

Fixture values and axes are cross-checked against `rsciio.hamamatsu` 0.13.0.
RosettaSciIO is GPL-3.0 and is used only as an external conformance reference,
not as a runtime dependency.
