# Porting manifest

Source authority: Bennu `main` commit `d0adce00a67446f2883e24029682d54b9809b0d7` (`test: align Windows package help contract (#98) (#99)`). The snapshot was clean and inspected from `D:\bennu`; no path is a shipped dependency.

Topology rediscovered at that commit: **195 named doctest cases**, **59 registered Windows tests**, and **64 statically registered Linux tests** (the Linux-only unreadable-file, structural-allocation-refusal, ABI/package checks account for the host delta). There are 155 normative traceability rows.

## Implementation modules

| Bennu source | Faraweave destination |
| --- | --- |
| `src/application.cpp` | `src/primitive.rs + src/evaluator.rs` |
| `src/error.cpp` | `src/error.rs` |
| `src/host_storage.cpp` | `src/resources.rs` checked admission/injection seam plus fallible `Vec::try_reserve*` storage |
| `src/native_builder.cpp` | `src/native_builder.rs` |
| `src/path_encoding.cpp` | `std::path/OsString in src/main.rs and src/native_builder.rs` |
| `src/primitive.cpp` | `src/primitive.rs` |
| `src/rewrite.cpp` | `src/parser.rs + src/primitive.rs + src/evaluator.rs + src/c_emitter.rs` |
| `src/rewrite_c_runtime.cpp` | `src/c_emitter.rs generated strict-C11 runtime` |
| `src/resources.cpp` | `src/resources.rs` |
| `src/type.cpp` | `src/value.rs Type` |
| `src/value.cpp` | `src/value.rs` |
| `src/cli_output.cpp` | `src/main.rs transactional stdout` |
| `src/main.cpp` | `src/main.rs` |

Headers under `include/bennu/` map to the corresponding public re-exports in `src/lib.rs`; private headers map to their named Rust module. CMake version generation maps to Cargo package metadata and `rust-toolchain.toml`. Windows `.rc`/manifest identity maps to release-time PE verification and Rust linker metadata.

## Named unit cases (195)

Every case below is retained as an executable intent in the table-driven Rust semantic/resource/contract suites. Low-level malformed C++ record tests are adapted to safe Rust constructors plus public invariant/error tests; allocator-host probes map to `try_reserve` and deterministic allocation injection; no case is ignored.

| # | Bennu named case | Faraweave evidence/adaptation |
| ---: | --- | --- |
| 1 | shared application preserves scalar kernel behavior | `tests/parity_contracts.rs` primitive/lifting matrices |
| 2 | typed application executes the lowered implementation without redispatch | `tests/parity_contracts.rs` primitive/lifting matrices |
| 3 | shared application broadcasts a scalar over a vector | `tests/parity_contracts.rs` primitive/lifting matrices |
| 4 | structural iota produces one through its positive scalar bound | `tests/parity_contracts.rs` primitive/lifting matrices |
| 5 | application errors carry deterministic arity type and shape context | `tests/parity_contracts.rs` primitive/lifting matrices |
| 6 | elementwise conformance covers vector positions equal vectors and promotion | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 7 | typed empty lifting preserves promotion shape and result element type | `tests/parity_contracts.rs` primitive/lifting matrices |
| 8 | inc equals and not preserve pointwise kernels and result types | `tests/parity_contracts.rs` primitive/lifting matrices |
| 9 | resource preflight precedes scalar and vector domain execution | `tests/resource_contracts.rs` resource/ownership matrix |
| 10 | lifted domain failure reports the lowest index and no partial vector | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 11 | iota exact structural contract covers typed empties rejected lifting and resources | `tests/parity_contracts.rs` primitive/lifting matrices |
| 12 | small deterministic shapes obey pointwise integer addition | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 13 | errors retain semantic context independently of presentation text | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 14 | native compiler selection preserves precedence and attribution | `tests/parity_contracts.rs::native_compiler_selection_is_explicit_then_environment_then_platform` |
| 15 | native compiler command lines preserve argument boundaries | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 16 | production primitive descriptors are static explicit and valid | `tests/parity_contracts.rs` primitive/lifting matrices |
| 17 | primitive descriptor validation rejects every invalid fixture class | `tests/parity_contracts.rs` primitive/lifting matrices |
| 18 | PARG-001-RESERVED-PRIMITIVE-DESCRIPTOR | `tests/parity_contracts.rs` primitive/lifting matrices |
| 19 | overload selection is Cartesian deterministic and exact-first | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 20 | scalar projection conversion covers every type pair and Int64 bits | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 21 | integer scalar kernels cover every boundary and structured overflow | `tests/parity_contracts.rs` primitive/lifting matrices |
| 22 | Double inc and add kernels match every normative binary64 vector | `tests/parity_contracts.rs` primitive/lifting matrices |
| 23 | equals and not kernels cover Bool Int and Double domains | `tests/parity_contracts.rs` primitive/lifting matrices |
| 24 | scalar kernel dispatch rejects unselected or structural invocation | `tests/parity_contracts.rs` primitive/lifting matrices |
| 25 | checked arithmetic primitives expose the required stable signatures | `tests/parity_contracts.rs` primitive/lifting matrices |
| 26 | checked Int arithmetic kernels cover boundaries without partial arithmetic | `tests/parity_contracts.rs` primitive/lifting matrices |
| 27 | checked Double arithmetic preserves exact binary64 semantics and the host environment | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 28 | malformed profile configurations refuse every admission entry | `tests/resource_contracts.rs` resource/ownership matrix |
| 29 | neighboring valid profile configurations remain operational | `tests/resource_contracts.rs` resource/ownership matrix |
| 30 | vector boundary enforces zero exact and one-past profile limits | `tests/resource_contracts.rs` resource/ownership matrix |
| 31 | each profile limit honors zero one exact and one-past semantics | `tests/resource_contracts.rs` resource/ownership matrix |
| 32 | resource sizing rejects multiplication and cumulative overflow | `tests/resource_contracts.rs` resource/ownership matrix |
| 33 | allocation failure injection is ordinal and transactional | `tests/resource_contracts.rs` resource/ownership matrix |
| 34 | vector copies and generic workspace share the allocation seam | `tests/resource_contracts.rs` resource/ownership matrix |
| 35 | profile refusal precedence is vector then live then work | `tests/resource_contracts.rs` resource/ownership matrix |
| 36 | generic workspace uses live bytes but not vector bytes | `tests/resource_contracts.rs` resource/ownership matrix |
| 37 | work charging is monotonic exact and reset per context | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 38 | vector release refunds live bytes but not work | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 39 | trusted profile omits policy limits but retains mandatory safety | `tests/resource_contracts.rs` resource/ownership matrix |
| 40 | resource token issuance permanently exhausts without reuse | `tests/resource_contracts.rs` resource/ownership matrix |
| 41 | rewrite tokenizer uses generic categories and one-based byte spans | parser/lowering/evaluator unit and integration matrices |
| 42 | rewrite tokenizer enforces canonical and complete numeric literals | parser/lowering/evaluator unit and integration matrices |
| 43 | rewrite tokenizer preserves binary64 boundaries and signed zero | parser/lowering/evaluator unit and integration matrices |
| 44 | rewrite tokenizer distinguishes decimal overflow from underflow | parser/lowering/evaluator unit and integration matrices |
| 45 | rewrite parser builds postorder generic calls and contiguous arenas | parser/lowering/evaluator unit and integration matrices |
| 46 | rewrite parser matches normative flat conformance fixtures | parser/lowering/evaluator unit and integration matrices |
| 47 | rewrite parser retains typed homogeneous vector payloads and spans | parser/lowering/evaluator unit and integration matrices |
| 48 | rewrite parser applies logical-record and line-ending rules | parser/lowering/evaluator unit and integration matrices |
| 49 | rewrite parser rejects normative invalid syntax at exact spans | parser/lowering/evaluator unit and integration matrices |
| 50 | rewrite syntax diagnostics retain exact positions and context | parser/lowering/evaluator unit and integration matrices |
| 51 | rewrite primitive resolution is separate and uses stable metadata | `tests/parity_contracts.rs` primitive/lifting matrices |
| 52 | typed lowering is value independent and retains dynamic shape data | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 53 | PARG-004-PARAMETER-LOWERING-METADATA | `tests/cli_contracts.rs` parameter/runner matrices |
| 54 | PARG-016-REPRESENTABILITY | `tests/cli_contracts.rs` parameter/runner matrices |
| 55 | typed lowering applies whole-program phase precedence | parser/lowering/evaluator unit and integration matrices |
| 56 | SHARED-001 static liveness borrows scalar vector empty-vector and tuple nodes | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 57 | SHARED-002 generated C releases a shared argument only at static last use | `tests/resource_contracts.rs` resource/ownership matrix |
| 58 | SHARED-KINDS scalar and empty-vector sharing reaches both production backends | `tests/resource_contracts.rs` resource/ownership matrix |
| 59 | SHARED-ORDER distinct final uses release in reverse argument order | `tests/resource_contracts.rs` resource/ownership matrix |
| 60 | SHARED-FAILURE production C completes final uses before cleanup | `tests/resource_contracts.rs` resource/ownership matrix |
| 61 | SHARED-TUPLE prepared tuple sharing is evaluator and C equivalent | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 62 | SHARED-ROOT duplicate owned roots are rejected before either backend | `tests/resource_contracts.rs` resource/ownership matrix |
| 63 | SHARED-003 failure cleanup and live-byte boundaries are deterministic | `tests/resource_contracts.rs` resource/ownership matrix |
| 64 | SHARED-004 prepared flat graph runs through the production evaluator | `tests/resource_contracts.rs` resource/ownership matrix |
| 65 | typed lowering checks type errors before static shape errors across roots | parser/lowering/evaluator unit and integration matrices |
| 66 | rewrite parser preserves explicit arity before metadata validation | parser/lowering/evaluator unit and integration matrices |
| 67 | rewrite flat program satisfies all arena and postorder invariants | parser/lowering/evaluator unit and integration matrices |
| 68 | rewrite parser handles deep valid and invalid input iteratively | `tests/parity_contracts.rs::deep_unary_programs_use_iterative_parse_analysis_and_evaluation` (4,000 prefix/bracket levels and exact missing-close span) |
| 69 | rewrite parser is deterministic over a fixed adversarial corpus | parser/lowering/evaluator unit and integration matrices |
| 70 | rewrite evaluator returns formatted scalar roots in source order | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 71 | typed runtime shape checks honor static anchors and first mismatch order | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 72 | rewrite evaluator validates every complete execution profile early | `tests/resource_contracts.rs` resource/ownership matrix |
| 73 | TUP-050 invalid configuration precedes source analysis | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 74 | rewrite evaluator constructs accounted typed vector literals | parser/lowering/evaluator unit and integration matrices |
| 75 | rewrite evaluator applies nested primitives through shared semantics | `tests/parity_contracts.rs` primitive/lifting matrices |
| 76 | rewrite evaluator locates structured runtime diagnostics from spans | parser/lowering/evaluator unit and integration matrices |
| 77 | rewrite evaluator completes static analysis before executing roots | parser/lowering/evaluator unit and integration matrices |
| 78 | rewrite evaluator enforces cumulative work and live-byte lifetimes | parser/lowering/evaluator unit and integration matrices |
| 79 | rewrite evaluator refuses resources before latent scalar domain work | `tests/resource_contracts.rs` resource/ownership matrix |
| 80 | TUP-001-GRAMMAR | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 81 | TUP-050-EVALUATOR-FORMAT-PROFILE | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 82 | TUP-050-FAULT-TRANSACTION | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 83 | TUP-050-DIRECT-PRESERVATION | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 84 | TUP-010-STATIC-SPREAD | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 85 | TUP-011-SPREAD-RUNTIME | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 86 | TUP-013-PROVENANCE | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 87 | rewrite evaluator uses one deterministic allocation seam | `tests/resource_contracts.rs` resource/ownership matrix |
| 88 | rewrite evaluator matches the tracked Section 15 and 16 corpus | parser/lowering/evaluator unit and integration matrices |
| 89 | rewrite evaluation matches direct primitive values and errors | `tests/parity_contracts.rs` primitive/lifting matrices |
| 90 | rewrite evaluator executes deep programs without recursive evaluation | `tests/parity_contracts.rs::deep_unary_programs_use_iterative_parse_analysis_and_evaluation` (4,000 applications, exact work count) |
| 91 | rewrite evaluator clears formatted roots after a formatting failure | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 92 | runner scalar text decoding agrees with typed public evaluation | `tests/cli_contracts.rs` parameter/runner matrices |
| 93 | runner scalar text failures preserve structured argument context | `tests/cli_contracts.rs` parameter/runner matrices |
| 94 | runner result teardown releases vector and nested tuple roots | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 95 | FAN-001-GRAMMAR | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 96 | FAN-003-STATIC-ALL-BRANCHES | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 97 | FAN-005-OPERAND-ONCE | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 98 | FAN-006-TABLE-BEFORE-BRANCH | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 99 | FAN-010-SPREAD-DIRECT | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 100 | FAN-016-STRICT-C-NATIVE | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 101 | FAN-002-PLACEHOLDER | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 102 | FAN-004-STATIC-TYPE-ORDER | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 103 | FAN-007-SEQUENTIAL-FIRST-FAILURE | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 104 | FAN-008-TRANSFER-CLEANUP | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 105 | FAN-008-PREADMITTED-ASSEMBLY-INVARIANTS | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 106 | FAN-008-PREADMITTED-ASSEMBLY-HOST-ALLOCATION-FREE | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 107 | FAN-009-OPERAND-KINDS | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 108 | FAN-011-PROFILE-EVENTS | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 109 | FAN-012-ALLOCATION-ORDINALS | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 110 | FAN-013-PROVENANCE | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 111 | FAN-014-NESTING-BOUNDARY | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 112 | FAN-015-ATOMIC-OUTPUT | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 113 | FAN-017-REGRESSION-PLATFORMS | `tests/parity_contracts.rs` FAN stable-ID matrix |
| 114 | typed scalar construction produces valid direct tagged values | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 115 | vectors keep one untagged typed payload and preserve empty types | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 116 | construction and validation reject invalid homogeneous payloads | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 117 | rank length and projection follow scalar and vector identity | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 118 | canonical scalar and vector formatting is byte exact | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 119 | binary64 formatting round trips boundaries and normalizes spelling | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 120 | public value consumers reject malformed plain records | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 121 | explicit destruction releases owned payload and leaves an empty owner | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 122 | S16-00 matrix enumerates every Level 2 elementwise primitive | `tests/parity_contracts.rs` primitive/lifting matrices |
| 123 | S16-01 scalar arguments produce exact scalar values and result types | `tests/parity_contracts.rs` primitive/lifting matrices |
| 124 | S16-02 every argument position accepts a vector and preserves pointwise order | `tests/parity_contracts.rs` primitive/lifting matrices |
| 125 | S16-03 dyadic primitives accept equal vectors in both argument positions | `tests/parity_contracts.rs` primitive/lifting matrices |
| 126 | S16-04 dyadic primitives reject unequal vectors before execution | `tests/parity_contracts.rs` primitive/lifting matrices |
| 127 | S16-05 singleton vectors remain vectors and never broadcast | `tests/parity_contracts.rs` primitive/lifting matrices |
| 128 | S16-06 typed empty vectors with scalars preserve each primitive result type | `tests/parity_contracts.rs` primitive/lifting matrices |
| 129 | S16-07 equal-typed empty vector pairs preserve dyadic result types | `tests/parity_contracts.rs` primitive/lifting matrices |
| 130 | S16-08 mixed numeric typed empties promote for every numeric dyad in both positions | `tests/parity_contracts.rs` primitive/lifting matrices |
| 131 | S16-09 empty and nonempty vectors mismatch for each dyadic primitive and order | `tests/parity_contracts.rs` primitive/lifting matrices |
| 132 | S16-10 exact overloads win and retain their declared result types | `tests/parity_contracts.rs` primitive/lifting matrices |
| 133 | S16-11 Int-to-Double promotion works in scalar and every vector position | `tests/parity_contracts.rs` primitive/lifting matrices |
| 134 | S16-12 promotion uses the signed-64 precision-loss boundary in scalar and vector positions | `tests/parity_contracts.rs` primitive/lifting matrices |
| 135 | S16-13 every primitive rejects unsupported element-type combinations | `tests/parity_contracts.rs` primitive/lifting matrices |
| 136 | S16-14 equals returns Bool from numeric scalar and vector inputs | `tests/parity_contracts.rs` primitive/lifting matrices |
| 137 | S16-15 every primitive rejects missing and excess arity without execution | `tests/parity_contracts.rs` primitive/lifting matrices |
| 138 | S16-16 type validation precedes shape validation for every dyadic primitive | `tests/parity_contracts.rs` primitive/lifting matrices |
| 139 | S16-17 checked dyad shape validation precedes integer domain validation | `tests/parity_contracts.rs` primitive/lifting matrices |
| 140 | S16-18 resource preflight precedes every checked arithmetic domain check | `tests/parity_contracts.rs` primitive/lifting matrices |
| 141 | S16-19 checked arithmetic reports the lowest deterministic domain-failure index | `tests/parity_contracts.rs` primitive/lifting matrices |
| 142 | S16-20 every primitive invokes zero kernels for an empty result | `tests/parity_contracts.rs` primitive/lifting matrices |
| 143 | S16-21 every applicable preflight failure invokes zero kernels | `tests/parity_contracts.rs` primitive/lifting matrices |
| 144 | S16-22 every primitive is pointwise-consistent across deterministic small shapes and boundaries | `tests/parity_contracts.rs` primitive/lifting matrices |
| 145 | ISSUE54-REGISTRY Boolean and inequality identities are registered | `tests/parity_contracts.rs` primitive/lifting matrices |
| 146 | ISSUE54-REGISTRY covers all nine identities and fifteen signatures | `tests/parity_contracts.rs` primitive/lifting matrices |
| 147 | ISSUE54-BOOLEAN and and or use ordinary pointwise truth tables | `tests/parity_contracts.rs` primitive/lifting matrices |
| 148 | ISSUE54-NOT-EQUALS is the exact complement of selected equals | `tests/parity_contracts.rs` primitive/lifting matrices |
| 149 | ISSUE54-REGISTRY numeric predicates and ordering are registered | `tests/parity_contracts.rs` primitive/lifting matrices |
| 150 | ISSUE54-PARITY handles negative values and Int64 extrema without overflow | `tests/parity_contracts.rs` primitive/lifting matrices |
| 151 | ISSUE54-SIGN handles extrema zeros infinities NaN and raw NaN normalization | `tests/parity_contracts.rs` primitive/lifting matrices |
| 152 | ISSUE54-ORDERING preserves written operands and IEEE unordered cases | `tests/parity_contracts.rs` primitive/lifting matrices |
| 153 | ISSUE54-LIFTING covers the complete shared elementwise matrix | `tests/parity_contracts.rs` primitive/lifting matrices |
| 154 | ISSUE54-VALIDATION covers arity type shape and precedence matrix | `tests/parity_contracts.rs` primitive/lifting matrices |
| 155 | ISSUE54-ERRORS reject Bool ordering conversions arity and shape before work | `tests/parity_contracts.rs` primitive/lifting matrices |
| 156 | ISSUE54-RESOURCES preserve shared preflight charging and cleanup semantics | `tests/parity_contracts.rs` primitive/lifting matrices |
| 157 | ISSUE54-WORK-CHARGE observes kernels work and transactional results | `tests/parity_contracts.rs` primitive/lifting matrices |
| 158 | CUTOVER-01 public evaluator accepts rewrite syntax and rejects the removed constructor spelling | parser/lowering/evaluator unit and integration matrices |
| 159 | CUTOVER-02 public evaluator exposes exactly the rewrite primitives | `tests/parity_contracts.rs` primitive/lifting matrices |
| 160 | CUTOVER-03 public APIs preserve structured located rewrite errors | parser/lowering/evaluator unit and integration matrices |
| 161 | checked arithmetic evaluator errors preserve every structured overflow field | equivalent Rust unit/contract case; C++ harness mechanics adapted |
| 162 | CUTOVER-04 public source evaluation is transactional | parser/lowering/evaluator unit and integration matrices |
| 163 | CUTOVER-05 public emitter lowers arbitrary typed vector contents | parser/lowering/evaluator unit and integration matrices |
| 164 | CUTOVER-06 trusted local public evaluation omits arbitrary policy caps | parser/lowering/evaluator unit and integration matrices |
| 165 | CUTOVER-07 explicit bounded profile agrees across public evaluator and emitter | `tests/resource_contracts.rs` resource/ownership matrix |
| 166 | PUBLIC-RESOURCE-01 allocation injection reaches evaluation and generated runtime | `tests/resource_contracts.rs` resource/ownership matrix |
| 167 | LOWERING-01 emission defers value-dependent failures to runtime | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 168 | PARG-004-TYPED-API | `tests/cli_contracts.rs` parameter/runner matrices |
| 169 | PARG-009-DYNAMIC-IOTA-SHAPE | `tests/parity_contracts.rs` primitive/lifting matrices |
| 170 | PARG-005-ARGUMENT-ERROR reports exact count context before value inspection | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 171 | PARG-005-ARGUMENT-ERROR | `tests/cli_contracts.rs` parameter/runner matrices |
| 172 | PARG-001-HEADER | `tests/cli_contracts.rs` parameter/runner matrices |
| 173 | PARG-001-NAMES | `tests/cli_contracts.rs` parameter/runner matrices |
| 174 | PARG-002-STATIC-ORDER | `tests/cli_contracts.rs` parameter/runner matrices |
| 175 | PARG-003-SHAPE-ANALYSIS | `tests/cli_contracts.rs` parameter/runner matrices |
| 176 | PARG-008-ZERO-ROOTS | `tests/cli_contracts.rs` parameter/runner matrices |
| 177 | PARG-010-RUNTIME-ORDER | `tests/cli_contracts.rs` parameter/runner matrices |
| 178 | PARG-011-PROFILES | `tests/cli_contracts.rs` parameter/runner matrices |
| 179 | PARG-012-EMIT-C parameter slots are value independent | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 180 | PARG-017-REGRESSION | `tests/cli_contracts.rs` parameter/runner matrices |
| 181 | host arrays reject writes beyond their allocation | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 182 | TUP-002-TYPES | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 183 | TUP-003-VALUES | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 184 | TUP-004-MOVE-CLEANUP | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 185 | TUP-005-FORMAT | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 186 | TUP-006-PROFILE-IDENTITY | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 187 | resource failures retain their admitted profile during configuration changes | `tests/resource_contracts.rs` resource/ownership matrix |
| 188 | TUP-007-TABLE-CHARGE | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 189 | TUP-008-ALLOCATION-ORDINAL | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 190 | TUP-008-REGISTRY-REENTRANCY | safe-Rust adaptation: no forgeable owner-token registry; `tests/resource_contracts.rs::resource_observer_reports_commit_refusal_and_cleanup_order` proves synchronous observable commit/refusal/release order |
| 191 | TUP-009-CONSTRUCTION | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 192 | TUP-HOST-ALLOCATION-FAILURES | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 193 | TUP-012-DIRECT-PRESERVATION | `tests/parity_contracts.rs` value/tuple/profile matrices |
| 194 | TUP-016-DEEP-NESTING | `tests/parity_contracts.rs::deep_structural_values_and_types_format_and_drop_iteratively` (4,096 levels) plus 512-level evaluator/strict-C/native CLI journey |
| 195 | ADJACENT-LOW-LEVEL-REGRESSION | equivalent Rust unit/contract case; C++ harness mechanics adapted |

## Registered contract tests

| Bennu registered test | Faraweave equivalent/adaptation | Hosts |
| --- | --- | --- |
| `unit.doctest` | Rust-native equivalent contract | Linux x64, Windows x64, macOS arm64 where applicable |
| `spec.traceability` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `spec.traceability_bounded_once_negative` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `spec.traceability_tuple_negative` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `architecture.obsolete_surface` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `architecture.doctest_topology` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `workflow.validation_entry_points` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `stdout.unit` | `src/main.rs::output_tests` exact short-write/flush byte positions; Linux `/dev/full` journeys in validation tooling | Linux x64, Windows x64, macOS arm64 for sink unit; `/dev/full` on Linux |
| `cli.help` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.version` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.no_arguments` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.unknown_option` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.unknown_subcommand` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_transcript` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_eof` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_recovers_after_errors` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_ignores_blank_lines` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_rejects_parameter_header` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.repl_rejects_arguments` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_example` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_path_with_spaces` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_crlf_and_blank_lines` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_missing_path` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_extra_argument` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_nonexistent_file` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_directory_path` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_evaluator_errors` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.run_batch_no_partial_output` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.runner_arguments` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.emit_c_example` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.emit_c_empty_program` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.emit_c_differential` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.parameterized_c_artifacts` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.public_path_error_matrix` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `public.api_resource_matrix` | Rust semantic/resource tests plus emitted-C/native journeys | Linux x64, Windows x64, macOS arm64 where applicable |
| `tuple.literal_strict_c_native` | Rust semantic/resource tests plus emitted-C/native journeys | Linux x64, Windows x64, macOS arm64 where applicable |
| `fanout.strict_c_native` | Rust semantic/resource tests plus emitted-C/native journeys | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.emit_c_negative_atomic` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.build_missing_source` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.build_invalid_arguments` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `docs.smoke` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `docs.decision_record_policy` | manifest/spec/documentation policy contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.build_native` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `cli.build_fake_process` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.windows_version_resource` | release workflow/tooling offline state-machine and provenance contracts | Windows x64 |
| `cli.windows_unicode_paths` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Windows x64 |
| `cli.windows_long_paths` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Windows x64 |
| `cli.source_output_alias` | `tests/cli_contracts.rs` and `tools/validation/contracts.py` | Linux x64, Windows x64, macOS arm64 where applicable |
| `workflow.main_contract` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `workflow.main_contract_negative` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `workflow.checkout_contract` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `workflow.checkout_contract_negative` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.workflow_contract` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.version_source_contract` | release workflow/tooling offline state-machine and provenance contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.stable_version_contract` | release workflow/tooling offline state-machine and provenance contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.windows_version_resource_contract` | release workflow/tooling offline state-machine and provenance contracts | Windows x64 |
| `release.windows_long_path_policy_contract` | release workflow/tooling offline state-machine and provenance contracts | Windows x64 |
| `release.provenance` | release workflow/tooling offline state-machine and provenance contracts | Linux x64, Windows x64, macOS arm64 where applicable |
| `release.future_workflow_contract` | checked-in YAML plus `tools/validation/contracts.py` positive/negative mutations | Linux x64, Windows x64, macOS arm64 where applicable |

Linux additionally registers `unit.structural_host_allocation_refusal`, `cli.run_unreadable_file`, Linux package/ELF compatibility contracts, and Unix permission/device fixtures. Their Rust adaptations run on Linux only. macOS runs the portable set and macOS arm64 archive execution.

## Fixtures and traceability

All 21 files from Bennu `tests/fixtures/` are retained byte-for-byte, including
`.bennu` sources, expected `.out` bytes, C++ corpus includes used as provenance
data, and Linux ABI JSON. New public examples use `.faraweave`. The exact
155-row authority is retained as `tests/source-spec-traceability.tsv`;
`tools/validation/contracts.py` checks its structure, snapshot count, and known
specification identities and contains negative missing-row, duplicate-row, and
stale-spec mutations.

## Intentional differences

- Branding/version: all public Bennu/C++/CMake names become Faraweave/Rust/Cargo; version is exactly Cargo `0.1.0`.
- ABI: Rust public enums replace C++ plain-record ABI; semantic fields, ordering, values, spans, and diagnostics are preserved.
- Harness: Cargo tests and Python offline contracts replace doctest/CTest/CMake without dropping named intent.
- Allocation: Rust `try_reserve` is the physical seam; canonical logical byte charges, ordinals, refusal order, and release observations remain explicit.
- Sanitizers: Linux generated-C ASan/UBSan replaces the applicable C++ sanitizer configuration. Stable Rust 1.97.1 does not distribute Miri; that explicit exclusion and the covered platform seams are documented in `doc/validation-ladder.md`.
- Packaging: Rust binaries have no `libstdc++` expectation. Linux checks use the documented glibc/kernel floor and inspect actual ELF dependencies.
- Release: Faraweave v0.1.0 is new; Bennu historical immutable-v0.1.0 URLs/assets and its 0.1.0 exclusion are intentionally absent.
