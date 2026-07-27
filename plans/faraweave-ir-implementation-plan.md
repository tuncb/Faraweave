# Faraweave Portable Typed IR Implementation Plan

**Status:** Proposed

**Related specifications:**

- [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)
- [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)
- [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Relevant decision record:**
[Issue #45 — value-independent typed lowering](../doc/decisions/issue-45-value-independent-typed-lowering.md)

**Purpose:** Decompose a portable, verified, typed Faraweave IR into
dependency-ordered GitHub issues. The completed work makes the IR the one
semantic input to the direct interpreter and C11 generator and then exposes a
versioned artifact that other systems can validate, inspect, and execute.

## 1. Outcome

The completed pipeline is:

```text
UTF-8 source
  -> tokenizer and parser
  -> name/declaration resolution
  -> whole-program static analysis
  -> typed lowering
  -> verified immutable FWIR
       -> direct Rust interpretation
       -> deterministic strict-C11 generation
       -> native build
       -> canonical serialization
       -> validation and execution by another system
```

The IR, rather than the parser AST, owns the backend-relevant semantic
decisions:

- indexed parameter references;
- complete result types;
- `StaticScalar`, `StaticVector(n)`, or `DynamicVector` cardinality;
- primitive, signature, and selected implementation identity;
- ordered semantic operands and per-operand conversions;
- direct-call preservation and one-level prefix spreading;
- static shape anchors and ordered dynamic shape checks;
- fan-out operand, branch order, borrow, preadmission, and result structure;
- logical ownership and release points needed for resource traceability; and
- complete diagnostic provenance.

Execution policy remains separate. An execution profile, resource limits, and
allocation-failure injection are supplied when an IR module is instantiated or
executed. The IR records required semantic capabilities, such as tuple/profile
v2 support, but does not permanently bake one deployment's arbitrary limits
into the program.

## 2. Current gap

The Rust rewrite currently has a parser AST and a value-independent analysis
pass, but not one authoritative typed executable representation:

- evaluator entry points call `analyze` and discard its result;
- runtime primitive application reconstructs types and selects a signature
  again;
- parameterized C emission recursively walks the parser AST;
- the C emitter independently infers expression result types and known vector
  lengths;
- primitive names are independently mapped to generated-C numeric tags;
- generated C receives a generic primitive/result-type pair and performs
  additional runtime dispatch; and
- zero-parameter C emission evaluates the source during emission and embeds a
  constant result or failure program.

This plan replaces those parallel decisions without changing Faraweave
language semantics, failure precedence, resource events, formatting, output
transactions, native process isolation, or publication guarantees.

## 3. Planning boundaries

This document plans work; it does not define the final public FWIR schema.
Every work package below becomes a GitHub issue before implementation.

The plan covers:

- a normative semantic IR contract;
- stable semantic identities;
- a flat in-memory typed program and verifier;
- parser-AST-to-IR lowering;
- IR-driven direct interpretation;
- IR-driven C11 generation;
- removal of post-lowering semantic redispatch;
- a canonical versioned artifact encoding;
- safe loading and validation of untrusted IR;
- public library and CLI artifact paths;
- inspection tooling, conformance, malformed-artifact, and compatibility
  evidence; and
- final documentation and format-v1 acceptance.

The initial plan does not cover:

- an optimizer, constant folding, dead-code elimination, common-subexpression
  elimination, or node reordering;
- a JIT, LLVM, WebAssembly, GPU, or distributed-execution backend;
- user-defined functions or a general control-flow graph;
- arbitrary SSA node sharing or reference counting;
- third-party authorship of FWIR v1;
- a stable in-process C ABI for IR records;
- source confidentiality, encryption, or artifact signing;
- embedding execution-profile limits into canonical program identity; or
- changing any existing language, resource-profile, diagnostic, formatting, or
  publication contract.

Faraweave is the authoritative FWIR v1 producer. Other systems may validate,
inspect, translate, or execute a conforming artifact. Supporting third-party IR
producers requires a later specification and conformance boundary.

## 4. Design rules shared by every issue

1. The parser AST remains a source representation and never becomes the public
   artifact.
2. Backends accept only a verified typed program, not raw decoder records.
3. All static phases finish before argument decoding, binding, resource
   creation, or execution.
4. Primitive selection, promotion, spreading, and shape-anchor selection happen
   exactly once during lowering.
5. Interpreter and C generation do not look up overloads or infer result types.
6. Flat vectors, ranges, indexes, and stable numeric identities are preferred
   over recursive ownership or host pointers.
7. Every count, byte size, index conversion, and reservation is checked.
8. Fallible storage uses `try_reserve` at allocation seams and returns explicit
   `Result`.
9. Serialized data never contains Rust discriminants, native struct layout,
   `usize`, native endianness, addresses, or borrowed strings.
10. Binary64 constants are represented by exact IEEE-754 bits.
11. Source spans and semantic origins remain IR sidecars; runtime `Value`s do
    not acquire source ownership.
12. Ownership edges and fan-out borrows are explicit enough to preserve
    logical release order and resource-event traces.
13. Unknown mandatory feature bits, opcodes, primitive IDs, implementation IDs,
    or incompatible major versions are rejected before execution.
14. An optimization may be added only in later work with proof that it
    preserves results, diagnostics, dynamic failure order, logical allocations,
    allocation ordinals, live/peak accounting, work, releases, and output
    behavior.

## 5. Milestones

| Milestone | Result | Work packages |
| --- | --- | --- |
| M0 — Accepted contract | The semantic boundary and non-goals are normative. | IR-00 through IR-01 |
| M1 — One interpreter authority | Source lowers once and direct execution consumes only typed IR. | IR-02 through IR-06 |
| M2 — One backend authority | C emission and native build consume the same typed IR. | IR-07 through IR-08 |
| M3 — Portable product | Canonical bytes can be safely published, loaded, inspected, and executed. | IR-09 through IR-12 |
| M4 — Product acceptance | Compatibility, hostile-input, differential, docs, and final QA are complete. | IR-13 through IR-14 |

## 6. Dependency graph

The symbolic IDs below are temporary planning identifiers. Replace them with
GitHub issue numbers after issue creation and update dependency text in the
issues, not in per-issue decision-record filenames.

```text
IR-00 Umbrella
  |
  v
IR-01 Semantic contract
  |
  v
IR-02 Stable identities
  |
  v
IR-03 In-memory IR + verifier
  |
  v
IR-04 Typed lowering
  |
  +-----------------------+
  |                       |
  v                       v
IR-05 IR interpreter    IR-07 IR C generator
  |                       |
  v                       |
IR-06 Eval cutover        |
  |                       |
  +-----------+-----------+
              v
        IR-08 C/native cutover
              |
              v
        IR-09 Encoding decision
              |
              v
        IR-10 Canonical encoder
              |
              v
        IR-11 Decoder + validation
              |
              v
        IR-12 Public API + CLI
              |
              v
        IR-13 Conformance/security
              |
              v
        IR-14 Final acceptance
```

IR-05 and IR-07 may be implemented independently after IR-04, but IR-08 waits
for the direct-evaluation cutover so differential evidence compares two IR
consumers rather than an IR backend with the legacy AST evaluator.

## 7. Issue creation order

Create issues in this order so every dependency can use a real issue number:

1. IR-00
2. IR-01
3. IR-02
4. IR-03
5. IR-04
6. IR-05
7. IR-06
8. IR-07
9. IR-08
10. IR-09
11. IR-10
12. IR-11
13. IR-12
14. IR-13
15. IR-14

After creation, add every child to the IR-00 checklist and replace symbolic
dependency references in issue bodies with `#<number>` links.

## 8. GitHub issue template

Use this structure for every work package:

```markdown
## Summary

<One observable outcome.>

## Why

<Current gap and relevant specification/decision links.>

## Scope

- <Required production work>
- <Required tests and traceability>

## Out of scope

- <Explicitly excluded adjacent work>

## Dependencies

- Blocked by #...
- Parent: #...

## Acceptance criteria

- [ ] <Observable behavior or invariant>
- [ ] <Removal criterion when replacing an old path>
- [ ] <Required decision record is appended>

## Validation

- `cargo test <focused selection>`
- `tier.review`
- <strict/full/QA additions appropriate to risk>

## Handoff evidence

- Exact changed files
- Exact commands and results
- Platform exclusions
- Decision-record path
- Traceability rows added or changed
```

Every material representation, schema, ownership, compatibility, error,
backend, or test-policy decision belongs in that GitHub issue's append-only
record under `doc/decisions/issue-<number>-<stable-slug>.md`.

## 9. IR-00 — Track portable typed IR as a product

**Suggested title:** `Plan portable typed FWIR as the canonical backend input`

### Summary

Accept this implementation plan, create the complete dependency graph, and
track completion without combining implementation into the umbrella.

### Scope

- Review and accept or correct this plan.
- Create IR-01 through IR-14.
- Link the current specifications and Issue #45 decision.
- Record the intended format name, provisional `.fwir` extension, and the rule
  that no public format is frozen before IR-09.
- Add a checklist of all child issues and their accepted dependencies.

### Out of scope

- Production Rust or C changes.
- Choosing exact binary field widths or command spelling.
- Creating a release artifact.

### Acceptance criteria

- [ ] Every work package has one GitHub issue with scope, non-goals,
      dependencies, acceptance criteria, and validation.
- [ ] The dependency graph has no cycle or implementation work hidden in the
      umbrella.
- [ ] Existing language and backend specifications are linked.
- [ ] Public-format compatibility is explicitly deferred to IR-09.

### Validation

- Markdown links and structure.
- Dependency-graph review.
- `git diff --check`.

## 10. IR-01 — Specify the semantic IR boundary

**Suggested title:** `Specify the typed FWIR semantic and trust boundary`

### Summary

Write and accept the normative contract for the in-memory semantic program
before selecting its external byte encoding.

### Scope

- Define module, parameter, type, constant, node, edge, root, provenance,
  feature, ownership, and fan-out semantics.
- Define stable cardinality and conversion classes.
- Define which static decisions lowering must record.
- Define execution-order and logical-release requirements.
- Define separation between canonical program semantics and execution policy.
- Define producer/consumer trust levels and `VerifiedProgram`.
- Define compatibility goals without fixing physical encoding.
- Add requirement-to-test identifiers for every contract section.

### Out of scope

- Exact serialized bytes.
- An optimizer or new execution backend.
- Third-party FWIR production.

### Acceptance criteria

- [ ] A backend can execute every current Faraweave construct without consulting
      the parser AST or primitive overload table.
- [ ] Static and dynamic failure precedence is completely specified.
- [ ] Tuple spreading and fan-out borrow/cleanup semantics are explicit.
- [ ] Required provenance is sufficient for existing structured diagnostics.
- [ ] The contract identifies malformed-IR failures separately from source
      semantic failures.
- [ ] The contract has traceability IDs and an accepted decision record.

### Validation

- Manual mapping to FARAWEAVE-SPEC-0005 through 0007.
- Review every current `ExprKind`, `Type`, `Value`, error context, and resource
  event against the proposed semantic records.
- Markdown checks and `git diff --check`.

## 11. IR-02 — Centralize stable semantic identities

**Suggested title:** `Centralize stable primitive, signature, and implementation IDs`

### Summary

Replace name-based and backend-local semantic identity with one checked
registry usable by lowering, interpretation, generated C, and serialization.

### Scope

- Add explicit stable IDs for primitives, signatures, and scalar kernel
  implementations.
- Centralize names, arities, accepted scalar types, result types, conversions,
  and structural behavior such as `iota`.
- Validate uniqueness, completeness, and ID-to-descriptor consistency.
- Provide checked conversions from source names and serialized numeric IDs.
- Route current analysis through the centralized registry without changing
  evaluation or C behavior yet.

### Out of scope

- The IR node arena.
- Evaluator or C-emitter cutover.
- New primitives.

### Acceptance criteria

- [ ] One production table owns every primitive name and semantic numeric ID.
- [ ] Invalid-registry fixtures cover duplicate, missing, unknown, and
      inconsistent identities.
- [ ] C-emitter-local name/tag mapping is marked for removal by IR-08 and no new
      mapping is introduced.
- [ ] Existing arity, type, promotion, and primitive tests are unchanged or
      strengthened.
- [ ] Stable numeric values and their rationale are recorded in the issue
      decision record.

### Validation

- Focused registry and overload tests.
- Existing evaluator and C differential tests.
- `tier.review`.

## 12. IR-03 — Add the flat in-memory typed program and verifier

**Suggested title:** `Implement the flat TypedProgram model and invariant verifier`

### Summary

Create the plain-data in-memory representation consumed by later lowering and
backends, with construction and verification separated.

### Scope

- Add module metadata, parameter table, type arena, constant pools, node arena,
  operand/origin sidecars, root table, and feature set.
- Add node forms for literals, parameter borrows, vector and tuple construction,
  selected primitive application, prefix-spread preparation, and fan-out.
- Represent indexes and ranges with checked internal newtypes or equivalent
  explicit validation.
- Model ownership, borrows, and logical release points.
- Add `RawProgram`/builder versus `VerifiedProgram` separation.
- Implement nonrecursive validation with deterministic first-invariant failure.
- Use checked sizing and `try_reserve` for every fallible arena.

### Out of scope

- Source lowering.
- Execution.
- Serialization.

### Acceptance criteria

- [ ] `VerifiedProgram` cannot be constructed from unvalidated raw records
      through the public crate API.
- [ ] Verifier fixtures cover every node, edge, range, type, root, ownership,
      provenance, feature, and fan-out invariant.
- [ ] Cycles, forward/non-postorder references, aliases, orphans, invalid
      implementations, and inconsistent result metadata are rejected.
- [ ] Deep valid structures validate iteratively under reduced-stack tests.
- [ ] No host pointer, borrowed source string, native layout, or backend handle
      is part of the semantic records.

### Validation

- Focused IR model and verifier tests.
- Synthetic checked-overflow and allocation-refusal seams.
- Deep tuple, unary-chain, and fan-out fixtures.
- `tier.review`.

## 13. IR-04 — Lower source to verified typed IR

**Suggested title:** `Lower parsed programs once into verified TypedProgram`

### Summary

Replace root-only `TypeInfo` output with complete value-independent typed
lowering while preserving whole-program static winner order.

### Scope

- Lower every parsed root in deterministic left-to-right postorder.
- Resolve parameter slots, primitive/signature/implementation IDs, conversions,
  cardinality, result types, shape anchors/checks, spreading, provenance, and
  ownership metadata.
- Preserve dependency-aware whole-program arity-before-type-before-shape
  precedence.
- Treat `iota` as `DynamicVector` even for literal bounds.
- Substitute fan-out operand types into branch placeholders without values.
- Verify the completed raw program before returning it.
- Expose a temporary internal `compile_source` seam for direct tests.

### Out of scope

- Executing IR nodes.
- Changing public evaluator or CLI routing.
- Serializing IR.

### Acceptance criteria

- [ ] Lowering receives no runtime arguments or execution values.
- [ ] Every current source construct has exact IR golden coverage.
- [ ] Selected primitive implementations and conversions are asserted, not
      inferred from result strings.
- [ ] Static shape anchors and diagnostic origins match current contracts.
- [ ] Cross-root and fan-out static precedence fixtures remain exact.
- [ ] Invalid source never produces a partially verified program.

### Validation

- Focused source-to-IR golden tests.
- Existing parser, parameter, tuple, fan-out, type, and shape contracts.
- Allocation-failure tests at each lowering arena.
- `tier.review`.

## 14. IR-05 — Interpret verified IR directly

**Suggested title:** `Implement direct execution of VerifiedProgram`

### Summary

Add an interpreter that consumes selected IR operations and never performs
overload selection or source-AST traversal.

### Scope

- Bind typed parameter slots after complete IR verification.
- Execute flat nodes in specified order with explicit owned and borrowed values.
- Dispatch selected implementation IDs directly to scalar/vector kernels.
- Apply recorded conversions, lifting, shape checks, tuple construction,
  fan-out sequence, and release points.
- Preserve resource admission, work, fault ordinal, refusal, and reverse cleanup
  behavior.
- Return existing `ProgramResult`, `Value`, `Error`, and resource-usage forms.

### Out of scope

- Public evaluator cutover.
- C generation.
- Serialization or optimization.

### Acceptance criteria

- [ ] The IR interpreter never calls overload/signature selection.
- [ ] It does not consult primitive source names for execution.
- [ ] Result values, structured errors, locations, resource usage, and observer
      event streams match the current evaluator.
- [ ] Parameters and fan-out placeholders are borrowed without semantic clones.
- [ ] Failure cleanup releases the same successfully initialized prefix in the
      same logical order.
- [ ] Deep valid programs execute without host-stack recursion.

### Validation

- Differential legacy-evaluator versus IR-interpreter corpus.
- Resource observer and fault-ordinal matrices.
- Domain, shape, tuple, spreading, fan-out, parameter, and deep-structure tests.
- `tier.review` and Release focused tests.

## 15. IR-06 — Route evaluation and runner paths through IR

**Suggested title:** `Cut direct evaluation, REPL, and runner over to TypedProgram`

### Summary

Make the IR interpreter the only production direct-execution path and remove
runtime semantic redispatch.

### Scope

- Route expression, source, typed-argument, observer, runner, and REPL APIs
  through source compilation plus IR execution.
- Preserve static-before-argument and format-before-publication boundaries.
- Remove or reduce AST evaluator code that is no longer reachable.
- Remove runtime reconstruction of `TypeInfo` and `select_call`.
- Keep formatting and stdout publication as separate phases.

### Out of scope

- C-emitter routing.
- Public IR files.
- Performance optimization.

### Acceptance criteria

- [ ] Every public direct-evaluation surface executes only `VerifiedProgram`.
- [ ] No production runtime path selects a primitive overload.
- [ ] Existing API signatures and CLI output remain compatible unless IR-01
      explicitly accepted a change.
- [ ] Static failures still precede argument inspection.
- [ ] Runner output remains all-or-nothing before device publication.
- [ ] Dead legacy evaluator paths are deleted rather than left dormant.

### Validation

- All evaluator, CLI, resource, parity, and golden-corpus tests.
- Searches proving removal of runtime `select_call` from execution.
- `tier.review` and `tier.full`.

## 16. IR-07 — Generate strict C11 from verified IR

**Suggested title:** `Implement an IR-driven strict-C11 generator`

### Summary

Add a deterministic C generator that consumes the same typed operations as the
IR interpreter, initially behind an internal test seam.

### Scope

- Emit constants, construction, direct selected kernel calls, conversions,
  lifting, recorded shape checks, tuples, fan-out, and cleanup from IR.
- Emit parameter and provenance metadata from IR tables.
- Use stable semantic IDs or direct generated kernel symbols without
  source-name inference.
- Preserve strict floating-point, resource, formatting, diagnostic, and output
  runtime support.
- Retain deterministic bytes and strict C11 portability.

### Out of scope

- Routing public `emit-c` or `build`.
- External FWIR serialization.
- C optimization.

### Acceptance criteria

- [ ] The new generator accepts only `VerifiedProgram`.
- [ ] It does not call `static_expression_type`, `known_vector_length`, or
      emitter-local overload logic.
- [ ] Generated call sites contain the already-selected implementation and
      recorded shape plan.
- [ ] Generated C performs no runtime overload lookup.
- [ ] Generated results, diagnostics, resource behavior, and output transaction
      match the IR interpreter.
- [ ] Repeated generation is byte-identical.

### Validation

- Focused IR-to-C source fixtures.
- Strict C11 compilation and execution.
- Interpreter/new-generator differential corpus.
- Generated-source inspection for forbidden redispatch.
- `tier.review`, `tier.strict`, and applicable sanitizer journey.

## 17. IR-08 — Cut C emission and native build over to IR

**Suggested title:** `Cut emit-c and build over to TypedProgram and remove AST emission`

### Summary

Make the IR-driven generator authoritative for public C and native artifacts and
delete the duplicate semantic paths.

### Scope

- Route `emit_c_source`, `emit-c`, and `build` through source-to-IR lowering and
  the IR C generator.
- Remove AST-recursive C generation, independent static type inference,
  known-length rediscovery, and local primitive tag mapping.
- Remove generated runtime overload/type selection.
- Replace evaluator-backed zero-parameter emission with uniform IR-driven
  generation.
- Preserve native compiler selection, temporary isolation, cleanup, and atomic
  publication.

### Out of scope

- Constant-folding optimization.
- Public `.fwir` files.
- New native targets.

### Acceptance criteria

- [ ] Direct interpretation and C generation start from the same verified IR.
- [ ] No C-emission path executes Faraweave primitives during generation.
- [ ] No source AST is accepted by a production backend after lowering.
- [ ] Parameterized and zero-parameter programs use the same semantic backend
      model.
- [ ] Existing emitted/native success and failure bytes remain conforming.
- [ ] Obsolete AST-emitter and generic-overload code is deleted.

### Validation

- Full evaluator/generated/native differential corpus.
- Search-based removal checks.
- Atomic output and fake/real compiler tests.
- `tier.review`, `tier.full`, `tier.strict`, and `tier.sanitize` where applicable.

## 18. IR-09 — Decide and specify FWIR v1 encoding

**Suggested title:** `Decide the canonical FWIR v1 artifact encoding`

### Summary

Select the external representation only after both production consumers prove
the semantic model.

### Scope

- Compare a simple sectioned binary format, a canonical text format, and any
  credible schema-driven alternative.
- Measure artifact size, decode complexity, deterministic encoding, toolability,
  dependency cost, and cross-language implementation burden.
- Specify magic, major/minor versions, feature flags, fixed-width integer
  encoding, byte order, section directory, strings, constants, tables, unknown
  field behavior, canonical ordering, and trailing-byte rules.
- Specify producer metadata, semantic version identity, optional source digest,
  and what participates in canonical program identity.
- Specify compatibility and rejection rules.
- Decide the command/API spelling provisionally proposed for IR-12.

### Out of scope

- Implementing the codec.
- Signing, encryption, compression, or network transport.
- Accepting third-party-produced artifacts.

### Acceptance criteria

- [ ] The selected format has no dependence on Rust layout or native word size.
- [ ] Exact canonical bytes are defined for representative modules.
- [ ] Major/minor and mandatory/optional feature behavior is unambiguous.
- [ ] Resource-exhaustion and hostile-length handling are specified.
- [ ] Alternatives, measurements, and any dependency decision are recorded.
- [ ] The normative contract is accepted before IR-10 starts.

### Validation

- Hand-encoded examples independently decoded from the specification.
- Cross-check every in-memory IR field for lossless representation.
- Format-size and decode-complexity measurements.
- Markdown, link, and `git diff --check`.

## 19. IR-10 — Implement canonical FWIR encoding

**Suggested title:** `Implement deterministic canonical FWIR v1 encoding`

### Summary

Serialize a `VerifiedProgram` into canonical bytes without exposing an unsafe or
backend-specific layout.

### Scope

- Implement checked size preflight and section construction.
- Encode fixed-width fields, strings, constants, types, nodes, sidecars, roots,
  features, and metadata exactly as specified.
- Use exact binary64 bits and deterministic table ordering.
- Add a writer API and atomic file-publication integration seam.
- Add byte-for-byte golden artifacts.

### Out of scope

- Decoding untrusted bytes.
- CLI exposure.
- Compression or signing.

### Acceptance criteria

- [ ] The same verified program always produces byte-identical output.
- [ ] Encoding does not include paths, timestamps, capacity, addresses, or host
      endianness unless explicitly normative metadata says otherwise.
- [ ] Every size and offset is checked before allocation or conversion.
- [ ] Allocation refusal and output failure return explicit errors without a
      partially published artifact.
- [ ] Golden artifacts cover every opcode and sidecar.

### Validation

- Focused encoder and golden-byte tests.
- Synthetic size/offset overflow and allocation failure.
- Repeatability across debug/Release and supported hosts.
- `tier.review`.

## 20. IR-11 — Decode and verify untrusted FWIR

**Suggested title:** `Implement bounded FWIR v1 decoding and VerifiedProgram loading`

### Summary

Load external bytes through a checked decoder and complete semantic verifier
before returning a backend-consumable program.

### Scope

- Validate header, version, features, sections, offsets, lengths, alignment if
  specified, UTF-8, constant encodings, and canonical ordering.
- Preflight every allocation and conversion.
- Reconstruct raw flat records without pointers or borrowed artifact memory.
- Run the full IR semantic verifier before producing `VerifiedProgram`.
- Add a distinct structured artifact/IR error model.
- Preserve deterministic first-failure order for malformed artifacts.

### Out of scope

- Executing partially decoded data.
- CLI commands.
- Recovery or normalization of noncanonical artifacts.

### Acceptance criteria

- [ ] No backend can receive a partially decoded or unverified program through
      public APIs.
- [ ] Truncation at every byte boundary, oversized lengths, overlapping
      sections, unknown mandatory features, invalid IDs, graph corruption, and
      trailing bytes are covered.
- [ ] Decoder allocation is bounded by checked artifact claims and configured
      host limits where specified.
- [ ] Decode-encode of canonical artifacts is byte-identical.
- [ ] Malformed IR never causes a panic, unwrap, out-of-bounds access, or source
      semantic error classification.

### Validation

- Table-driven malformed corpus.
- Mutation and truncation tests.
- Deep valid and invalid graphs under reduced stack.
- Debug/Release tests and `tier.review`.

## 21. IR-12 — Expose the FWIR product through APIs and CLI

**Suggested title:** `Expose compile, inspect, run, emit-c, and build paths for FWIR`

### Summary

Publish and consume verified IR through explicit library and command boundaries
without inferring input type from file extension.

### Scope

- Add the accepted public APIs for source-to-IR, encode, decode/verify,
  IR execution, and IR-to-C generation.
- Add explicit CLI commands or flags accepted by IR-09 for:
  - source to `.fwir`;
  - IR inspection;
  - IR execution with arguments;
  - IR to C; and
  - IR to native executable.
- Keep parsing, lowering, encoding, decoding, execution, formatting, process
  launch, and publication visibly separate.
- Publish IR and derived files atomically and reject input/output aliases.
- Provide a stable human-readable inspection form that is not the canonical
  executable encoding.
- Define source-name behavior for diagnostics when only an artifact is present.

### Out of scope

- Automatic source-versus-IR detection.
- Remote transport.
- Third-party IR production certification.

### Acceptance criteria

- [ ] Source and equivalent decoded IR produce identical values, errors,
      resource traces, generated C, and native behavior.
- [ ] Argument count and decoding occur only after IR verification.
- [ ] Existing destination files survive compile, validation, formatting,
      compiler, and publication failures.
- [ ] Help and README examples describe every new boundary.
- [ ] Inspection output is deterministic and exact-value safe.
- [ ] Artifact diagnostics use retained logical source provenance.

### Validation

- CLI contract matrix for success, malformed IR, arguments, aliases, long paths,
  output failure, and destination preservation.
- Library roundtrip and execution tests.
- Strict-C/native journeys from decoded artifacts.
- `tier.review`, `tier.full`, and `tier.strict`.

## 22. IR-13 — Establish compatibility and hostile-artifact conformance

**Suggested title:** `Add FWIR compatibility, mutation, and cross-backend conformance suites`

### Summary

Prove that the published format is deterministic, safely rejected when
malformed, and semantically identical across every consumer.

### Scope

- Add machine-readable traceability from FWIR requirements to tests.
- Create a complete canonical artifact corpus.
- Add mutation, truncation, unknown-version, unknown-feature, oversized-count,
  invalid-graph, invalid-provenance, and invalid-implementation cases.
- Compare source execution, in-memory IR, decoded IR, emitted C, and native
  execution.
- Compare structured errors, exact stdout/stderr, resource events, usage, fault
  ordinals, and cleanup.
- Add compatibility fixtures for accepted same-major minor versions and
  mandatory rejection cases.
- Exercise decoder and generated C under applicable sanitizers.

### Out of scope

- A security claim stronger than the tested parser/validator boundary.
- Network fuzzing or distributed execution.
- Optimizer correctness.

### Acceptance criteria

- [ ] Every FWIR field and invariant has positive and negative evidence.
- [ ] Every supported execution surface consumes the same canonical artifacts.
- [ ] No mutation case panics, hangs, allocates unchecked claimed sizes, or
      reaches a backend before verification.
- [ ] Cross-platform canonical bytes and result/diagnostic parity are proven.
- [ ] Compatibility behavior matches IR-09 exactly.
- [ ] The corpus contains no host-specific paths or timestamps.

### Validation

- Complete malformed/canonical corpus.
- `tier.full`, `tier.strict`, and `tier.sanitize`.
- Windows long-path/publication tests.
- Linux and macOS host-applicable artifact journeys.

## 23. IR-14 — Accept FWIR v1 and remove migration seams

**Suggested title:** `Complete the FWIR v1 product cutover and documentation`

### Summary

Remove temporary adapters, accept the public v1 contract, and finish isolated
quality assurance.

### Scope

- Delete dormant AST execution/emission adapters and provisional codec seams.
- Confirm only verified IR reaches interpreter and C backends.
- Update architecture, README, public API documentation, examples, and
  traceability.
- Record the stable v1 compatibility commitment and producer policy.
- Document unsupported features and explicit security/non-confidentiality
  boundaries.
- Run isolated final QA and record exact commands and exclusions in
  `doc/validation-ladder.md`.

### Out of scope

- Adding the first optimizer or additional backend.
- Publishing a release unless separately authorized.
- Expanding v1 after acceptance.

### Acceptance criteria

- [ ] One source-to-IR lowerer owns every typed semantic decision.
- [ ] One IR interpreter owns direct execution.
- [ ] One IR-to-C generator owns C/native semantics.
- [ ] No production AST backend or runtime overload selection remains.
- [ ] Canonical FWIR v1 bytes, version behavior, and producer policy are
      documented.
- [ ] Every requirement has traceability and passing evidence.
- [ ] All temporary code, feature flags, and migration-only tests are removed.
- [ ] Final isolated QA passes with platform exclusions recorded accurately.

### Validation

- Formatting and clippy with warnings denied.
- All debug and Release tests.
- Release build.
- Contract tooling.
- Strict C11/native journeys.
- Host-applicable platform tests.
- Final `tier.qa`.
- Search-based proof of legacy-path removal.
- Exact changed-file and documentation review.

## 24. Cross-issue test architecture

Tests stay separated by phase so a failure identifies the broken boundary:

1. **Registry tests:** stable identity and descriptor validity.
2. **IR model tests:** raw record and invariant validation.
3. **Lowering tests:** source to exact typed nodes and provenance.
4. **Interpreter tests:** verified nodes to values/errors/events.
5. **C generator tests:** verified nodes to deterministic strict C11.
6. **Codec tests:** verified nodes to canonical bytes and back.
7. **Malformed artifact tests:** raw bytes to deterministic rejection.
8. **CLI tests:** explicit artifact lifecycle and atomic publication.
9. **Differential tests:** source, memory IR, decoded IR, C, and native parity.
10. **Platform tests:** exact bytes and behavior on supported hosts.

At minimum, the data-driven corpus covers:

- zero roots and zero parameters;
- every scalar type and exact binary64 special value;
- every primitive/signature/implementation combination;
- exact `Int -> Double` conversion boundaries;
- empty, singleton, and unequal/equal vectors;
- dynamic `iota` cardinality;
- direct tuple preservation and one-level prefix spreading;
- empty, nested, heterogeneous, and deep tuples;
- fan-out with one and multiple branches, unlike results, and branch failure;
- every static and dynamic precedence direction;
- every profile limit and allocation-failure ordinal;
- formatting and output-device failure; and
- every public provenance and structured-error field.

## 25. Risk register

### Premature format lock-in

**Risk:** Public bytes are frozen before the semantic model handles every
backend.

**Mitigation:** Keep IR internal through IR-08. IR-09 begins only after both
production consumers pass differential tests.

### Semantic identity drift

**Risk:** Rust, encoded IR, and generated C assign different meanings to a
numeric primitive or implementation ID.

**Mitigation:** IR-02 creates one registry; codec and C generation consume it;
fixtures assert every numeric identity.

### Malformed artifact resource exhaustion

**Risk:** Hostile lengths cause overflow or large allocation before rejection.

**Mitigation:** Decoder section preflight, checked arithmetic, configured host
decode limits if normatively selected, `try_reserve`, mutation tests, and no
backend access before verification.

### Resource-observable optimization

**Risk:** Constant folding, node sharing, or reordered releases change logical
allocation attempts, work, peaks, or failure winners.

**Mitigation:** No optimizer in v1 implementation. Ownership and release order
are represented or deterministically derived by the normative IR contract.

### Provenance loss

**Risk:** Remote execution cannot reproduce source-bound diagnostics.

**Mitigation:** Required source/name/declaration/argument/tuple/fan-out origins
are serialized and verified; runtime values remain provenance-free.

### Duplicate backends during migration

**Risk:** AST and IR paths drift while both remain available.

**Mitigation:** New paths stay internal until focused parity passes, public
cutovers happen in IR-06 and IR-08, and the replaced paths are deleted in those
issues.

### Cross-platform encoding drift

**Risk:** Word size, byte order, float formatting, or path metadata changes
artifact bytes.

**Mitigation:** Fixed-width canonical encoding, exact float bits, no host
metadata, and identical artifact golden checks on supported platforms.

### Scope expansion into a VM platform

**Risk:** The first IR release accumulates optimization, plugin, foreign
producer, and backend framework work.

**Mitigation:** Keep v1 as a typed semantic program for the existing
interpreter/C use cases. Each excluded capability requires a later specification
and issue graph.

## 26. Definition of completion

The portable typed IR program is complete only when:

- IR-00 through IR-14 are accepted and closed;
- source static analysis produces one complete verified typed program before
  argument binding;
- direct interpretation and C generation consume only that program;
- no backend repeats overload selection, promotion selection, spreading
  decisions, result-type inference, or shape-anchor selection;
- no C generation path executes Faraweave primitives;
- canonical FWIR bytes have a documented version and compatibility policy;
- untrusted artifacts are completely decoded and verified before backend use;
- source, in-memory IR, decoded IR, C, and native results/errors/events agree;
- IR publication and derived outputs preserve atomicity and alias protections;
- traceability covers every semantic and encoding requirement;
- strict, sanitizer, complete, platform, and isolated QA tiers pass; and
- exact validation commands and exclusions are recorded.

