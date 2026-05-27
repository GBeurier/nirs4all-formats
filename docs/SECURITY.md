# Security Policy

`nirs4all-formats` parses untrusted binary and text files. Readers must fail closed.

## Runtime Rules

- Bound all reads and decompression.
- Reject archive path traversal, absolute paths, symlinks and device files.
- Preserve native values but report NaN/Inf and non-monotonic axes.
- Never execute vendor macros or embedded scripts.
- Treat GPS, operator names, serial numbers and comments as potentially
  sensitive metadata.

## Reporting

Before public releases, report issues directly to the maintainer. After the
GitHub repository is public, use GitHub private vulnerability reporting if
enabled.
