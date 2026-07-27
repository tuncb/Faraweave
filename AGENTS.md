# Product and architecture

- Faraweave is a data-oriented language runtime written in Rust.
- Prefer plain structs, enums, slices, vectors, and module functions.
- Use explicit `Result` for recoverable failure. Never panic, unwrap, or expect
  on user input, filesystem input, process output, allocation refusal, or other
  recoverable conditions.
- Safe Rust is the default. Any future unsafe block must document invariants,
  be narrowly isolated, and have focused tests.
- Do not add a dependency without a brief decision record.


