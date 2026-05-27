# Integration With nirs4all

`nirs4all-formats` is a leaf I/O library. It does not depend on `nirs4all`.

The intended integration is:

1. load files with `nirs4all-formats`;
2. export arrays, axes and targets through Python or R bindings;
3. feed those outputs into `nirs4all` modelling pipelines;
4. preserve provenance so downstream models can report file origin and reader
   version.

`SignalType` and shared I/O types live here and can be re-exported by higher
level projects.
