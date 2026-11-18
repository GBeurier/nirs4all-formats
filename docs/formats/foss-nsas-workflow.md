# Rapitec / NSAS calibration workflow

The documentation received on 2026-11-18 explains the relationship between the
[decoded DA spectra](foss-nsas-da.md), constituent calibrations, product settings
and the Beer Analyzer deployment file. It describes their logical contents and
the original software procedures. It does not provide byte layouts for `.CA`,
`.OPR` or `RAPITEC.RAM`, and no examples of those binaries were supplied.

## Sources and page references

These are private user-supplied scans; the PDFs, page images and full OCR are not
redistributed. References below give both the 1-based PDF page and the page
printed in the original manual. Important passages were checked against the
page images after local OCR and orientation correction.

| Reference | Document | Identification and pagination |
|---|---|---|
| R | `RapitecUserManual.pdf` | Beer Analyzer Manual, FOSS NIRSystems, version 3.0, part 720-750-0075, February 1998; 66 PDF pages. |
| N1 | `3. Calibration 1.pdf` | NSAS Calibration chapter, version 3, 1993; 31 PDF pages containing printed pages 3-1, 3-3, …, 3-61. |
| N2 | `3. Calibration 2.pdf` | The even pages of that chapter in reverse order: PDF pages 1–31 correspond to printed pages 3-62, 3-60, …, 3-2. |

The manuals describe a historical workflow; their versions do not establish
which software or firmware created the 1996 acquisitions. The complete NSAS
chapter must be read by interleaving N1 with N2 in reverse order, rather than by
concatenating the PDFs. OCR is a navigation aid; it is insufficient for copying
numeric coefficients or inferring binary field codes.

## File roles

| Component | Documented role | What remains unverified |
|---|---|---|
| `.DA` | Stored spectra used by NSAS. The Analyzer creates or appends to `RAPITEC.DA` on the RAM card; copying and renaming that file is documented. NSAS can associate laboratory reference information with data files. | Where reference values or processing flags reside in additional DA variants; the three received files have no decoded constituent targets. |
| `.CA` | A constituent calibration, containing an equation plus descriptive and acquisition/processing context. The Beer Analyzer manual explicitly names MLR and PLS models. | Byte offsets, field lengths, numeric representations, signatures and actual fitted coefficients. |
| `.OPR` | An operations file associates products with constituent calibrations and the settings needed to use them. | Serialized product tables, references to CA files, correction fields and any checksums. |
| `RAPITEC.RAM` | An instrument calibration assembled from an operations file and its associated constituent calibrations, suitable for the embedded processor. | Binary packing, processor representation, identifiers, checksums and deployed calibration contents. |

The file-role definitions come from R PDF page 45 / printed page 39. The original
DA storage and append behavior is described in R PDF pages 39–40 / printed
pages 33–34. R PDF page 52 / printed page 46 explains copying or directly
reading `RAPITEC.DA` from a PCMCIA card and recommends a unique name when archiving
it. Thus renamed DA files are compatible with the documented workflow; filenames
alone cannot identify their selected products or calibration versions.

## What a constituent calibration contains

N2 PDF page 19 / printed page 3-26 lists an equation, creation date and time,
instrument model, spectral range, product and constituent names, mathematical
transformation and up to two descriptive lines associated with a `.CA` file.
These are semantic search targets for a future reader, not a serialized schema.

The standard-regression example gives the single-term relationship
`concentration = K(0) + K(1) * absorbance(wavelength)` (N2 PDF page 23 / printed
page 3-18). The chapter also describes multiple linear and ratio terms and
allows transducer values to enter a regression (N2 PDF page 19 / printed
page 3-26). Consequently:

- A calibration reader must preserve term types and selected wavelengths, rather
  than assuming every equation is a single wavelength and two constants.
- The three stored DA transducer values may be needed by an eventual calibration.
  Their units and their use in these particular acquisitions remain unknown;
  they must not be relabelled as alcohol, extract, temperature or pathlength.
- PLS is a separate supported model family. The chapter describes saving
  loadings, weights and equations and selecting the number of factors (N1 PDF
  page 23 / printed page 3-45; N2 PDF page 9 / printed page 3-46).
- A bias adjustment can change `K(0)` without changing the other constants
  (N2 PDF page 4 / printed page 3-56). A recreated calibration therefore needs
  the correct historical version and corrections, not just a model family.

The chapter names preprocessing operations, including smoothing, derivatives,
Savitzky–Golay and other transforms. It also describes segment and gap controls,
and the possibility of writing transformed spectra to a new data file while
preserving the original (N2 PDF page 31 / printed page 3-2; N1 PDF page 2 /
printed page 3-3). It supplies no reliable mapping from the observed DA constant
words to those controls. The decoder therefore leaves those words uninterpreted.

## Reference values and prediction

The NSAS instructions allow laboratory information to accompany spectra or to
be added through Edit Data (N2 PDF page 31 / printed page 3-2). This does **not**
prove that references are present in the three supplied DA files, nor identify
their byte positions. The absence of decoded targets in the current reader must
not be generalized to all NSAS DA files.

Percent Predict applies a saved calibration to stored spectra; Routine Analysis
performs prediction during live acquisition. Percent Predict applies processing
that has not already been applied (N2 PDF page 8 / printed page 3-48). Tracking
the preprocessing state is necessary to avoid transforming data twice.

The prediction procedure requires data covering the calibration's spectral range.
When laboratory references are available, its output includes sample numbers and
names, laboratory values, predictions, residuals and groups; without references,
predictions can still be displayed (N1 PDF page 26 / printed page 3-51). Such an
export is a useful independent check for future semantic decoding.

## Operations files and firmware conventions

The factory example is `NIRSBEER.OPR`. Its table associates the first two products
with `INTL1ALC.CA` and `INTL1REX.CA`, another product with `NOAMALC.CA` and
`NOAMREX.CA`, and the pathlength product with `PATHLEN.CA` (R PDF page 46 /
printed page 40). These filenames are documented examples, not files recovered
from the supplied archive.

R PDF pages 47–48 / printed pages 41–42 give ordering rules:

- For the first three products, constituent 1 must be alcohol and constituent 2
  must be real extract if the firmware's beer calculations are to be valid.
- A pathlength product and its constituent calibration must be included, but the
  product need not occupy the fourth position.
- Later products may also represent beers, but do not receive the same predefined
  beer-parameter calculations. Products can have more than two constituents.

The 26 additional beer parameters are calculated in firmware from the measured
alcohol and real extract, rather than being 26 independent constituent models
inside the calibration file (R PDF page 45 / printed page 39; Appendix D starts
at PDF page 53 / printed page 47). In that appendix, the two measured inputs are
defined on a weight/weight percentage basis. The formulas do not recover their
missing underlying calibration coefficients. Implementing prediction or those
calculations is outside the Formats reader's scope.

## Building and deploying the instrument calibration

Appendix B describes selecting an existing operations file under NSAS's embedded
processor configuration and writing an instrument calibration named
`RAPITEC.RAM`, then copying it to the PCMCIA card (R PDF pages 50–51 / printed
pages 44–45). It also names a supplied **Calibration Files** diskette with an
original `RAPITEC.RAM` under `\RAMFILE`.

There is also a direct RS232 route. Although the UI presents it as downloading
an operations file, the manual explicitly explains that an instrument
calibration is assembled in an intermediate step (R PDF page 41 / printed
page 35). That label is not evidence that a raw `.OPR` is sent unchanged.

R PDF pages 48–49 / printed pages 42–43 mention roughly 25 constituent
calibrations as a working estimate and illustrative sizes of several hundred
bytes for MLR versus about 6000 bytes for PLS. The maximum RAM-file size is
explicitly unknown there. Neither those estimates nor the RAM-card capacity
define a safe binary parser limit.

## Result and next useful evidence

The documented **logical** chain is now established: stored spectra and optional
reference data support calibration development; `.CA` equations are associated
with products through `.OPR`; NSAS assembles the instrument calibration used
by the analyzer. It cannot yet be reproduced or decoded at the byte level.

Even an incomplete archive can help. In descending order of immediate value:

1. A single `.CA`, ideally a simple MLR example, together with its NSAS
   Calibration Files display or report: constants, wavelengths, math/segment/gap
   settings and any transducer terms.
2. Any `.OPR`, `RAPITEC.RAM`, **Calibration Files** diskette image, `\RAMFILE`
   directory, or original RAM-card/backup directory. A complete matching set is
   useful for end-to-end validation, but is not required to begin byte analysis.
3. An Edit Data or Percent Predict export for a supplied DA file, with any
   laboratory values that actually exist.
4. The NSAS Edit Data, Calibration Files and mathematical-theory sections, plus
   any `.WAV` wavelength selection or model loadings/weights saved separately.

No calibration binary reader, coefficient reconstruction, header-code mapping or
successful deployment of a recreated calibration is claimed from these manuals.
