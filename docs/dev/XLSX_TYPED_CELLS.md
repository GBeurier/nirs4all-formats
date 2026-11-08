# XLSX physical cells, Rust profile v1

`nirs4all_formats::decode_xlsx_cells(bytes, source_name, limits)` returns
`Result<DecodedWorkbook, XlsxDecodeError>`. This is a separate Rust operation
from the spectral reader registry. Its public constants are re-exported by
Formats-core and the Formats facade:

```rust
XLSX_CELLS_OPERATION = "nirs4all.formats.xlsx-cells/v1"
XLSX_CELLS_VERSION = 1
```

The profile accepts unencrypted OOXML XLSX/XLSM archives. The source name is a
provenance label; callers choose this operation explicitly. XLS, XLSB and ODS,
formula evaluation, macro execution, raw-XLSX core WASM and thin host typed-cell
views are unsupported. Existing registry readers, CLI and bindings advertise
their existing spectral capabilities; this operation adds no host DTO, audit
field, package field or canonical wire change.

## Ownership and carrier

Formats alone decodes OOXML and preserves physical provenance. IO owns sheet
selection, dense table admission, roles, NA policy, columns, storage domains,
keys, identities, metadata, masks and partitions. Consumers must check both
`DecodedWorkbook.operation` and `.version` before mapping its cells. In
particular, the IO facade rejects an unknown operation or version and publicly
re-exports the constants; IO-core remains independent of Formats.

`DecodedWorkbook` contains the operation/version, original `source_name`,
lowercase hexadecimal `source_sha256` over the exact archive bytes, ordered
`sheet_names`, `sheets` and `usage`. Each `DecodedSheet` contains its `name`,
XML-ordered sparse `cells`, and optional `used_extent`. Each `DecodedCell` has
an absolute zero-based `(row, column)` `position`, a typed `value`, an `origin`
and the raw optional `reference` (`r`) and `cell_type` (`t`) attributes.
`origin` is `Literal` or `FormulaCache`; the latter records the presence of an
`f` child even when its cache is Empty. Formula source and evaluation are not
part of this contract.

The value kinds are:

| Kind | Preserved authority |
| --- | --- |
| `IntegerLexeme { token }` | Complete ASCII `[+-]?[0-9]+`, including sign and leading zeros; no i64/i128/u64 narrowing. |
| `Float64 { bits, token }` | Calamine's decoded binary64 bits and the original decimal/exponent spelling; no float reparse. |
| `Text(String)` | Physical text, including whitespace, NA-looking text and numeric-looking strings. |
| `Bool(bool)` | Calamine's Boolean value. |
| `Empty` | Structurally empty or unavailable cache, preserved for alignment and duplicate checks. |
| `Error(String)` | Calamine's error spelling; occupies the used extent even if IO later treats it as missing. |
| `Date { kind, serial_bits, token, epoch_1904, rendered }` | DateTime/TimeDelta kind, raw serial bits and token, `has_1904_epoch`, and current `as_datetime().to_string()` when available. |

Date classification precedes integer spelling. `rendered` is `Option<String>`;
IO uses rendered Text for Some; for None, an integer-shaped token with
`|serial| >= 2^53` uses IntegerLexeme, otherwise raw Float(serial), including
signed zero. `DateTimeIso` and `DurationIso` are Text. No calendar parser
or forced string fallback is introduced.

For example, XML `9007199254740993` is an IntegerLexeme with those exact digits,
distinct from `9007199254740992`. XML `1E+30` remains Float64 with bits
`46293e5939a08cea`; it does not become an integer-domain refusal. The integer
tokens `1000000000000000000000000000000` and
`1000000000000000019884624838656` remain integer tokens regardless of their
relationship to that float. Cross-spelling join results and the unchanged
Python reference literals belong to the IO qualification fixtures.

Physical integer `-0`, Float `-0.0` (bits `8000000000000000`) and Text `"-0"`
retain distinct origins. IO owns header canonicalization, including `007`,
`+007`, `+5`, signed zero and beyond-i128 integer headers. Physical Text `123`
beside IntegerLexeme `18446744073709551615` remains Text; IO's per-row numeric
mask must not relabel the whole column as numeric text.

## Pinned decoding and intentional refusals

Calamine is pinned to `=0.37.0` with `dates`. Re-run the focused fixtures before
changing that dependency. The only lexical reader added here is the Formats
XML overlay, using the existing zip and quick-xml dependencies.

The overlay starts after `Xlsx::new` succeeds, preserving calamine's package
and relationship error precedence. Both XML readers use local element names,
`check_end_names=false`, `trim_text(false)`, `check_comments=false` and
`expand_empty_elements=true`. For every `c` in the first `sheetData`, the
overlay records optional `r` and `t`, presence of an `f` child, and the last
child's raw content only when that child is `v`.

Record n joins calamine `next_cell` n, including Empty results. Counts must
agree. Supplied explicit references must agree with calamine's position;
calamine alone infers missing positions. There is no coordinate-based token
lookup or independent missing-position calculation. Ordered overlay names
must equal `workbook.sheet_names()`.

Two refusals intentionally tighten earlier behavior:

* Every repeated returned coordinate refuses, for numeric, Text and Empty
  cells and every combination of these kinds. Earlier decoding could silently
  keep the last value.
* For missing `t` or `t="n"`, a nonempty final `v` must contain exactly one
  Text event. Entity-only, entity-split, CDATA, comment-split and other fragmented
  content refuse, even if calamine would lose it as Empty or truncate it as
  Text. This includes untyped `a&amp;b`. The overlay never concatenates,
  unescapes or rescues numeric content.

Other physical types (`s`, `b`, `e`, `str`, `d` and inline strings) follow
calamine unchanged. A plain untyped `abc` stays Text; Bool/Error/Text values
need no numeric token. An empty `v` stays Empty. Last-child semantics remain:
`<v>7</v><f>A2</f>` is Empty, while `<f>A2</f><v>7</v>` is cached integer 7.

Worksheet resolution follows the pinned dependency precisely. The
`_rels/.rels` officeDocument Target supplies `xl_path` by cutting at its last
slash and removing a leading slash; no slash means an empty base. Read
`{xl_path}workbook.xml` and `{xl_path}_rels/workbook.xml.rels`. A sheet target
starting with `/` loses that slash; otherwise prepend `xl_path`. ZIP cache
keys use `name.replace('\\', "/").to_ascii_lowercase()`. **Lookup keys only
lowercase the requested path**, then fall back to the exact raw requested path
on a cache miss. Lookup does not normalize backslashes, URI segments or `..`.

`used_extent={height,width}` is anchored at A1 and includes only non-Empty
values; an all-Empty sheet has None. Styled blank Z99, XML dimensions and
empty formula caches do not extend it. Error cells do. Empty sparse entries
remain in `cells` to retain ordinal and duplicate validation. IO keeps its
existing `height × width > archive_decoded_bytes` sparse-sheet refusal.

## Decode-time accounting (binding condition C3)

`XlsxDecodeLimits` has exactly `max_decoded_file_bytes` and
`decoded_allowance`. IO supplies the remaining allowance after its existing
raw/file/total/max_files accounting. IO refuses legacy XLS before decoding
with its existing message.

Before calamine initialization, Formats checks the archive's declared
uncompressed sum with checked arithmetic, guards every shared-string declared
reservation against its part size. Consumed parts stream through ZIP's bounded
reader, which refuses entries larger than their declared uncompressed size;
unrelated archive members are not eagerly expanded. Each expanded shared-string copy is charged
before creating the owned cell text; both byte ceilings apply during decode.
Token allocations are bounded by the already charged archive part bytes.

Success returns `XlsxDecodeUsage { archive_decoded_bytes, shared_copy_bytes }`.
`total_decoded_bytes()` returns their checked sum. IO calls `charge_decoded`
once for that sum, keeps the separate archive value for the sparse guard, and
does not charge raw bytes or the overlay's repeated worksheet read again.

`XlsxDecodeError::Limit { label, maximum }` preserves the caller's limit
mapping. Labels are `decoded file/total bytes` for archive/remaining-allowance
failures, `decoded file bytes` for shared expansion exceeding the file limit,
and `decoded bytes overflow` for checked-arithmetic overflow. Non-limit
`Decode(String)` errors retain the IO context
`spreadsheet read failed for {path}`. The established fragments
`declares more strings than`, `larger than its declared uncompressed size`
and IO's `refusing a sparse sheet` remain meaningful.

Row, column, cell, record and field limits stay in IO's **selected-sheet**
`read_sheet` with their existing messages. Decode must not reject an
unselected sheet based on those limits. IO counts a typed numeric cell as
`max(8, token.len())` bytes: a 100,000-digit `000…7` cannot pass a field
limit as an eight-byte integer. Formats preserves the full token so the
selected-sheet reader can enforce this rule.

## Source fixtures and qualification boundary

`tests/fixtures/xlsx_cells_v1.json` retains the 11 original OOXML shape-probe
archives, seven original numeric-probe archives, their exact XML parts and
raw BEFORE stdout/stderr with SHA-256 evidence. Those outputs describe the
pre-N2 lossy exporter and are not corrected expectations. Separate literal
typed packets derive from the original XML tokens, never from a new decoder
run. `tests/xlsx_cells.rs` checks those packets plus additional synthetic
source literals for numeric kinds, fragments, duplicates, paths, occupancy,
dates, budgets and long tokens. New synthetic cases are identified as such;
they are not recreated historical evidence.

Stage D source changes alone establish no functional acceptance. Compilation,
owner and unchanged IO spreadsheet tests, MSRV/no-default checks, bank/caller
qualification and package validation require the separately authorized frozen
cohort. Qualification uses external configuration/locks patching both Formats
crates; committed locks and versions stay unchanged in Stage D. Version and
R reduced-workspace checks remain `STAGE_P_PENDING`, neither passed nor failed.
Stage P records actual version-carrier diffs and complete R dependency closure;
no WASM Formats dependency edge is added.
