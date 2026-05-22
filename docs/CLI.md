# CLI Contract

The CLI binary is `nirs4all-io`.

Current commands:

```bash
nirs4all-io probe path/to/file
nirs4all-io read-json path/to/file
nirs4all-io read-json --rows 10:20 --cols 30:40 path/to/cube.hdr
nirs4all-io read-json --pixel 10,20 --pixel 11,21 path/to/cube.hdr
nirs4all-io read-json --pixels-file pixels.txt path/to/cube.hdr
nirs4all-io scan path/to/directory
nirs4all-io scan path/to/directory --max-depth 2 --include-unsupported --json
```

`probe` prints JSON candidate readers with format, reader, confidence and
reason.

`read-json` opens the file through the native Rust registry and prints the
normalized `SpectralRecord` array as JSON. For image cubes, `--rows` and
`--cols` accept half-open `START:END` pixel windows; an omitted end such as
`10:` means "to the cube edge". For sparse selections, `--pixel ROW,COL` can
be repeated and `--pixels-file PATH` reads one `ROW,COL` pair per non-empty
non-`#` line; both forms preserve caller order and allow duplicates.
Rectangular and sparse selections cannot be combined in the same call. These
options currently apply to ENVI Standard and ERDAS LAN / AVIRIS cube readers.
This command is currently also the transport used by the Python bridge while
the native extension/C ABI bindings are being filled in.

`scan` recursively walks a directory (or a single file) and prints one line
per visited entry with status `parsed` / `error` / `unsupported`, an end
summary on stderr, and a structured JSON payload when `--json` is set. Hidden
entries and symlinks are skipped by default. The same surface is exposed
natively to Python via `nirs4all_io.walk_path(...)` and to R via
`nirs4allio_walk_path(...)`.

Planned commands:

- `inspect`: summarize records without dumping arrays;
- `convert`: write Arrow or Parquet;
- `validate`: compare against golden output;
- `bench`: run reader-level performance scenarios.
