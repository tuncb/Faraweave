# Issue #12 — bounded FWIR v1 decoder

The public decoder validates the complete physical artifact and canonical record encoding before fallibly allocating owned arenas, then returns only the result of the existing complete semantic verifier. Caller-supplied artifact, section, record, aggregate-record, and string-byte limits bound hostile claims, while allocation refusal remains distinct from physical and verifier failures. Forward-minor advisory sections and advisory features are skipped only under the v1 compatibility rules and are never retained in `VerifiedProgram`.
