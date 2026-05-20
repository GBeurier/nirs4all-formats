# Fixture Governance

Fixtures are part of the validation contract.

Every fixture needs:

- source URL or generation script;
- license or redistribution permission;
- SHA-256 hash;
- expected format family;
- notes about PII or redaction;
- reference-reader expectations when available.

Large, private or non-redistributable fixtures stay outside the public
repository. Their local manifests can be used for private conformance runs but
must not be committed.
