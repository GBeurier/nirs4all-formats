# Reverse-Engineering Workflow

Reverse engineering is part of the project, not an informal side task.

## Rules

- Keep runtime code clean-room and MIT-compatible.
- Record fixture origin, license and hash before analysis.
- Prefer controlled differences between related files.
- Write observations and false starts in `docs/reviews/` or `docs/formats/`.
- Validate against external readers when they exist.
- Treat extension collisions as a first-class test case.

## Tooling

`tools/reverse-lab` currently provides:

- byte-level diffs;
- overlapping pattern scans;
- a command-line entrypoint for quick notes.

More tools can be added for bit fields, endian sweeps, structured block maps
and numerical payload hypothesis tests.
