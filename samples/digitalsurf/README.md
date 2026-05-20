# DigitalSurf `.sur` / `.pro` (Surface / spectral / hyperspectral maps)

Binary format from Digital Surf Mountains software, widely used as the export from AFM-Raman combo instruments (Bruker, NanoSurf, Park) and from optical surface profilers. Carries spectra, line profiles, surfaces, and hyperspectral maps in a single container.

## Samples

All from [`hyperspy/rosettasciio@main/rsciio/tests/data/digitalsurf/`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/digitalsurf) — GPL-3.0.

| File | Type |
|---|---|
| `test_surface.sur` | Plain surface (height map). |
| `test_spectral_map.sur` | **Hyperspectral map** (XY of spectra). |
| `test_spectral_map_compressed.sur` | Same, RLE-compressed. |
| `test_spectrum.pro` | Single profile spectrum (`.pro` = profile). |
| `test_spectra.pro` | Multi-spectrum profile. |

## Parser hints

- The Digital Surf `.sur`/`.pro` format is a chunk container — each block has a leading header with type, dimensions, units, and compression flag, followed by the payload.
- Reference reader: [`rsciio.digitalsurf`](https://hyperspy.org/rosettasciio/).
- For hyperspectral maps the spectral axis is encoded in the W (channel) dimension; the loader has to detect this and expose the spectra rather than treating it as a 3-D image.
