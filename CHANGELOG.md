# Changelog

## [0.2.0](doc/releases/v0.2.0.md) — 2026-07-30

### Language and runtime

- Added stable unary-predicate vector `filter` ([#86](https://github.com/tuncb/Faraweave/issues/86)).
- Added first-class UTF-8 `String` values, including exact CLI argument and FWIR support ([#87](https://github.com/tuncb/Faraweave/issues/87)).
- Added canonical `format` values and raw `printf` output for scalars, vectors, and tuples ([#89](https://github.com/tuncb/Faraweave/issues/89)).
- Added immutable `let name = expression` bindings with verified ownership rules ([#90](https://github.com/tuncb/Faraweave/issues/90)).
- Accepted ordinary trivia in typed empty vector syntax ([#93](https://github.com/tuncb/Faraweave/issues/93)).
- Added placeholder-free connected completion and bind-once `_`/`_n` connected placeholders, with FWIR 1.2 semantics ([#97](https://github.com/tuncb/Faraweave/issues/97), [#98](https://github.com/tuncb/Faraweave/issues/98)).

### Reliability and maintenance

- Unified recoverable allocation handling and failure reporting for `run` and `run-ir` ([#91](https://github.com/tuncb/Faraweave/issues/91)).
- Exposed wrapped failures through `std::error::Error::source` ([#96](https://github.com/tuncb/Faraweave/issues/96)).
- Removed the C emitter and native compiler path; the verified Rust interpreter is now the sole execution backend ([#99](https://github.com/tuncb/Faraweave/issues/99)). The Windows compiler-path work in [#94](https://github.com/tuncb/Faraweave/issues/94) was consequently superseded.
- Updated CI checkout actions to the approved Node 24 revision and consolidated backend conformance helpers ([#88](https://github.com/tuncb/Faraweave/issues/88), [#95](https://github.com/tuncb/Faraweave/issues/95)).
- Corrected the examples guide’s comment-syntax documentation ([#92](https://github.com/tuncb/Faraweave/issues/92)).

## 0.1.0

- Initial Faraweave release.
