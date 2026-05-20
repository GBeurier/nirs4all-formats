# Thermo / Galactic GRAMS SPC

Status: experimental native reader.

## Scope Implemented

The Rust reader recognizes Galactic/Thermo GRAMS SPC by the file-version byte,
not by extension. This is mandatory because `.spc` is also used by unrelated
Ocean Optics, Shimadzu, Renishaw and hyperspectral fixtures.

Implemented:

- new little-endian header `FVERSN = 0x4B`;
- generated evenly spaced X axes from `ffirst`, `flast`, `fnpts`;
- explicit global X arrays when `TXVALS` is set;
- multi-subfile common-X files as one `SpectralRecord` per subfile;
- independent-X `TXYXYS` / `-XYXY` files, including directory-backed subfile
  offsets;
- fixed-point 32-bit Y arrays, fixed-point 16-bit Y arrays (`TSPREC`), and
  IEEE float32 Y arrays (`fexp` or `subexp` equal to `0x80`);
- axis and signal labels from SPC enumerations, with `TALABS` custom labels
  overriding the enum labels when present;
- log-text key/value parsing when the SPC log block is present.

Limited support:

- old little-endian header `FVERSN = 0x4D` is decoded for generated-X files and
  old word-swapped Y values, but old-format XY/log variants are not complete.

Not implemented yet:

- new big-endian `FVERSN = 0x4C`;
- full old-format multi/ordered-Z semantics;
- binary log payloads;
- promotion of quantitative calibration metadata into `targets`.

## Record Mapping

Every readable subfile becomes one normalized `SpectralRecord`. This keeps
multi-spectrum SPC files usable by the Python and R tabular adapters when the
subfiles share an axis. Independent-X `-XYXY` files remain available through
raw records; `open_dataset()` intentionally rejects them until an explicit
resampling or ragged-array policy is added.

Reader metadata includes the decoded global header under `galactic_spc`, the
subfile header under `galactic_spc_subfile`, optional `galactic_spc_log`, and a
top-level `sample_id`. If the log contains `SUBFILE<n>` labels, those labels
are used as sample IDs; otherwise the reader emits `subfile_<n>`.

## Fixtures and Reference Checks

Committed smoke and golden coverage currently includes:

| Fixture | Variant | Expected shape |
|---|---|---|
| `BENZENE.SPC` | new LSB, generated X | 1 record, 1842 absorbance points |
| `s_xy.spc` | new LSB, explicit global X | 1 record, 512 points |
| `nir.spc` | new LSB, multi common-X | 20 records, 700 points each |
| `m_xyxy.spc` | new LSB, `-XYXY` directory | 512 records, variable point counts |
| `LC_DIODE_ARRAY.SPC` | old LSB | limited old-header smoke test |

Reference comparisons were checked against the local `spc_spectra` Python
reader for the new-LSB fixtures. Important controls:

- `BENZENE.SPC`: first Y `0.1015599817`, sum `189.390214`.
- `s_xy.spc`: first X `1.0866667032`, first Y `45333`, sum `30065112`.
- `nir.spc`: 20 records, first record first Y `0.0002004839`, sum `238.526`.
- `m_xyxy.spc`: 512 records, first subfile has 8 points, first X
  `16943.600006`, first Y `6823`, sum `45327`.

`spc_spectra` does not implement new big-endian SPC and is unreliable for at
least one old ordered-Z fixture, so old-format promotion requires an additional
independent review or another reference reader.

## Next Work

- Add adversarial truncation tests for SPC header, global X arrays, subfile
  data and directory entries.
- Add full-array reference conformance for representative new-LSB fixtures.
- Decide whether independent-X SPC files should expose a ragged Python/R
  adapter or require explicit resampling before tabular export.
- Expand old `0x4D` and future `0x4C` coverage when reliable references or
  controlled fixtures are available.
