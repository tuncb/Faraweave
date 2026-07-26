# Issue #44 — Specify explicit sequential fan-out branch templates

- **Issue:** https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/44
- **File status:** Active append-only decision record

## 2026-07-23 — Define brace-delimited sequential fan-out over one borrowed operand

- **Context:** Level 3 needs one deterministic construct that evaluates an operand once, shares it without a vector or tuple payload copy across ordered computations, and returns unlike results in the structural tuple and profile-v2 model accepted by Issue #43. Anka executors provide a language-design cue, but adjacency-based grouping, bars, implicit placeholders, connected tuples, blocks, and executor scheduling do not provide the explicit branch boundaries, ownership, static validation, cross-backend failure order, or resource events Faraweave requires. Bare identity and tuple-constructor branches would retain or copy a borrow without an accepted aliasing policy. The issue is specification-only and must leave the current parser and every execution backend unchanged.
- **Decision:** Reserve adjacent `fanout[` as a construct and delimit every branch independently with `{...}`. Require one operand, at least one branch, one primitive-call root per branch, and exactly one branch-local `_` placeholder whose ancestor path to the root contains only primitive applications. Reject `_` in tuple or other owned-constructor positions, bare branch results, and lexical nested fan-out. Parse, resolve, and statically validate the complete program and every branch before execution; derive placeholder type from the operand and derive the fan-out type as `Tuple<R0, ..., Rn-1>` without flattening. At runtime evaluate and charge the operand once, admit the existing positive profile-v2 tuple table `branch_count * 16` before branch 0, then execute branches completely and sequentially from left to right. Move each independently owned result into its preassigned slot exactly once. On branch failure, release the completed result prefix in reverse order, then the table, then the operand; on success, release the operand after final transfer and publish the ordinary owned tuple. Preserve ordinary one-level prefix spreading and direct-call tuple preservation at both placeholder and outer-consumer boundaries. Use branch spans as result-element origins, preserve operand element origins through tuple-placeholder spreading, and retain `_` as related forwarding context. Add no work for fan-out mechanics, no new profile or charge kind, and no allocation stream beyond operand events, the existing result-table event, and branch events in source order.
- **Alternatives considered:** Use Anka-style bars or adjacent primitive words; spell the construct `executor` or `fork`; allow zero branches; infer or repeat placeholders; allow bare `_` or tuple-constructor branches; allocate the result table after all branches; retain branch results as temporaries until the end; reevaluate or copy the operand per branch; flatten tuple results; execute branches in parallel or leave scheduling unspecified; mutate v1 or add a fan-out-specific profile event; or allow nested fan-out immediately. Inferred grouping and scheduling make parse, cost, and failure order unstable. `executor` and `fork` imply concurrency that is not promised. Zero branches add a special no-use lifetime for no product need. Repeated, identity, and aggregate uses require alias/copy semantics. Late table admission lets branch work precede a failure that must guard the result container, while delayed transfer adds unnecessary temporary ownership. Copying violates the shared-view goal; flattening destroys branch-result structure. New resource identities duplicate the accepted v2 tuple event. Nesting adds placeholder capture, provenance, ordinal, and cleanup composition beyond the initial capability.
- **Rationale:** Braces make branch boundaries visible without consulting primitive arity and keep bars invalid elsewhere. A single call-only placeholder path proves the shared value cannot escape into the result under the current primitive contract. One preadmitted table and an initialized result prefix give a flat, inspectable transaction: operand once, table once, branch results once, and one exact reverse cleanup path. Sequential source order makes dynamic failure, work, allocation ordinals, live bytes, provenance, and backend behavior deterministic. Reusing profile v2 and ordinary tuple ownership keeps the capability small and coherent rather than introducing a fan-out-only runtime value or allocator policy.
- **Consequences or follow-up:** FARAWEAVE-SPEC-0007 is the normative contract. Issue #53 must implement its `FAN-*` traceability plan across analyzer, evaluator, runner, emitted strict C11, native execution, allocation-fault seams, and Linux/Windows/macOS. Bare identity/pass-through, owned aggregate branches, repeated placeholders, tuple-aware primitives, callable values/user functions, nested fan-out, explicit mixed spreading, and parallel execution each require separate specification. Optimizers may coalesce or elide physical work only while preserving the exact semantic tuple admission, ordinal, charge, failure, origin, branch order, and lifetime observations.
- **Validation/evidence:** FARAWEAVE-SPEC-0007 resolves grammar, spans, placeholder scope, complete static validation, operand/table/branch ordering, ownership and failure cleanup, scalar/vector/tuple behavior, spreading/direct calls, profile-v2 charges and faults, nesting, Anka differences, cross-backend obligations, valid/invalid examples, and 17 exact future test-plan identifiers. Compatibility pointers amend FARAWEAVE-SPEC-0001, 0002, 0004, 0005, and 0006 without changing runtime code. Repository documentation/traceability checks, clean Release and strict builds, full CTest, current-behavior rejection probes, link/structure checks, `git diff --check`, and exact-head pull-request CI provide implementation-stage evidence.
- **Supersedes:** FARAWEAVE-SPEC-0006 sections 6.3, 9.1, 11.2, 11.3, 12, and 13 only where they describe sequential fan-out as future or excluded work; FARAWEAVE-SPEC-0007 now defines that construct while preserving every accepted tuple, spreading, ownership, provenance, and profile-v2 rule.

## 2026-07-25 — Implement fan-out as flat source arenas with preadmitted tuple ownership

- **Context:** Issue #53 must implement the accepted construct without adding
  recursive AST ownership, operand copies, reference counting, or a second
  tuple-table allocation. The existing rewrite pipeline stores postorder nodes
  and contiguous side arenas, while ordinary tuple construction admits its
  table only after all element values already exist.
- **Decision:** Add brace and placeholder token categories plus flat fan-out and
  branch side arenas. A placeholder node stores the fan-out operand node index;
  typed lowering clones only the operand's structural type and runtime call
  preparation redirects the placeholder to the still-owned operand value.
  Admit the existing positive profile-v2 tuple reservation immediately after
  operand completion. A narrow tuple-construction entry point adopts that
  reservation without another semantic admission. Generated C constructs the
  private result table at the same point, transfers each completed branch into
  its fixed slot, and releases the operand after the final transfer. All branch
  resolution and typed selection remain complete before resource creation.
- **Alternatives considered:** Desugar by duplicating the operand into an
  ordinary tuple; allocate the table after branch execution; introduce shared
  runtime values/reference counts; recursively evaluate a fan-out subtree; or
  add a fan-out-specific resource event. Duplication violates evaluate-once and
  borrowing, late allocation changes failure order, shared runtime ownership
  changes the language cost model, recursive evaluation conflicts with flat
  lowering and static releases, and a new resource identity contradicts
  profile v2.
- **Anka difference:** Faraweave retains explicit `fanout[...]` and `{...}` branch
  boundaries, one lexical `_`, strict left-to-right execution, and an ordinary
  owned structural tuple. It imports no Anka bars, connected tuples, blocks,
  implicit grouping, callable values, or executor scheduling.
- **Evidence:** The `FAN-*` cases in `src/rewrite.cpp` cover grammar and spans,
  complete static selection, typed branch order, scalar/vector/empty/tuple
  operands, fixed-slot transfer, incomplete-publication refusal, reverse
  initialized-prefix cleanup, spreading boundaries, profile-v2 accounting,
  every positive allocation ordinal, public-result atomicity, and ordinary
  expression regressions. `tests/fanout_contract.cmake` covers public
  run/emit-c/build static winners, exact stdout, deterministic emitted C,
  strict C11 compilation, native success and prefix failure, same-address
  operand borrows in generated code, allocation counts, resource snapshots,
  and operand/table/branch injected faults in the generated runtime. The
  current public primitive set has no tuple-producing primitive, so a branch
  cannot naturally produce a nested tuple; the internal fixed-slot executable
  fixture verifies nested tuple/vector preservation without claiming a public
  source journey. Exact local Windows validation and any separately executed
  strict, sanitizer, and platform results belong in the Issue #53 handoff;
  this decision record does not claim unrun target matrices.
