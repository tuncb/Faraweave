# Faraweave

Faraweave 0.1.0 is a standalone Rust implementation of the data-oriented
language defined by the current Bennu specifications at source commit
`d0adce00a67446f2883e24029682d54b9809b0d7`. Bennu was used only as a
development-time differential oracle; the shipped library, executable, emitted
C, and native artifacts neither invoke nor require it.

The language has rank-0 `Bool`, signed 64-bit `Int`, and IEEE-754 binary64
`Double` scalars; homogeneous rank-1 vectors; and immutable heterogeneous
structural tuples. The public primitives are:

| Group | Primitives |
| --- | --- |
| Checked numeric unary | `inc`, `dec`, `neg`, `abs` |
| Checked numeric dyadic | `add`, `sub`, `mul`, `div` |
| Equality and logic | `equals`, `not_equals`, `not`, `and`, `or` |
| Integer predicates | `odd`, `even` |
| Numeric predicates | `is_positive`, `is_negative` |
| Numeric ordering | `less_than`, `greater_than` |
| Structural constructor | `iota` |
| Container query | `length` |
| Container transform | `sort` |
| Numeric reduction | `sum` |
| Boolean reduction | `all_of` |
| Boolean reduction | `any_of` |
| Boolean reduction | `none_of` |
| Explicit-initializer reduction | `foldl` |

Calls use adjacent brackets (`sub[10 2.5]`) or right-associative prefix syntax
(`inc iota 3`). Vectors use parentheses: `(1 2 3)`, `(false true)`,
`Int()`. Tuples use square brackets: `[1 2.5 true]`. A tuple supplied to a
prefix primitive is spread by one level (`add [1 2]`); an adjacent call
preserves it as one argument (`add[[1 2]]`, an arity error). Explicit sequential
fan-out evaluates its operand once and branches left-to-right:

```faraweave
fanout[iota[3] {inc[_]} {add[_ 10]}]
```

This produces `[(2 3 4) (11 12 13)]`.

Elementwise calls broadcast scalars over vectors and require equal vector
lengths. Singleton vectors stay vectors. Exact overloads win; the only
conversion is `Int` to `Double`. Integer arithmetic is checked and publishes no
partial result on overflow; integer `div` truncates toward zero and reports
division by zero as a structured domain error. Double results canonicalize
NaNs; signed zero, infinities, gradual underflow, IEEE division, and unordered
comparisons are preserved. Canonical output includes visible `.0` for integral
Doubles.

`length` accepts a homogeneous Bool, Int, or Double vector, including a typed
empty or dynamically sized vector, and returns its element count as an Int. It
borrows the existing vector without copying it and charges one semantic work
unit independently of the vector length.

`sort` accepts those same vector types and returns a newly owned ascending
vector without mutating its input. Bool and Int use their ordinary order;
Double uses a total bit-defined order with `-0.0` before `0.0` and canonical
NaN after positive infinity, while semantic work is exactly the input length.

`sum` accepts an Int or Double vector and reduces it left-to-right from typed
zero. Int addition is checked at every element; Double addition uses the
strict binary64 arithmetic contract without reassociation, and the complete
input length is charged as work before reduction.

`all_of` accepts a Bool vector and returns true exactly when every element is
true, including the empty-vector identity. Implementations may stop inspecting
after the first false element, but they charge the complete input length before
inspection so work limits and resource observers cannot reveal its position.

`any_of` accepts a Bool vector and returns true exactly when at least one
element is true, with false as the empty-vector identity. Implementations may
stop inspecting after the first true element only after charging the complete
input length, keeping work limits and resource observers position-independent.

`none_of` accepts a Bool vector and returns true exactly when every element is
false, including the empty vector. It has its own selected identity,
diagnostics, and resource producer while using the same complete-length work
admission before an optional first-true short circuit.

`foldl[@add init vector]` accepts a closed registered binary operation
reference, a scalar initializer, and a homogeneous Bool, Int, or Double vector.
It applies the reducer strictly left-to-right, returns the initializer without
invoking the reducer for an empty vector, and charges the full vector length as
work before the first step. Reducer overloads and the only permitted
initializer promotion (`Int` to `Double`) are fixed during lowering; no backend
performs runtime name lookup.

The accepted [architecture](doc/architecture.md) and
[typed FWIR semantic contract](spec/typed-fwir-semantic-contract.md) define
one verified boundary shared by the interpreter and generated-C/native
backends. [FWIR v1](spec/fwir-v1-encoding.md) is the stable canonical artifact
format for that boundary.

## Build and use

Rust 1.97.1 is pinned in `rust-toolchain.toml`.

```sh
cargo build --release
cargo run -- --version
cargo run -- repl
cargo run -- run examples/rewrite.faraweave
cargo run -- emit-c examples/rewrite.faraweave -o rewrite.c
cargo run -- build examples/rewrite.faraweave -o rewrite
cargo run -- compile-ir examples/rewrite.faraweave -o rewrite.fwir
cargo run -- inspect-ir rewrite.fwir
cargo run -- run-ir rewrite.fwir
cargo run -- emit-c-ir rewrite.fwir -o rewrite-from-ir.c
cargo run -- build-ir rewrite.fwir -o rewrite-from-ir
```

Within the REPL, `.internal` prints a read-only view of the production semantic
registry for overload and lifting diagnosis. Its human-readable text is an
internal troubleshooting aid, not a stable machine-readable format or
compatibility contract.

The REPL command `.history` prints the current process session's retained input
with absolute entry numbers. Every nonempty submitted line is included exactly
after LF or CRLF removal, including space/tab-only lines, failed expressions,
and `.history` itself; the oldest entries are evicted to retain at most 100
entries and 65,536 UTF-8 bytes. History is not persisted, and line navigation
and `.clear` are not provided.

Inside the REPL, the exact case-sensitive `.cls` meta-command clears and homes
interactive Windows consoles or ANSI terminals with a nonempty, non-`dumb`
`TERM`; Windows PTYs that are not native console screen buffers use the ANSI
path. Redirected output remains byte-clean, while unsupported terminals and
terminal failures produce a deterministic diagnostic and leave the session
running.

In the REPL, `.exit` ends the session successfully without evaluating source.
Matching is case-sensitive and exact after removing the line ending and
ignoring surrounding ASCII spaces or tabs; arguments, prefixes, and trailing
source comments do not match the command. End-of-file also ends the session
successfully.

`emit-c` writes deterministic self-contained strict C11. `build` selects the C
compiler in this order: explicit `--cc`, `CC`, then `cc` on Unix or `cl.exe` on
Windows. Compiler values are executable names or paths, not shell fragments.
Both commands reject source/output aliases, prepare privately, clean temporary
files after failure, and replace the destination only at publication.

FWIR commands are explicit: `compile-ir` is the only source-to-artifact
boundary, while `inspect-ir`, `run-ir`, `emit-c-ir`, and `build-ir` accept only
canonical artifacts that fully decode and verify first. `run-ir` accepts
parameters after `--`, and `build-ir` accepts the same optional `--cc
<compiler>` selection as `build`; no command infers source versus FWIR from a
file extension. Inspection text is deterministic and includes exact binary64
bits plus the canonical bytes, but it is not executable FWIR.

The library exposes the same phase boundaries through
`compile_source_to_verified_program`, `compile_source_to_fwir`, `encode_fwir`,
bounded `decode_fwir`, `inspect_fwir`,
`evaluate_verified_program_with_arguments`, `emit_c_from_verified_program`,
and `build_native_from_verified_program`. Named compilation retains a logical
source name inside the artifact so later execution diagnostics do not depend
on the artifact's filesystem path.

FWIR v1 commits to physical formats 1.0 and 1.1, semantic contract 1.1,
`.fwir`, and the documented API and CLI spellings. The canonical
semantic/physical-1.0 corpus remains accepted and round-trips byte-for-byte.
Artifacts that use explicit application plans carry mandatory feature
`5=ApplicationPlans` and physical format 1.1; artifacts that use the
stable operation-reference sidecar carry mandatory feature
`6=OperationReferences` and physical format 1.1; artifacts that use the
backend-native math identities carry mandatory feature
`7=BackendNativeMathV1`. Artifacts without these capabilities need not carry
the corresponding feature.
Unknown class-1 advisory features and explicitly optional, non-identity
forward-minor sections may be skipped. Unknown mandatory semantics,
unsupported semantic minors, and other unsupported current-minor extensions
are rejected before a backend runs. Faraweave is the authoritative producer;
accepted canonical bytes do not become trusted because of `PROD` metadata,
and third-party producer support is not promised.

Artifacts are deterministic but not confidential: they can expose diagnostic
source names, literal values, graph structure, provenance spans, and producer
metadata. Decoding is bounded, checked, and fully verified, but it is not a
sandbox; apply limits to untrusted bytes, do not put secrets in artifacts, and
run generated C or native executables only under an appropriate trust policy.
The complete compatibility, unsupported-feature, security, and identity rules
are normative in the [FWIR v1 specification](spec/fwir-v1-encoding.md).

The parser is extension-agnostic. New examples use `.faraweave`; retained
`.bennu` fixtures prove arbitrary extensions continue to work.

## Program parameters

A leading header declares ordered scalar inputs:

```faraweave
parameters[count Int scale Double enabled Bool]
count
add[scale count]
not[enabled]
```

Run it with an explicit boundary:

```sh
faraweave run program.faraweave -- 3 2.5 true
```

Bool accepts only `true`/`false`; Int uses canonical signed decimal without
`+`, leading zeros, or `-0`; Double requires a decimal point or exponent, or
exactly `inf`, `-inf`, or `nan`. Count errors precede decoding, static source
errors precede binding, and all decoding precedes execution.

## Errors, transactions, and profiles

Library APIs return structured `Error` values with one-based byte locations.
CLI diagnostics retain stable category names and Faraweave-prefixed argument,
formatting, and output records, including exact pending/accepted byte counts
for runner output-device failures. Source evaluation, runner formatting, stdout
publication, C-file publication, and native replacement are all-or-nothing
apart from an unavoidable external output-device failure after publication
begins.

The default `trusted-local-v2` profile has no arbitrary policy caps but retains
checked sizing and complete-allocation guarantees. `bounded-v2` supports
`max_vector_bytes`, `max_tuple_table_bytes`, `max_live_evaluation_bytes`, and
`max_work_units`. The v1 profiles retain scalar/vector compatibility and reject
tuple values before execution. Allocation-failure ordinals are available
through `EvaluationConfiguration`. The
`evaluate_expression_with_observer` and
`evaluate_source_with_arguments_and_observer` APIs expose synchronous,
post-decision admission, refusal, and logical-release events without changing
the accounting stream.

## Validation and platforms

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features --release
cargo build --workspace --all-targets --all-features --release
python tools/validation/contracts.py full
python tools/release/provenance.py --help
```

The supported release targets are Ubuntu 24.04 x64, Windows 2022 x64, and
macOS 15 arm64. Only `faraweave build` needs an external C11 compiler. The
complete focused/review/full/strict/sanitize/QA ladder and host-specific
adaptations are documented in [doc/validation-ladder.md](doc/validation-ladder.md).

Canonical example artifacts are inventoried in
[spec/examples](spec/examples/README.md); their exact byte identities and
executable evidence are checked by the conformance suite.

## Packaging and release verification

Release tooling creates exactly:

- `faraweave-v0.1.0-linux-x64.tar.gz`
- `faraweave-v0.1.0-windows-x64.zip`
- `faraweave-v0.1.0-macos-arm64.tar.gz`

Each archive contains only the target executable and `LICENSE`. The compact,
newline-terminated `release-manifest.json` binds the exact source commit,
annotated `v0.1.0` tag, archive SHA-256, contained executable SHA-256, target,
and version. Production publication refuses existing tags/releases/assets,
re-downloads and compares every remote byte, attests every archive and the
manifest through GitHub OIDC, and publishes as its final mutation.

## Deliberate differences from Anka and Rust adaptations

Anka is inspiration, not a compatibility target. Faraweave keeps explicit
bracket calls, checked Int64 arithmetic, fixed one-level tuple spreading,
sequential brace-delimited fan-out with one `_`, deterministic profile-v2
tuple charges, and `iota` as its sole sequence constructor. It has no implicit
currying, functions, effects, reductions other than `sum`, `all_of`, `any_of`,
`none_of`, and explicit-initializer `foldl`,
multidimensional arrays, or container-wide operations other than `length`,
`sort`, `sum`, `all_of`, `any_of`, `none_of`, and `foldl`.

Rust enums and vectors replace C++ tagged/plain records at the public boundary.
Ordinary syntax remains visibly separated by parse, resolution, analysis, and
execution phases; the normative 4,000-deep unary and 512-deep tuple journeys
use compact iterative chains so parser, analysis, evaluation, formatting, and
cleanup do not depend on the host call stack. Checked allocation seams, logical
ownership, resource charges, and failure ordering remain explicit. Cargo
replaces CMake; Rust drop is not treated as permission to weaken logical
release accounting. Platform-specific C/PE/sanitizer checks are adapted as
recorded in [doc/porting-manifest.md](doc/porting-manifest.md).
