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

`VerifiedProgram` is an in-process trust boundary, not a serialized ABI.
Callers may build `RawProgram`, but only complete verification can construct a
`VerifiedProgram`; decoding likewise returns nothing until physical checks and
semantic verification both succeed. The public boundaries and stable artifact
policy are summarized in [the FWIR v1 specification](../spec/fwir-v1-encoding.md).

## Product acceptance traceability

| Requirement | Implementation | Executable evidence |
| --- | --- | --- |
| `FWIR-CUTOVER-001` — one typed lowerer | `src/lowering.rs` | `validate_product_cutover`; lowering goldens |
| `FWIR-CUTOVER-002` — one direct interpreter | `src/interpreter.rs` | interpreter parity and stable-ID tests |
| `FWIR-CUTOVER-003` — one C/native generator | `src/c_emitter.rs` | strict C11/native journey; generated-C parity |
| `FWIR-CUTOVER-004` — no AST backend or runtime overload selection | verified-only backend signatures | `validate_product_cutover` forbidden-token audit |
| `FWIR-CUTOVER-005` — stable bytes, versions, and producer policy | `spec/fwir-v1-encoding.md` | canonical corpus and compatibility tests |
| `FWIR-CUTOVER-006` — complete requirement evidence | semantic map plus physical TSV | `traceability_references_complete_executable_evidence_sets` |
| `FWIR-CUTOVER-007` — no migration seam or flag | named lowering entry; no Cargo features | `validate_product_cutover` |
| `FWIR-CUTOVER-008` — isolated final QA | `doc/validation-ladder.md` | three-host Main CI and host package contract |
