# Conformance Tests

Reference-reader comparisons live here. GPL readers must be executed through an
isolated subprocess or container boundary.

Individual formats may skip when an optional reference reader is unavailable,
but the suite is a strict gate: a run with zero non-skipped cases fails. Build
and install the Python binding before invoking the suite.
