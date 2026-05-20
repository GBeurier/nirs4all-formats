# mzML

Status: detected and refused.

mzML is a HUPO PSI standard for mass spectrometry. It is useful design
inspiration for `nirs4all-io`, but it is not a NIRS / optical spectroscopy
format and its primary X axis is `m/z`, which is outside the current
`SpectralAxis` model.

The native registry now detects XML `.mzML` / `.mzMLb` text containers and
returns a clear error directing users to `pyteomics`, `pymzML` or `pyOpenMS`.
It does not decode spectra, chromatograms, zlib arrays or MS-Numpress payloads.

## Covered Fixtures

| Fixture | Behavior | Notes |
|---|---|---|
| `samples/mzml/example.mzML` | refused | MS1 spectrum collection |
| `samples/mzml/mini.chrom.mzML` | refused | Chromatogram example |
| `samples/mzml/mini_numpress.chrom.mzML` | refused | Chromatogram with MS-Numpress compression |

## Decision

Do not coerce `m/z` arrays into NIRS `SpectralRecord` objects. If mzML support
is needed later, it should be introduced as an explicit adjacent MS model or a
separate adapter, not as a silent optical-spectrum import path.
