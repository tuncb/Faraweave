# Issue 9: C and native IR cutover

Public C emission now lowers source to `VerifiedProgram` and invokes the same verified-only generator for zero-parameter and parameterized programs. Generation never evaluates a Faraweave primitive, and selected implementation IDs, conversions, shapes, provenance, and parameter metadata come only from verified IR. The AST emitter and generic generated-runtime dispatch are deleted while native compiler selection and transactional publication remain unchanged.
