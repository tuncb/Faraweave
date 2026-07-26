# Rust rewrite architecture

## 2026-07-26 — Preserve observable arenas with Rust-owned phase records

- **Context:** The imported issue records describe Bennu's C++ plain-record,
  owner-token, and host-array mechanisms. Faraweave must preserve their
  language, failure-order, accounting, and traversal contracts without
  publishing forgeable C++ pointers or requiring manual context teardown.
- **Decision:** Use Rust-owned enums and vectors at the public boundary and
  keep tokenization, parsing, name resolution, value-independent analysis,
  execution, C lowering, and publication as separate phases. Ordinary syntax
  is represented by explicit owned expression records. Normative deep unary
  input is collapsed into an inner-to-outer operation vector, and uniform deep
  tuples/types use compact depth records during parsing and analysis. Their
  evaluation, formatting, accounting, and cleanup are iterative. Resource
  admission remains one checked transaction with the same logical bytes,
  work, zero-based ordinal, precedence, and reverse cleanup; public observer
  callbacks expose committed admission, refusal, and logical release events.
  Rust ownership replaces forgeable C++ registry tokens and explicit context
  destruction, while `try_reserve*` and synthetic allocation ordinals retain
  recoverable allocation boundaries.
- **Consequences:** C++ record-corruption, token-wrap, copied-alias, and manual
  teardown probes become safe-construction and observer-order adaptations.
  They are not claims that Rust exposes the removed unsafe ABI. The source
  snapshot's 4,000-level unary and 4,096-level structural journeys remain
  executable without host recursion. Generated C retains its own flat
  parent-linked values, iterative release/formatting, and strict C11 contract.
  Stable Rust 1.97.1 has no Miri component; Linux generated C runs under
  ASan/UBSan, and the exclusion is recorded in the validation ladder.
- **Evidence:** `tests/parity_contracts.rs` contains the deep unary and
  structural journeys; `tests/resource_contracts.rs` covers admission,
  refusal, ordinal, cleanup, and observer ordering; `tests/cli_contracts.rs`
  covers transactional paths and Windows long paths; and
  `tools/validation/c11_journey.py` covers strict/generated/native parity.
