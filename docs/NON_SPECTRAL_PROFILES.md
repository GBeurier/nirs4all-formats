# Declared non-spectral byte profiles

The separate `nirs4all.formats.multimodal.v1` operation decodes ordinary images and
genotypes without creating a fake spectral axis or changing SpectralRecord.
Native Rust exports `multimodal_reader_catalog` and
`decode_multimodal_bytes(profile, bytes, max_decoded_bytes)`. WASM exports
`multimodalReaderCatalog()` and `decodeMultimodalBytes(profile, bytes,
maxDecodedBytes)`. The explicit profile determines decoding; file suffixes do not.
The caller supplies the decoded-byte budget. This addition does not claim the
old spectral `openBytes`/`readerCatalog` routes or other language bindings expose
these non-spectral outputs.

| Profile | Accepted | Explicit refusals/output policy |
| --- | --- | --- |
| `png8` | PNG signature; 8-bit grayscale or RGB, standard zlib compression, PNG row filters 0–4, noninterlaced | Palette, alpha/transparency, 16-bit, animation, interlace, unknown critical chunks, CRC/truncation/length defects refuse. Output is raw HWC pixels and fixed intensity range [0,255]. No resize, colour management, learned normalization or sample-ID inference. |
| `vcf_gt_dosage` | UTF-8 VCF 4.x with exact sample header IDs, explicit biallelic sequence REF/ALT and diploid GT using 0/1 alleles | Multiallelic/symbolic variants, absent GT, unsupported ploidy, ambiguous separators, duplicate samples/variant identity refuse. GT dosage is alternate-allele count 0/1/2; phase is discarded and counted. Partial or complete diploid missing calls retain zero storage and validity=false. No DS fallback or imputation. |

VCF output retains canonical CHROM:POS:REF:ALT keys, original VCF ID fields, exact
sample order, source SHA-256, phase/missing counts and policy warnings. IO owns
joining those sample IDs to target/group/partition metadata. Images acquire their
sample identity through an IO mapping manifest, never filenames guessed by a
decoder. Synthetic CC0 fixtures live in `tests/fixtures/multimodal`; decoder tests
and integration gates must be executed before making a release-support claim.
