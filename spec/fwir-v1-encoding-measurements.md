# FWIR v1 encoding measurements

**Captured:** 2026-07-28 on Windows x64 with Python 3.11.15.

**Reproduce:** `python tools/validation/fwir_v1_measurements.py report`

The harness builds the same format-neutral empty, scalar-true, and
1,000-scalar-root programs in three physical representations. The selected
column is the complete format in
[the normative specification](fwir-v1-encoding.md); canonical JSON uses UTF-8,
sorted object keys, compact separators, and one final newline; the Protocol
Buffers column uses an explicit message per FWIR record and the official
varint/length-delimited [wire rules](https://protobuf.dev/programming-guides/encoding/).
The Protobuf number is actual wire bytes produced by the checked-in model, not
an estimate, but excludes schema and generated/runtime code from the artifact.

| Fixture | Sectioned binary | Canonical JSON | Protobuf wire model |
| --- | ---: | ---: | ---: |
| empty | 64 | 310 | 12 |
| scalar-true | 422 | 840 | 121 |
| 1,000 scalar roots | 76,346 | 245,271 | 55,684 |

The sectioned example decoder in the harness has 141 nonblank source lines and
11 explicit branch/loop AST nodes. That proxy deliberately excludes the
semantic verifier shared by all three choices and excludes library internals
for JSON and Protobuf, so it is not presented as total production LOC.
Fixed-width records let a Rust or strict-C11 decoder validate each
`length / record_size` before allocation and then decode without recursive
syntax or varint loops; JSON requires a recursive text parser plus duplicate
key, exact-integer, binary64-bit, and canonical-spelling checks; Protobuf
requires a wire parser/generated schema plus the same FWIR semantic checks.

## Toolability, dependencies, and cross-language burden

| Choice | Determinism | Human tooling | Production dependency cost | Strict-C11 / other-language burden |
| --- | --- | --- | --- | --- |
| Sectioned binary | Canonical ordering and every byte are specified here | Needs `inspect-ir`; fixed records remain hex-decodable | Zero new crate dependency and no schema compiler | Same small bounds-checked decoder can be implemented directly in Rust and C11 |
| Canonical JSON | Feasible only with an additional canonical profile | Best general-purpose viewing and diffs | Faraweave currently has zero dependencies; robust Rust and C JSON parsing would add libraries or substantial parser code | Exact `u64`, `i64`, and binary64 payloads must be strings because [JCS is constrained to IEEE-754 JSON numbers](https://www.rfc-editor.org/rfc/rfc8785.html#section-3.1) |
| Protocol Buffers | The official wire format does not guarantee serialization order, so FWIR would need an additional canonicalization layer | Excellent generated tooling when schema/compiler versions are available | Requires checked-in schema plus Rust runtime/codegen; local `protoc` was unavailable | Official references list C++ and several managed languages but not strict C; a C binding or custom decoder becomes another compatibility boundary |
| FlatBuffers | Deterministic construction still needs project-specific ordering rules | Strong generated accessors and reflection | Requires schema compiler/runtime; local `flatc` was unavailable, so no size result is claimed | The official C path is the separate [FlatCC project](https://flatbuffers.dev/languages/c/), adding a second toolchain boundary |

The selected sectioned format is not always the smallest—Protobuf wins all
three measured size cases. It is selected because it has bounded,
nonrecursive framing, exact fixed-width values, no generated-code or runtime
dependency, and the lowest symmetric implementation burden for the existing
safe-Rust and generated strict-C11 consumers. Canonical JSON remains the model
for issue #13's non-executable inspection output; neither JSON nor Protobuf
bytes are FWIR program identity.

## Reproducibility and limitations

`cargo tree --depth 1` reported only the root `faraweave` package, so the
zero-dependency baseline is measured rather than assumed. Python package
`google.protobuf` 6.33.5 was locally importable, but the harness deliberately
uses the published wire algorithm so reproduction does not depend on it;
`protoc` and `flatc` were not installed. Runtime throughput and memory were not
used to choose the format because a complete hostile-input decoder does not
exist until issue #12; comparing partial decoders would create misleading
performance evidence.
