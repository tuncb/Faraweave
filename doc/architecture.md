# Architecture

Faraweave has one semantic pipeline:

```text
source bytes
  -> syntax-only parser AST
  -> lexical declaration resolution
  -> whole-program ownership analysis
  -> typed lowering
  -> RawProgram verification
  -> VerifiedProgram
       -> Rust interpreter
       -> canonical FWIR v1 encoder

canonical FWIR v1 bytes
  -> bounded physical decoder
  -> RawProgram verification
  -> VerifiedProgram
       -> the same Rust interpreter
```

The parser AST is syntax-only and is never accepted by an execution backend.
`src/lowering.rs` is the sole owner of overload selection, conversion,
lifting, shape, provenance, and ownership decisions; `src/interpreter.rs` and
the FWIR encoder consume only `VerifiedProgram` and stable selected IDs.
Execution always uses the Rust interpreter.

Semantic FWIR 1.1 application plans extend those selected IDs with registry
owned operand-consumption, result-cardinality, and resource-work metadata.
Physical section 17 records the selected plan for feature-5 artifacts, while
1.0 artifacts reconstruct the same legacy plan from their implementation ID.
Semantic/physical FWIR 1.2 adds feature 8 for explicit connected bindings.
Lowering emits one `ConnectedBinding` after authored template nodes and the
operand, then binding-only whole/element borrow edges into exactly one selected
call; verification rejects escape, cross-call use, cycles, and ambiguous
ownership before the interpreter executes.

Semantic/physical FWIR 1.3 adds feature 9 for immutable source bindings.
Lexical analysis resolves parameters and declarations, rejects invalid
visibility or ownership escapes, and then lowering emits explicit `Binding`,
`BindingBorrow`, and `BindingMove` topology. The verifier independently checks
provenance, scope-safe consumers, one final move, and the binding owner's exact
logical release before execution.

`VerifiedProgram` is an in-process trust boundary, not a serialized ABI.
Callers may build `RawProgram`, but only complete verification can construct a
`VerifiedProgram`; decoding likewise returns nothing until physical checks and
semantic verification both succeed. The public boundaries and stable artifact
policy are summarized in [the FWIR v1 specification](../spec/fwir-v1-encoding.md).

## Product acceptance traceability

| Requirement | Implementation | Executable evidence |
| --- | --- | --- |
| `FWIR-CUTOVER-001` — one typed lowerer | `src/lowering.rs` | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:src/lowering.rs::lowering_materializes_the_only_typed_selection_decisions` |
| `FWIR-CUTOVER-002` — one direct interpreter | `src/interpreter.rs` | `rust:tests/fwir_public_contracts.rs::public_source_and_decoded_artifact_execution_and_resource_traces_match`<br>`rust:tests/parity_contracts.rs::fan_stable_id_matrix` |
| `FWIR-CUTOVER-003` — no alternate execution backend | interpreter-only API and CLI | `python:tools/validation/contracts.py::validate_product_cutover`<br>`command:interpreter-journey` |
| `FWIR-CUTOVER-004` — no AST execution or runtime overload selection | verified-only interpreter signature | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:src/interpreter.rs::every_selected_implementation_executes_by_stable_id` |
| `FWIR-CUTOVER-005` — stable bytes, versions, and producer policy | `spec/fwir-v1-encoding.md` | `rust:tests/fwir_conformance.rs::canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral`<br>`rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact` |
| `FWIR-CUTOVER-006` — complete requirement evidence | semantic map plus physical TSV | `rust:tests/fwir_conformance.rs::traceability_references_complete_executable_evidence_sets`<br>`python:tools/validation/contracts.py::validate_product_cutover` |
| `FWIR-CUTOVER-007` — no migration seam or flag | named lowering entry; no Cargo features | `python:tools/validation/contracts.py::validate_product_cutover` |
| `FWIR-CUTOVER-008` — isolated final QA | `.github/workflows/main.yml` | `command:main-ci-debug-tests`<br>`command:main-ci-release-tests`<br>`command:main-ci-full-contracts`<br>`command:host-package-contract` |
