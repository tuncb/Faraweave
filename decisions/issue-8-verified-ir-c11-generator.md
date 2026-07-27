# Issue 8: Verified IR C generator

The internal C generator accepts only `VerifiedProgram` and emits direct `fw_impl_<implementation_id>` call sites from verified stable identities. Conversions, lifting, dynamic shape checks, parameter metadata, and diagnostic origins come from IR records rather than AST re-analysis. The legacy source generator remains the public path until issue #9, while shared strict-float, resource, formatting, and transactional runtime support is retained.
