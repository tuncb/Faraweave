# Product and architecture

- Prefer plain structs, enums, slices, vectors, and module functions.
- Use explicit `Result` for recoverable failure. Never panic, unwrap, or expect
  on user input, filesystem input, process output, allocation refusal, or other
  recoverable conditions.
- Safe Rust is the default. Any future unsafe block must document invariants,
  be narrowly isolated, and have focused tests.
- Do not add a dependency without a brief decision record.

# Decisions

If you make a decision, document it under decisions folder. Create a markdown file per issue. Be brief, write at most 3-4 sentences per decision.
Shorter without loosing meaning is better.