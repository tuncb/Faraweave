# Issue #52 — Borrowed multi-consumer liveness

## 2026-07-25 — Derive ownership release from static uses

- **Context:** The flat typed program already stores call arguments as indices
  into one contiguous node arena, but direct evaluation moved each argument into
  a temporary owner and required its `remaining_uses` count to be exactly one.
  Generated C likewise released every argument after every call. Those rules
  made a repeated argument index invalid even though primitive application is
  immutable and does not retain its arguments.
- **Decision:** Lowering records one deterministic use count on every node,
  including tuple-element ownership edges and root retention, while keeping the
  existing contiguous node and argument-index arenas. Typed application
  receives a bounded span of immutable
  pointers into the evaluator's owned node-value arena. A consumer attempt
  decrements all of its static argument uses only after shape/application work
  completes; a non-root owner is released when its count reaches zero. Roots
  retain their owner until result transfer or generated-output cleanup.
  Root indices remain unique because the evaluator's result is a vector of
  uniquely owned values; a repeated root would require a copy or shared owner,
  so malformed repeated-root lowering is rejected before either backend. The
  ordinary lowering path performs that check directly over the root arena
  without allocating a node-sized seen-root sidecar. Full node/use/root
  invariant validation is reserved for prepared flat inputs: each lowering
  node must preserve its source-node kind and each call node must exactly
  preserve the source call's checked argument range and count before any
  prepared owner can move. A tuple-element edge is an exclusive ownership
  move: prepared validation rejects a child that is repeated in the same or
  another tuple, appears in any call argument, or is retained as a root.
  Generated C consumes the same counts while emitting code and writes a direct
  `faraweave_release` only at a non-root argument's last use. The identical static
  release list is emitted on shape failure, primitive failure, and success
  paths before reverse global cleanup, so a failed final consumer has the same
  release timing and order as the evaluator. Runtime values contain no
  reference count, borrowed result, or shared owner.
  After Issue #50 supplied the generated-C tuple representation, prepared
  immutable-borrow consumers use that same representation directly: tuple
  construction moves its uniquely owned children once, later consumers borrow
  the complete tuple without copying, and the tuple recursively releases its
  vector children and table immediately after the final non-root borrow.
- **Alternatives considered:** Move or clone one payload per consumer; introduce
  reference-counted values; retain all intermediates to program completion; or
  have the runtime search future calls for aliases. Moving invalidates later
  consumers, cloning changes charges and allocation/failure order, reference
  counting adds mutation and runtime policy, whole-program retention inflates
  live-byte behavior, and runtime searches hide a cost already knowable from
  flat lowering.
- **Rationale:** Static counts make ownership and costs visible, preserve
  primitive const-borrowing, add no payload copy or runtime allocation, and
  apply uniformly to scalars, vectors, empty vectors, and structural tuple
  owners. The same sidecar drives evaluator and C release placement. Ordinary
  duplicate-root detection deliberately remains allocation-free and therefore
  costs `O(root_count^2)` comparisons in the worst case. Root count is the
  number of published program results rather than the node count; this visible
  one-time lowering cost avoids a node-sized allocation, but should be measured
  again if workloads begin publishing very large root sets.
- **Anka difference:** Anka's executor experiments are a language-design cue,
  but Faraweave deliberately does not adopt executor-owned graphs, dynamic alias
  management, mutation, or runtime reference counting. Faraweave's ordered flat
  lowering fixes sharing and last use before execution.
- **Validation/evidence:** `SHARED-001` covers immutable scalar, vector, and
  empty-vector owners, plus a valid prepared tuple producer feeding two
  successful immutable-borrow consumers through the evaluator's fail-fast
  execution loop. It also covers tuple consumer failure, cleanup, and rejection
  of an invalid static use count before ownership moves. `SHARED-002` duplicates
  a flat argument index in an invariant-valid program and drives the complete
  production C translation-unit emitter, proving C release is emitted only
  after its final call with no runtime use counter. `SHARED-ROOT` proves repeated
  owned roots are rejected before evaluator/backend behavior can diverge.
  `SHARED-003` covers the 47/48-byte live boundary, deterministic allocation
  ordinal 2 failure, retained completed-result cleanup, and exact releases.
  `SHARED-004` runs that graph through the production prepared evaluator seam
  and checks successful roots, maximum-live precedence, allocation ordinals,
  first failure, and zero-live cleanup. Prepared mutation probes cover node-kind,
  call-range, first-argument, argument-count, and use-count mismatches, checking
  evaluator and emitter rejection while the prepared owner remains intact.
  Tuple-owner mutation probes additionally reject a child reused by a later
  call, retained as a root, or repeated in the tuple before the prepared vector
  owner moves.
  Generated C from `SHARED-002` compiles under strict C11 and runs natively with
  the same successful output. `SHARED-KINDS` runs invariant-valid shared scalar
  and typed empty-vector nodes through the prepared evaluator and complete C
  emitter and exports both strict-C11/native journeys, completing the same
  production-path evidence already present for nonempty vectors and tuples.
  `SHARED-FAILURE` uses production `inc`, `equals`, `add`, and `iota` behavior
  rather than the synthetic immutable-borrow failure: a later overflow and a
  later dynamic-shape mismatch compare evaluator and native-C release ordinals,
  prove zero live bytes, and prove exactly-once cleanup.
  `SHARED-TUPLE` integrates Issue #50's tuple
  parser, lowering, evaluator, and C runtime with the #52 prepared graph: two
  vector payloads and one tuple table are constructed exactly once, borrowed by
  two ordered consumers, and released after the final borrow. Evaluator and
  strict-C11/native cases cover the 63/64-byte live boundary, allocation
  ordinals 0, 1, and 2, profile-before-injection precedence, a deterministic
  second-consumer failure, retained-root deferral, invalid prepared ownership,
  zero-live cleanup, and reverse child cleanup (second vector, first vector,
  then tuple table).
- **Integrated tuple C result:** Issue #50 is now present on `main`, so there is
  no remaining tuple-emission dependency. Faraweave deliberately keeps the
  immutable-borrow operation in typed lowering rather than adopting Anka-style
  dynamic alias ownership: the emitted C contains neither a reference count nor
  a deep copy, and its release sites remain statically fixed.
