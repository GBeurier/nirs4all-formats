# CLI Contract

The CLI binary is `nirs4all-io`.

Current command:

```bash
nirs4all-io probe path/to/file
```

It prints JSON candidate readers with format, reader, confidence and reason.

Planned commands:

- `inspect`: summarize records without dumping arrays;
- `convert`: write normalized JSON, Arrow or Parquet;
- `validate`: compare against golden output;
- `bench`: run reader-level performance scenarios.
