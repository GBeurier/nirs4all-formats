# MFR Sun Photometer `.OUT`

Regular fixed-width text. The format reports per-channel solar irradiance measurements with timestamp, geo and airmass metadata. SPECCHIO claims support.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_mfr.OUT` | Generated locally | CC-0 | Mock MFR-7 export with `Record HH:MM:SS AirMass Channel_415 … Channel_940` columns. |

## Parser hints

- Whitespace-separated, fixed columns.
- Two header lines (site/date metadata) before the column-name row.
- Multi-channel sun photometers (MFR-7 = 7 channels at 415/500/614/673/870/940 + broadband). Wavelengths come from the column-name row.
- **No publicly redistributable real samples** were found on GitHub; the AERONET archive (NASA GSFC) hosts MFR/sun-photometer data but requires registration.
