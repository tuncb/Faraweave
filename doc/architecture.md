# Architecture

Faraweave has one semantic pipeline:

```text
source bytes
  -> parser AST
  -> name resolution and whole-program typed lowering
  -> RawProgram verification
  -> VerifiedProgram
       -> Rust interpreter
       -> strict C11 generator -> native compiler
       -> canonical FWIR v1 encoder

canonical FWIR v1 bytes
  -> bounded physical decoder
  -> RawProgram verification
  -> VerifiedProgram
       -> the same interpreter or strict C11 generator
```

The parser AST is syntax-only and is never accepted by an execution backend.
`src/lowering.rs` is the sole owner of overload selection, conversion,
lifting, shape, provenance, and ownership decisions; `src/interpreter.rs` and
`src/c_emitter.rs` consume only `VerifiedProgram` and stable selected IDs.
Native builds compile the output of that same C generator, so there is no
separate native semantic backend.

Semantic FWIR 1.1 application plans extend those selected IDs with registry
owned operand-consumption, result-cardinality, and resource-work metadata.
Physical section 17 records the selected plan for feature-5 artifacts, while
1.0 artifacts reconstruct the same legacy plan from their implementation ID.

`VerifiedProgram` is an in-process trust boundary, not a serialized ABI.
Callers may build `RawProgram`, but only complete verification can construct a
`VerifiedProgram`; decoding likewise returns nothing until physical checks and
semantic verification both succeed. The public boundaries and stable artifact
policy are summarized in [the FWIR v1 specification](../spec/fwir-v1-encoding.md).

## Product acceptance traceability

| Requirement | Implementation | Executable evidence |
| --- | --- | --- |
| `FWIR-CUTOVER-001` — one typed lowerer | `src/lowering.rs` | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:src/lowering.rs::lowering_materializes_the_only_typed_selection_decisions` |
| `FWIR-CUTOVER-002` — one direct interpreter | `src/interpreter.rs` | `rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential`<br>`rust:tests/parity_contracts.rs::fan_stable_id_matrix` |
| `FWIR-CUTOVER-003` — one C/native generator | `src/c_emitter.rs` | `rust:src/c_emitter.rs::public_generated_c_matches_direct_ir_for_success_and_failure_corpus`<br>`command:strict-c11-journey` |
| `FWIR-CUTOVER-004` — no AST backend or runtime overload selection | verified-only backend signatures | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:src/c_emitter.rs::every_selected_id_emits_a_direct_kernel_symbol_without_type_redispatch` |
| `FWIR-CUTOVER-005` — stable bytes, versions, and producer policy | `spec/fwir-v1-encoding.md` | `rust:tests/fwir_conformance.rs::canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral`<br>`rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact` |
| `FWIR-CUTOVER-006` — complete requirement evidence | semantic map plus physical TSV | `rust:tests/fwir_conformance.rs::traceability_references_complete_executable_evidence_sets`<br>`python:tools/validation/contracts.py::validate_product_cutover` |
| `FWIR-CUTOVER-007` — no migration seam or flag | named lowering entry; no Cargo features | `python:tools/validation/contracts.py::validate_product_cutover` |
| `FWIR-CUTOVER-008` — isolated final QA | `doc/validation-ladder.md` | `command:main-ci-debug-tests`<br>`command:main-ci-release-tests`<br>`command:main-ci-full-contracts`<br>`command:host-package-contract` |
