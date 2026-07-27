# Issue #2 — accept the typed FWIR semantic boundary

Accepted: `spec/typed-fwir-semantic-contract.md` is the normative in-memory FWIR contract, and all backends consume only an immutable `VerifiedProgram`. Lowering records types, cardinality, selected identities, conversions, provenance, ownership, and failure order while execution profiles and resource policy remain external. Physical encoding and numeric identity assignments stay deferred to their owning issues, and malformed FWIR remains distinct from source semantic failure.
