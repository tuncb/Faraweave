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
Shorter without losing meaning is better. Treat decisions as historical records not ideas that must be followed.

# Testing

Add tests for each added feature or behavioral change.  While adding tests do not stop at the happy path, make sure we have good coverage of all paths. DO NOT create tests for documentation. Do not damage architecture for testing, architecture is more important.