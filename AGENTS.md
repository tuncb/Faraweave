# Product and architecture

- Faraweave is a data-oriented language runtime written in Rust.
- Preserve behavior and traceability against Bennu commit
  `d0adce00a67446f2883e24029682d54b9809b0d7`.
- Keep parsing, typed lowering, execution, resource accounting, formatting,
  C emission, process launch, and publication as visibly separate phases.
- Prefer plain structs, enums, slices, vectors, and module functions.
- Use explicit `Result` for recoverable failure. Never panic, unwrap, or expect
  on user input, filesystem input, process output, allocation refusal, or other
  recoverable conditions.
- Use checked sizing and `try_reserve` at allocation seams. Preserve semantic
  allocation ordinals and logical releases even when Rust manages memory.
- Safe Rust is the default. Any future unsafe block must document invariants,
  be narrowly isolated, and have focused tests.
- Do not add a dependency without a brief decision record.
- The shipped product must never depend on a Bennu checkout or binary.

# Validation ladder

Run focused tests while editing, then the affected areas and `tier.review`.
Before handoff run formatting, clippy with warnings denied, all debug and
Release tests, Release build, contract tooling, strict-C11/native journeys, and
host-applicable platform tests. Final isolated QA uses `tier.qa`. Record exact
commands and exclusions in `doc/validation-ladder.md`.

