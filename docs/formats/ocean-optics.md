# Ocean Optics / Ocean Insight

Experimental native Rust reader for Ocean Optics-style ASCII exports and
OceanView `.ProcSpec` archives.

## Scope Implemented

The current reader covers the committed text fixtures:

- SpectraSuite text exports with `>>>>>Begin Processed Spectral Data<<<<<`;
- OceanView text exports with `>>>>>Begin Spectral Data<<<<<`;
- OOIBase32 `*.Master.Transmission` text exports;
- Jaz ASCII exports (`.jaz`, `.JazIrrad`) with `W/D/R/S/P` columns;
- CRAIC two-column reflectance text export;
- simple two-column Ocean-style CSV export.
- OceanView `.ProcSpec` ZIP archives containing `ps_*.xml`, `OOIVersion.txt`
  and `OOISignatures.xml`.

The first tranche intentionally does not parse:

- Ocean Optics `.spc` binary flavor;
- Ocean Optics JCAMP beyond what the JCAMP reader can already decode.

The `.ProcSpec` reader validates the SHA-512 signature when
`OOISignatures.xml` is present. Ocean Optics JCAMP `LINK` exports are routed
through the JCAMP reader, which keeps sample/dark/reference arrays and computes
the processed transmittance signal. The committed Ocean Optics `.spc` fixture
uses the Galactic/Thermo new-LSB explicit-X SPC layout, so it is intentionally
routed through the Galactic SPC reader instead of a duplicate Ocean-specific
reader.

## Record Mapping

Each file becomes one `SpectralRecord`.

For two-column exports:

- first column: wavelength axis in `nm`;
- second column: `processed`, `reflectance`, `transmittance` or `irradiance`
  depending on headers and file names.

For Jaz multichannel exports:

- `W`: wavelength axis in `nm`;
- `D`: `dark_reference` raw counts;
- `R`: `white_reference` raw counts;
- `S`: `sample` raw counts;
- `P`: processed signal, mapped to `irradiance` for `Jaz Absolute Irradiance`
  files and to `processed` when the semantic type is not explicit.

For `.ProcSpec` archives:

- `channelWavelengths`: wavelength axis in `nm`;
- source `pixelValues`: `sample` raw counts;
- `darkSpectrum/pixelValues`: `dark_reference` raw counts;
- `referenceSpectrum/pixelValues`: `white_reference` raw counts;
- `processedPixels`: `processed` signal.

Metadata is preserved under `metadata.vendor` using normalized key names. The
reader stores the source file name there as well, because some workflows encode
the measurement type in the extension rather than in the text header.

## Fixtures and Value Controls

Current committed controls:

| File | Points | Signal | Axis | Value control |
|---|---:|---|---|---|
| `OOusb4000.txt` | 3648 | `processed` | `178.65 -> 888.37 nm` | last `-12.792` |
| `OceanView.txt` | 2389 | `processed` | `187.92 -> 2116.50 nm` | first `18.995` |
| `CRAIC_export.txt` | 3761 | `reflectance` | `280.11 -> 949.93 nm` | first `13.3999`, last `169.6574` |
| `FMNH6834.00000001.Master.Transmission` | 3648 | `transmittance` | `178.53 -> 889.03 nm` | first `95.380`, last `25.753` |
| `spec.csv` | 1994 | `processed` | `299.99 -> 700.03 nm` | first `10.013`, last `15.408` |
| `jazspec.jaz` | 2048 | `dark_reference`, `white_reference`, `sample`, `processed` | `190.8535 -> 886.439331 nm` | processed last `13.679238` |
| `irrad.JazIrrad` | 2048 | `dark_reference`, `sample`, `irradiance` | `191.016296 -> 891.915466 nm` | irradiance last `3.643908` |
| `OceanOptics_Linux.ProcSpec` | 3648 | `sample`, `dark_reference`, `white_reference`, `processed` | `176.360418 -> 893.694340 nm` | processed `0.0 -> 125.074331` |
| `OceanOptics_Windows.ProcSpec` | 2048 | `sample`, `dark_reference`, `white_reference`, `processed` | `190.939253 -> 888.233535 nm` | processed `282.857143 -> 40.050321` |
| `whiteref.ProcSpec` | 3648 | `sample`, `dark_reference`, `white_reference`, `processed` | `176.360418 -> 893.694340 nm` | processed `0.0 -> 97.294250` |
| `OceanOptics_period.jdx` | 3648 | `sample`, `dark_reference`, `white_reference`, computed `processed` via JCAMP reader | `176.36 -> 893.69 nm` | processed `0.0 -> 171.977070` |
| `OceanOptics.spc` | 3648 | `transmittance` via Galactic SPC reader | `176.360413 -> 893.694336 nm` | `0.0 -> 119.425171` |

All 12 committed Ocean Optics / Ocean Insight data fixtures are golden-backed.
Direct semantic tests cover the text, CSV, Jaz, CRAIC, Master.Transmission and
ProcSpec families. Ocean JCAMP is validated through the JCAMP-DX reader, and
the committed `.spc` file is validated through the Galactic SPC reader because
its binary layout is that family rather than a distinct Ocean-only container.

`lightr` is the practical external reference for this family, but it remains a
conformance-only dependency because the Rust core is MIT.

## Next Work

- Add QE Pro, Maya and Apex export variants when redistributable samples are
  available.
- Add samples for any non-Galactic Ocean Optics `.spc` variant if encountered.
- Add reference reports against `lightr` and `pavo`.
- Improve semantic typing of generic `processed` spectra when the export
  records processing mode in metadata rather than column labels.
