# Versioning

Crate and package versions follow SemVer once the project leaves alpha.

The C ABI has its own additive version. Breaking ABI changes require a major
ABI bump and migration notes.

The normalized record schema also carries a schema version so serialized
goldens and cached outputs can be migrated intentionally.
