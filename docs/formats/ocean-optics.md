# Ocean Optics / Ocean Insight

Experimental native Rust reader for Ocean Optics-style ASCII exports.

## Scope Implemented

The current reader covers the committed text fixtures:

- SpectraSuite text exports with `>>>>>Begin Processed Spectral Data<<<<<`;
- OceanView text exports with `>>>>>Begin Spectral Data<<<<<`;
- OOIBase32 `*.Master.Transmission` text exports;
- Jaz ASCII exports (`.jaz`, `.JazIrrad`) with `W/D/R/S/P` columns;
- CRAIC two-column reflectance text export;
- simple two-column Ocean-style CSV export.

The first tranche intentionally does not parse:

- `.ProcSpec` ZIP/XML/binary containers;
- Ocean Optics `.spc` binary flavor;
- Ocean Optics JCAMP beyond what the JCAMP reader can already decode.

Those require separate sniffing because `.ProcSpec` is a proprietary mixed
container and `.spc` collides with Galactic SPC and other vendor families.

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

Metadata is preserved under `metadata.vendor` using normalized key names. The
reader stores the source file name there as well, because some workflows encode
the measurement type in the extension rather than in the text header.

## Fixtures and Reference Checks

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

`lightr` is the practical external reference for this family, but it remains a
conformance-only dependency because the Rust core is MIT.

## Next Work

- Decode `.ProcSpec` enough to expose the processed spectrum and checksum
  metadata.
- Disambiguate Ocean Optics `.spc` from Galactic SPC at the sniffer level.
- Add reference reports against `lightr`.
- Improve semantic typing of generic `processed` spectra when the export
  records processing mode in metadata rather than column labels.
