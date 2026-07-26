# FARAWEAVE-SPEC-0001 Implementation Plan

**Status:** Proposed

**Related issue:** [#23](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/23)

**Implements:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Syntax decision:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Scalar-domain decision:** [FARAWEAVE-SPEC-0003](faraweave-spec-0003-scalar-domain-semantics.md)

**Execution-profile decision:** [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md)

**Purpose:** Decompose the scalar-lifting rewrite into dependency-ordered,
issue-sized changes that keep integrated `main` buildable and independently
verifiable.

## 1. Outcome

The completed rewrite provides one coherent implementation of
FARAWEAVE-SPEC-0001 across:

```text
source text
  -> tokenizer and parser
  -> validated flat program
  -> typed evaluator
  -> canonical formatter
  -> emitted C11
  -> native executable
```

The direct evaluator, REPL, file runner, emitted-C executable, and native-build
executable accept and reject the same conforming programs and produce the same
canonical values or diagnostics.

The rewrite deliberately replaces the bootstrap Level 1 language. This plan
does not preserve the `ioata` spelling, hard-coded unary expression model,
integer-only value representation, count-only array emission, or Level 1 source
compatibility after public cutover.

## 2. Planning boundaries

This document plans implementation; it does not itself authorize or implement
runtime changes. Every work package below must become a GitHub issue with
observable acceptance criteria before code work begins.

The plan covers:

- blocking semantic decisions delegated by FARAWEAVE-SPEC-0001;
- typed scalar and vector data;
- structured errors and canonical formatting;
- centralized resource safety;
- primitive metadata, overload selection, scalar kernels, and lifting;
- generic primitive-call parsing and flat IR;
- evaluator integration;
- C11 and native backend integration;
- atomic public cutover; and
- unit, conformance, differential, strict, sanitizer, and cross-platform
  evidence.

The plan does not cover:

- multidimensional arrays or general rank polymorphism;
- user-defined functions or effects;
- reductions, scans, filters, sort, or indexing as shipped primitives;
- release signing, packaging, or platform compatibility policy;
- a new optimization framework; or
- performance claims without comparable measurements.

The initial conforming primitive set is:

```text
inc
add
equals
not
iota
```

`length` in the specification demonstrates a structural signature shape but is
not required for the initial rewrite cutover. `divide` is used only to explain
domain-error semantics and is not in the initial primitive set.

## 3. Repository and workflow constraints

Every implementation issue follows the repository contributor agreement:

1. Create an observable GitHub issue linked to #23 and its prerequisites.
2. Before dispatch, create exactly one dependency-gated pair:

   ```text
   Implement #<number> (coder) -> Verify #<number> (chef)
   ```

3. Start from clean, current local `main` after all product dependencies and the
   shared-main integration gate are satisfied.
4. Create `issue/<number>-<short-slug>` from `main`.
5. Keep the issue change and its tests on that branch.
6. Run focused and required complete validation on the issue branch.
7. Merge the issue branch into local `main`, push integrated `main`, and wait
   for required Main CI.
8. Leave the issue open and record branch, commits, merged Main SHA, exact
   files, commands, results, and CI URLs.
9. Complete the implementation card only after that durable handoff.
10. Let the dependency-gated Chef independently verify merged behavior and
    close the issue only on acceptance.

Faraweave does not use pull requests or GitHub code review. Only one Coder may
integrate through the shared local `main` checkout at a time. Verification must
not overlap a Coder integration in the same checkout.

Every material syntax, semantic, representation, ownership, error, parsing,
backend, test-policy, resource, and benchmark decision must append a section to
that work's issue-owned record under `doc/decisions/`, following
`doc/decisions/README.md`.

## 4. Starting point and transition strategy

At the time of this plan, bootstrap Level 1 has:

- `ValueKind::integer` and `ValueKind::array`;
- one scalar `std::int64_t` field and one `std::vector<std::int64_t>` field;
- hard-coded `inc` and `ioata` tokens and expression kinds;
- one argument index per expression node;
- direct `apply_inc` and `apply_ioata` dispatch;
- integer-only formatting;
- a C emitter that reconstructs every array as `1..count`; and
- a native builder that consumes that emitter.

An all-at-once rewrite would couple values, parser, evaluator, formatter,
emitter, CLI, documentation, and native execution in one issue. That scope is
too large for independent evidence and safe local-main integration.

The implementation therefore uses a **dormant foundation plus atomic cutover**:

1. Generalize internal data and add directly tested rewrite components while
   the public Level 1 journeys remain green.
2. Keep temporary bootstrap adapters only where needed to preserve a buildable
   and testable `main` between issues. These adapters are migration scaffolding,
   not a compatibility commitment.
3. Do not expose a partially implemented rewrite through one CLI command while
   other commands still implement different semantics.
4. Switch evaluator, REPL, runner, C emitter, native builder, examples, and
   public documentation together in the final cutover issue.
5. Delete bootstrap-only code, tokens, tests, fixtures, limits, and adapters in
   that cutover.

No implementation work branches from the Issue #23 documentation branch.
After #23 is accepted and integrated, each work package branches independently
from the then-current `main`.

## 5. Dependency graph

Issue #23 owns this specification and plan. The implementation graph is now
materialized as the following open GitHub issues. Creating an issue records its
scope and dependencies; it does not make the issue ready for dispatch. A Coder
card may start only after every listed prerequisite has been accepted and the
repository workflow gates in Section 6 are satisfied.

| Plan item | GitHub issue | Direct prerequisites |
| --- | --- | --- |
| Parent specification and plan | [#23](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/23) | None |
| WP0 application and literal syntax decision | [#24](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/24) | #23 |
| WP0 scalar-domain semantics decision | [#25](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/25) | #23 |
| WP0 execution-profile decision | [#26](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/26) | #23 |
| WP1 evaluator and test-boundary prerequisite | [#21](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/21) | Existing prerequisite |
| WP2 typed values, errors, and formatting | [#27](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/27) | #21, #23, #24, #25, #26 |
| WP3 centralized resource safety | [#28](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/28) | #21, #23, #26, #27 |
| WP4 primitive registry and scalar kernels | [#29](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/29) | #21, #23, #25, #27, #28 |
| WP5 shared lifting and structural `iota` | [#30](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/30) | #21, #23, #28, #29 |
| WP6 generic syntax and flat IR | [#31](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/31) | #21, #23, #24, #27 |
| WP7 integrated rewrite evaluator | [#32](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/32) | #21, #23, #30, #31 |
| WP8 measured C-backend strategy | [#33](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/33) | #21, #23, #32 |
| WP9 typed C and native execution | [#34](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/34) | #21, #23, #32, #33 |
| WP10 atomic public cutover | [#35](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/35) | #21, #23, #32, #34; all earlier work transitively |

Issue #21 is intentionally a direct prerequisite of every code-bearing rewrite
issue, #27 through #35. This keeps its evaluator/test-boundary resolution
visible even when an issue could also reach #21 transitively through another
work package. WP0 is split into three independently reviewable decision gates;
all three must be accepted before WP2 begins.

```text
WP0 Accept spec and settle blocking semantic decisions
 |
 +-------------------------+
 |                         |
 v                         v
WP1 Resolve Issue #21     WP2 Typed values, errors, and formatting foundation
 |                         ^
 +-------------------------+
                           |
                           v
                    WP3 Central resource boundary
                           |
                           v
                    WP4 Primitive registry and scalar kernels
                           |
                           v
                    WP5 Shared lifting and structural iota
                           |
              +------------+------------+
              |                         |
              v                         v
       WP6 Generic syntax and IR   direct semantic conformance
              |                         |
              +------------+------------+
                           v
                    WP7 Integrated rewrite evaluator
                           |
                           v
                    WP8 C backend strategy evidence
                           |
                           v
                    WP9 Typed C/native backend
                           |
                           v
                    WP10 Atomic public cutover
```

WP2 may begin only after all three WP0 decisions and the WP1 resolution. WP6
depends on the syntax decision in WP0 and the data/diagnostic contracts in WP2.
WP7 requires both the generic IR and the shared primitive application path.
WP10 requires the rewrite evaluator and backend to be independently complete.

## 6. WP0 — Accept the spec and settle blocking decisions

### Goal

Turn FARAWEAVE-SPEC-0001 from a proposed semantic basis into an accepted,
implementable contract, and resolve only the decisions that block the initial
primitive set.

### Required decision issues

Create focused documentation issues for:

1. **Application and literal syntax**
   ([FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md))
   - primitive application delimiters and whitespace;
   - Bool, Int, and Double literal grammar;
   - vector literal grammar, including typed empty vectors;
   - precedence or the explicit absence of precedence; and
   - exact source locations for call and argument diagnostics.
2. **Scalar primitive domains**
   ([FARAWEAVE-SPEC-0003](faraweave-spec-0003-scalar-domain-semantics.md))
   - `inc` and `add` signed overflow behavior;
   - binary64 arithmetic requirements;
   - `equals` behavior for signed zero, infinities, and NaN; and
   - exact `Int -> Double` conversion confirmation.
3. **Initial execution profile**
   ([FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md))
   - whether the trusted CLI ships without an arbitrary memory/work cap;
   - any configured profile names;
   - `ResourceError` presentation; and
   - the reset boundary for any optional work accounting.

### Acceptance criteria

- FARAWEAVE-SPEC-0001 is independently reviewed and either accepted or corrected.
- The five-primitive initial set is explicit.
- Every source example needed by implementation has normative syntax.
- Scalar kernels have complete value-domain and error contracts.
- The shipped CLI profile has a documented choice, including an explicit
  choice to omit optional caps if that is selected.
- No implementation issue relies on an unresolved section 20 decision.
- Every decision is appended to that issue's record under `doc/decisions/`.

### Validation

- Cross-reference every delegated decision in spec section 20.
- Check examples for one unambiguous parse.
- Review binary64 edge cases with exact bit-pattern test vectors.
- Run Markdown link, structure, terminology, and diff-hygiene checks.

## 7. WP1 — Resolve the evaluator/test-boundary dependency

### Goal

Resolve open [Issue #21](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/21)
before changing the same evaluator, public-header, CMake, and unit-test
boundaries.

### Required decision

Choose one path:

- complete #21 first and build the rewrite on its internalized evaluator stages
  and embedded-test topology; or
- explicitly close/supersede #21 and move only the still-required boundary and
  test-topology work into a rewrite foundation issue.

Do not implement #21 and WP2 concurrently in the shared checkout. Do not create
a second test framework or a second permanent public evaluator-stage API.

### Acceptance criteria

- The selected relationship to #21 is recorded on both issues.
- One production/test compilation topology is authoritative.
- The rewrite plan names which evaluator data is internal and which API remains
  cross-translation-unit.
- No duplicated tokenizer, parser, program-validation, or test-runner migration
  remains scheduled.
- Main CI is green at the chosen boundary.

## 8. WP2 — Typed values, structured errors, and formatting foundation

### Goal

Replace the integer/array-only data assumptions with plain typed scalar and
homogeneous vector data, without exposing rewrite syntax yet.

### Scope

- Add `ScalarType` for Bool, Int, and Double.
- Add explicit scalar versus vector container identity.
- Represent scalars with direct tagged fields.
- Represent vectors with one element-type tag and untagged typed payloads.
- Use byte storage for Bool vectors, not `std::vector<bool>`.
- Derive rank-1 shape from active payload length; do not allocate a shape vector.
- Preserve typed empty vectors.
- Add construction, validation, projection, length, and destruction free
  functions with visible ownership.
- Extend errors with optional structured primitive, argument, shape, element,
  resource, and domain context.
- Implement canonical Bool, Int, Double, and vector formatting from spec section
  14 using the scalar decisions from WP0.
- Adapt the bootstrap evaluator temporarily so existing public journeys remain
  green until cutover.

### Non-goals

- No primitive registry or lifting.
- No rewrite parser or source syntax.
- No C emitter support for new values.
- No public compatibility promise for temporary adapters.

### Acceptance criteria

- Scalar and vector data use plain structs and free functions.
- No object-oriented hierarchy, private state, operator overload, default
  argument, or Faraweave-owned exception control flow is introduced.
- Homogeneous payload invariants are validated at construction boundaries.
- Empty Bool, Int, and Double vectors retain distinct element types.
- No element carries a redundant scalar type tag.
- Bool, Int boundaries, binary64 special values, signed zero, and empty/nonempty
  vectors format canonically and deterministically on all supported compilers.
- Existing bootstrap journeys remain green through temporary adapters.

### Validation

- Focused value-construction and invariant tests.
- Byte-for-byte formatting fixtures for every scalar type and vector form.
- Binary64 round-trip tests, including subnormal, largest finite, infinities,
  signed zero, and NaN presentation.
- Strict warnings/conversions/no-exceptions build.
- AddressSanitizer and UndefinedBehaviorSanitizer where supported.
- Complete current CTest and Main CI.

## 9. WP3 — Central resource boundary

### Goal

Implement spec section 12 once for every vector and workspace allocation,
without an `iota`-specific magic cap.

### Scope

- Add a plain `EvaluationResources` or equivalent context.
- Add shared vector-allocation and workspace-reservation free functions.
- Check element-count representation and multiplication/addition overflow before
  allocation.
- Support an injected named execution profile with optional memory/work limits.
- Provide deterministic fault injection for allocation-unavailable tests.
- Return structured `ResourceError` with no partial value.
- Ensure the selected storage/allocation mechanism does not depend on uncaught
  `std::bad_alloc` or another host exception for recoverable control flow.
- Route temporary bootstrap vector creation through the same boundary. If the
  existing public million-element behavior must remain green before cutover,
  express it as a temporary bootstrap profile rather than a primitive constant.

### Non-goals

- No lifting engine.
- No claim that work units equal memory usage.
- No concrete production cap unless WP0 selected one.

### Acceptance criteria

- One boundary owns all vector payload and primitive-workspace allocation.
- `element_count * element_width` and cumulative size arithmetic cannot wrap.
- Injected size overflow, profile refusal, and allocation failure each return
  the corresponding structured reason.
- No scalar kernel runs and no partial value escapes after resource preflight
  fails.
- At least two distinct producers use the boundary in tests.
- Optional logical work charging, if present, uses documented optimizer-stable
  units and reset boundaries.

### Validation

- Boundary tests at `0`, `1`, maximum representable counts, and one-past limits.
- Deterministic allocator-failure injection before and during candidate setup.
- Leak, double-free, and use-after-free sanitizer coverage.
- Strict/no-exceptions build and complete CTest.
- Main CI on all supported platforms.

## 10. WP4 — Primitive registry and scalar kernels

### Goal

Create explicit static primitive metadata, validate it once, select overloads
deterministically, and implement the initial pure scalar kernels.

### Scope

- Add stable `PrimitiveId` values for `inc`, `add`, `equals`, `not`, and `iota`.
- Add static-lifetime container-aware signature and descriptor tables.
- Validate unique IDs/names, signatures, arity, implementation dispatch, and
  equal-cost ambiguity.
- Implement exact-match and minimum-cost `Int -> Double` overload selection.
- Implement scalar projection conversion without vector materialization.
- Implement scalar kernels for `inc`, `add`, `equals`, and `not` with explicit
  success/domain results.
- Keep lifting mode explicit metadata.
- Dispatch by primitive identity and selected signature, not inferred host
  function type.

### Non-goals

- No vector lifting yet.
- No source parser integration.
- `iota` is described by metadata here but structurally applied in WP5.

### Acceptance criteria

- The production descriptor table validates successfully once before evaluation.
- Invalid fixtures cover duplicate ID/name/signature, missing implementation,
  elementwise vector signature, zero arity, and equal-cost ambiguity.
- Exact signatures beat promotable signatures.
- Bool/numeric mixing and `Double -> Int` are rejected.
- Large Int promotion matches IEEE round-to-nearest, ties-to-even test vectors.
- Kernels implement the WP0 scalar contracts without exceptions or hidden
  container behavior.

### Validation

- Table-driven Cartesian coverage of actual and parameter scalar types.
- Promotion-cost and ambiguity fixtures.
- Scalar domain/boundary tests for every kernel.
- Strict/no-exceptions, sanitizer, complete CTest, and Main CI.

## 11. WP5 — Shared lifting and structural iota

### Goal

Implement the complete post-parse primitive application semantics independently
of source grammar.

### Scope

- Add one shared `apply_primitive` free function.
- Enforce observable order: arity, type/signature, shape, resources, execution.
- Compute exact scalar/vector container join.
- Use the first vector to establish expected length.
- Project scalars without materializing broadcast vectors.
- Promote Int values at scalar-kernel projection.
- Allocate homogeneous typed output through WP3.
- Invoke the scalar kernel zero times for empty output.
- Return the lowest failing result index for vector `DomainError`.
- Implement structural `iota` through the same resource boundary, with no fixed
  primitive-specific element threshold.

### Acceptance criteria

- Every applicable case in spec section 16 is represented in a data-driven
  conformance suite.
- Scalar/scalar, scalar/vector, vector/scalar, and equal vector/vector results
  have exact types, shapes, contents, and canonical formatting.
- Singleton vectors never broadcast.
- Typed empty promotion and result-type changes are correct.
- Arity/type/shape/resource failures invoke no scalar kernel.
- Multiple possible domain failures report the lowest zero-based result index.
- `iota[n]` produces `1..n` for positive `n` and typed empty Int for `n <= 0`.
- Large `iota` tests use an injected small profile or sizing seam; tests do not
  allocate dangerous host-sized vectors.

### Validation

- Spec-example golden tests.
- Full scalar/vector conformance matrix for all initial elementwise primitives.
- Kernel invocation counters for empty and preflight-failure cases.
- Fault-injected resource and domain failures.
- Property tests over small shapes and representative values where the selected
  test framework supports deterministic generators.
- Strict/no-exceptions, sanitizer, complete CTest, and Main CI.

## 12. WP6 — Generic application syntax and flat IR

### Goal

Parse the WP0 syntax into one generic, data-oriented flat program without one
expression kind or argument field per primitive.

### Scope

- Tokenize Bool, Int, Double, vector, application, delimiter, and primitive-name
  syntax selected in WP0.
- Parse generic primitive calls with explicit argument count.
- Store call arguments in a contiguous argument-index arena.
- Preserve postorder/earlier-node references and source-ordered roots.
- Carry call and argument source locations needed by structured diagnostics.
- Resolve primitive names to stable IDs without embedding lifting behavior in
  the parser.
- Reject the legacy name in rewrite syntax.
- Parse typed empty vectors according to WP0 rather than guessing element type.
- Avoid unbounded host recursion; use iterative parsing or an explicit,
  documented nesting limit enforced before stack exhaustion.

### Transition boundary

The new tokenizer/parser/IR remains internal and directly tested until WP10.
The old public source path may remain temporarily, but production code must not
permanently maintain two rewrite grammars.

### Acceptance criteria

- One generic call node represents unary, binary, and later higher-arity calls.
- Argument ranges and node references are bounds-safe and forward-evaluable.
- Every normative source example parses to the expected flat data.
- Invalid syntax reports exact one-based line/column locations.
- Deep valid and invalid input cannot exhaust the C++ call stack.
- Primitive metadata, not parser token kinds, determines arity and lifting.

### Validation

- Token and flat-IR fixtures for every literal/container/call form.
- LF, CRLF, blank-line, missing-final-newline, and hostile nesting cases.
- Malformed delimiter, literal, vector homogeneity, and call-arity source cases.
- Fuzz or deterministic adversarial parser corpus where supported.
- Strict/no-exceptions, sanitizer, complete CTest, and Main CI.

## 13. WP7 — Integrated rewrite evaluator

### Goal

Connect the generic program to the primitive engine and prove complete rewrite
semantics through an internal source-to-value path before public cutover.

### Scope

- Evaluate the flat program iteratively in source order.
- Create one evaluation-resource context per complete program.
- Apply primitive descriptors through WP5.
- Preserve batch all-or-nothing result behavior.
- Produce structured located errors and canonical formatted values.
- Manage intermediate ownership and release at documented liveness boundaries.
- Add a temporary internal test entry point if necessary; remove or internalize
  it during WP10.

### Acceptance criteria

- Every required spec example succeeds or fails exactly as specified from source
  text.
- Nested generic calls produce the same values as direct WP5 application.
- Batch evaluation emits no partial successful result before a later error.
- Resource context reset and lifetime match the WP0 profile decision.
- Intermediate values do not leak, alias mutable storage incorrectly, or retain
  payloads beyond the documented evaluation lifetime.
- The current public commands remain coherent until WP10; no command exposes a
  partial rewrite language.

### Validation

- Source-to-value golden corpus derived from spec sections 15 and 16.
- Direct-application versus parsed-evaluation differential tests.
- Error precedence and location tests.
- Batch ordering/no-partial-output tests through the internal harness.
- Strict/no-exceptions, sanitizer, complete CTest, and Main CI.

## 14. WP8 — C backend strategy evidence

### Goal

Choose the generated-C representation using comparable evidence rather than
extending the bootstrap count-only assumption or making an unmeasured
performance claim.

### Candidate strategies

1. **Embed validated result constants**
   - simplest semantic boundary;
   - generated source and compile cost can scale with result element count.
2. **Lower typed flat IR to C loops and runtime value helpers**
   - source size can scale with program size rather than materialized results;
   - requires explicit C-side typed storage, resource errors, and primitive
     execution.
3. **A documented hybrid**
   - constant-fold small values and lower larger operations;
   - requires an exact threshold and equivalent error/formatting behavior.

### Evidence set

Prototype only enough to compare:

- scalar Int, Double, and Bool roots;
- empty typed vectors;
- arbitrary lifted vectors not reconstructible from length;
- nested `iota` plus lifting;
- representative vector lengths such as 1, 1,000, and a safely chosen larger
  case; and
- source rejection and resource failure.

Record:

- emitted source bytes;
- emission time;
- configured C compiler time;
- generated executable bytes;
- execution time for comparable programs;
- peak or otherwise consistently measured memory where available; and
- implementation complexity and duplicated semantic surface.

### Acceptance criteria

- Candidate measurements use the same host, build type, compiler, programs, and
  repeated-run method.
- The selected strategy preserves arbitrary typed vector contents and canonical
  formatting.
- The strategy has an explicit resource/error boundary and does not reconstruct
  vectors from count.
- Alternatives, measurements, rationale, and consequences are appended to that
  issue's record under `doc/decisions/`.
- No discarded prototype becomes an unsupported permanent backend path.

## 15. WP9 — Typed C emitter and native backend

### Goal

Implement the WP8 strategy so emitted C and native output conform to the rewrite
evaluator for all initial types, shapes, promotions, and errors.

### Scope

- Consume the same validated typed program or equivalent semantic IR as WP7.
- Preserve Bool, Int, Double, vector element types, empty-vector types, and
  arbitrary vector contents.
- Emit deterministic self-contained standard C11.
- Implement canonical formatting exactly, including binary64 edge cases.
- Implement or pre-resolve resource and domain failures according to the
  selected backend strategy.
- Preserve validate-before-publish and atomic output replacement.
- Keep generated-program stdout failure observable.
- Keep native compiler selection, direct invocation, temporary isolation, and
  publish-last behavior intact.

### Acceptance criteria

- Invalid rewrite source produces no partial C output and preserves an existing
  destination.
- Repeated emission of the same program is byte-identical.
- Generated C contains no source path, build timestamp, or host-specific data.
- Strict C11 compilation succeeds on all supported platform compilers.
- Direct evaluator, compiled emitted C, and `faraweave build` executable output are
  byte-for-byte equivalent for the full differential corpus.
- Arbitrary lifted vectors are preserved by contents, not inferred from length.
- C and native resource behavior matches the selected named profile contract.
- Existing file/compiler/process safety and atomicity contracts remain green.

### Validation

- Deterministic emitted-source fixtures.
- Strict C compiler warnings-as-errors where supported.
- Differential corpus across every required spec example and error class.
- Empty, singleton, mixed numeric, binary64 special, large safe vector, and
  allocation/profile-failure cases.
- Native fake-process and real-compiler tests.
- Strict C++/no-exceptions, sanitizer, complete CTest, and Main CI.

## 16. WP10 — Atomic public cutover

### Goal

Replace the bootstrap public language and remove temporary migration scaffolding
only after the rewrite evaluator and backend are independently complete.

### Scope

- Route `evaluate_expression`, `evaluate_source`, REPL, runner, emitter, and
  native builder through the rewrite pipeline.
- Expose the WP0 source grammar and initial primitive set.
- Use `iota` as the only sequence-constructor spelling.
- Remove hard-coded primitive token/expression kinds and direct bootstrap
  primitive dispatch.
- Remove bootstrap-only value fields, adapters, allocation constants, parser,
  formatter, C count reconstruction, tests, fixtures, and examples.
- Replace public README/language documentation and documentation smoke journeys.
- Update the canonical example to exercise scalar, vector, promotion, Bool, and
  `iota` behavior.
- Keep immutable v0.1.0 release artifacts and Git history unchanged; no in-tree
  compatibility layer is required.

### Acceptance criteria

- No production source or active test refers to the legacy primitive spelling.
- No production expression tag is dedicated to one primitive.
- No active C backend path reconstructs a vector from length alone.
- All public commands expose one rewrite grammar and semantic implementation.
- Direct, REPL, runner, emitted-C, and native differential tests cover every
  initial primitive and required container/promotion/error category.
- Documentation describes only the shipped rewrite surface and names deliberate
  differences from Anka where relevant.
- Obsolete code and temporary adapters are removed rather than left dormant.
- Complete local validation and integrated Main CI pass on Linux x64, Windows
  x64, and macOS arm64.

### Required validation

1. Clean Release configure and build.
2. Complete CTest with output on failure.
3. Focused spec conformance suite.
4. REPL and file-runner transcript tests.
5. Emitted-C compile/execute differential suite.
6. Native-build real and fake compiler suites.
7. Strict warnings/conversions/Werror/no-exceptions build.
8. AddressSanitizer and UndefinedBehaviorSanitizer where supported.
9. Documentation smoke journey.
10. Searches proving removal of bootstrap-only spelling, tags, limits, and
    count-only emission.
11. `git diff --check` and exact changed-file review.
12. Required integrated Main CI and independent Chef verification.

## 17. Test architecture

### 17.1 Traceability

Maintain a machine-readable or clearly tabulated mapping from normative spec
requirements to automated test identifiers. At minimum:

| Spec sections | Primary work packages | Evidence |
| --- | --- | --- |
| 4 and 14 | WP2 | typed value invariants and canonical formatting fixtures |
| 5 and 7 | WP4 | descriptor validation, overload, and promotion tables |
| 8 through 11 | WP5 and WP7 | shape, projection, lifting, and failure precedence |
| 12 and 13 | WP3, WP5, WP7, and WP9 | resource injection and structured error fields |
| 15 through 17 | WP5, WP7, WP9, and WP10 | examples, conformance matrix, laws, and differential tests |
| 18 | WP2 through WP9 | representation/IR review plus direct layout/ownership evidence |

### 17.2 Test layers

Use distinct layers so failures identify the broken boundary:

1. **Data tests:** value invariants, typed buffers, formatting, error context.
2. **Metadata tests:** primitive descriptor validity and overload selection.
3. **Application tests:** direct scalar/structural/lifted primitive calls.
4. **Parser/IR tests:** source to flat data and exact locations.
5. **Evaluator tests:** flat data to values/errors and ownership.
6. **CLI tests:** REPL, runner, all-or-nothing batch output.
7. **Backend tests:** emitted C and native execution.
8. **Differential tests:** identical programs through every execution surface.
9. **Fault tests:** arithmetic overflow, profile refusal, allocation failure,
   compiler/output failure.
10. **Platform tests:** supported OS/compiler matrix.

### 17.3 Test data

Keep conformance cases data-driven. Include:

- container combinations for every argument position;
- shapes 0, 1, equal nontrivial, and unequal;
- Bool values and rejection against numeric kernels;
- Int minimum, maximum, overflow boundaries, and `2^53` promotion neighbors;
- Double signed zero, subnormal, finite extremes, infinities, and NaN according
  to the accepted scalar contract;
- result element type differing from input types;
- multiple domain failures proving lowest-index selection;
- invalid descriptor fixtures;
- resource fault injection without dangerous real allocations; and
- arbitrary vector contents that expose count-only backend defects.

Tests must not depend on host out-of-memory behavior, scheduler race order, or
locale-sensitive formatting.

## 18. Performance and cost evidence

Correctness and semantic agreement gate the rewrite. Optimization is not a
substitute for conformance.

Before claiming performance improvement or selecting a cost-sensitive backend:

- use Release builds;
- record source revision, host, OS, CPU, compiler, and flags;
- warm up where appropriate;
- run comparable programs and repeated samples;
- report raw measurements and aggregation method;
- separate parse, evaluation, emission, C compilation, and generated execution
  when those costs differ; and
- retain a reproducible benchmark command or test fixture.

Useful baseline dimensions include:

```text
scalar calls per second
elementwise elements per second
empty/singleton overhead
vector allocation bytes
emitted C bytes
C compile time
generated executable bytes
native execution time
```

Do not claim the rewrite is faster than Level 1: they implement different
semantics. Compare like-for-like rewrite revisions or clearly label capability
and workload differences.

## 19. Risk register

### Cross-platform Double formatting

**Risk:** Standard-library formatting differences produce different bytes.

**Mitigation:** Accept one canonical algorithm in WP0, use exact bit-pattern
fixtures, and differential-test every backend on all supported targets.

### Host allocation exceptions

**Risk:** `std::vector` allocation exposes `std::bad_alloc` or terminates instead
of returning `ResourceError`.

**Mitigation:** Resolve the storage/allocation boundary in WP3, use deterministic
failure injection, and pass the strict no-exceptions configuration.

### Duplicate transitional language paths

**Risk:** Bootstrap and rewrite parsers/evaluators drift while both exist.

**Mitigation:** Keep rewrite paths internal, directly tested, short-lived, and
delete bootstrap paths atomically in WP10. Do not expose mixed CLI behavior.

### Backend semantic duplication

**Risk:** Lowered C reimplements overload, promotion, shape, domain, or resource
rules differently.

**Mitigation:** Consume validated typed IR, record the WP8 decision, and require
full evaluator/C/native differential tests.

### Shared-main conflicts

**Risk:** Issue #21 or another evaluator issue changes the same files during
rewrite work.

**Mitigation:** Resolve WP1 first, enforce dependency edges, and serialize Coder
integration through local `main`.

### Oversized cutover

**Risk:** WP10 accumulates unfinished backend or semantic work.

**Mitigation:** WP10 contains routing, deletion, public docs, and final
integration only. WP2 through WP9 must already be independently accepted.

## 20. Definition of rewrite completion

FARAWEAVE-SPEC-0001 implementation is complete only when:

- WP0 through WP10 are represented by accepted, closed GitHub issues;
- every implementation issue completed the required Coder/Chef workflow;
- the normative spec has automated traceability;
- the five initial primitives conform for all required scalar/vector cases;
- the public grammar uses the accepted syntax and `iota` spelling;
- resource failures are centralized, explicit, and transactional;
- direct, CLI, emitted-C, and native execution agree;
- bootstrap-only implementation and migration adapters are removed;
- public documentation and examples describe the rewrite;
- strict, sanitizer, complete, documentation, differential, and supported Main
  CI validation pass; and
- no unresolved issue contains evidence that a normative acceptance criterion
  is unmet.

Release publication is a separate product decision after rewrite acceptance.
