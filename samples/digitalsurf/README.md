# DigitalSurf `.sur` / `.pro` (Surface / spectral / hyperspectral maps)

Binary format from Digital Surf Mountains software, widely used as the export from AFM-Raman combo instruments (Bruker, NanoSurf, Park) and from optical surface profilers. Carries spectra, line profiles, surfaces, and hyperspectral maps in a single container.

## Samples

All from [`hyperspy/rosettasciio@main/rsciio/tests/data/digitalsurf/`](https://github.com/hyperspy/rosettasciio/tree/main/rsciio/tests/data/digitalsurf) — GPL-3.0.

| File | Type |
|---|---|
| `test_surface.sur` | Plain surface (height map). |
| `test_spectral_map.sur` | **Hyperspectral map** (XY of spectra). |
| `test_spectral_map_compressed.sur` | Same, zlib-stream compressed. |
| `test_spectrum.pro` | Single profile spectrum (`.pro` = profile). |
| `test_spectra.pro` | Multi-spectrum profile. |

## Parser hints

- The Digital Surf `.sur`/`.pro` format is an object container — each object has a 512-byte header with type, dimensions, units, and compression flag, followed by comments/private bytes and the payload.
- `DSCOMPRESSED` payloads in these fixtures use a small zlib-stream directory, not RLE.
- Reference reader: [`rsciio.digitalsurf`](https://hyperspy.org/rosettasciio/).
- For hyperspectral maps the spectral axis is encoded in the W (channel) dimension; the loader has to detect this and expose the spectra rather than treating it as a 3-D image.
