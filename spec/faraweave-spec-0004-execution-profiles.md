# FARAWEAVE-SPEC-0004: Execution Profiles and Resource Accounting

**Status:** Proposed

**Related issue:** [#26](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/26)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Implementation plan:** [FARAWEAVE-SPEC-0001 Implementation Plan](faraweave-spec-0001-implementation-plan.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Scalar domains:** [FARAWEAVE-SPEC-0003](faraweave-spec-0003-scalar-domain-semantics.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Structural tuples and profile v2:** [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)

**Explicit sequential fan-out:** [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Target:** Faraweave language rewrite; execution-profile policy for the evaluator,
emitted C, and native execution paths

**Compatibility:** This profile policy is a deliberate Faraweave deployment
decision. It does not claim, require, or preserve compatibility with Anka or
with the bootstrap Level 1 language.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and
do not override the semantic requirements.

## 2. Outcome and scope

This specification resolves the execution-profile decision delegated by
FARAWEAVE-SPEC-0001 section 12 and section 20. It defines:

- the named initial profiles `trusted-local-v1` and `bounded-v1`;
- which resource limits exist, their exact units, and their charging events;
- the resource context, its reset boundaries, and charge lifetimes;
- `ResourceError` reasons, required context, and CLI presentation;
- the observable agreement contract between execution backends that claim the
  same profile and configuration; and
- how conformance tests inject small limits and allocation failures
  deterministically without dangerous host allocations.

The two `-v1` names in this document are frozen to scalar/vector execution.
FARAWEAVE-SPEC-0006 defines the separate `trusted-local-v2` and `bounded-v2`
identities, a canonical tuple-table unit and limit, and a complete amended
allocation-ordinal contract. It does not silently extend either v1 identity.

FARAWEAVE-SPEC-0001 continues to own validation order, the central resource
boundary, mandatory representability and complete-allocation checks, and the
no-partial-result requirement. This specification adds deployment policy on
top of those mandatory rules; it does not weaken any of them.

## 3. Mandatory safety independent of profile

Every execution profile, including one that omits every optional limit, must
still enforce the mandatory checks of FARAWEAVE-SPEC-0001 section 12.1:

1. logical element counts must be representable by the implementation's index
   and container-size types;
2. payload and workspace byte counts must be computed with overflow-safe
   arithmetic;
3. the complete required allocation must be obtained before scalar-kernel
   execution;
4. allocation failure must surface as an explicit `ResourceError` with reason
   `allocation_unavailable`; and
5. a result is either fully constructed or absent. No partial language value,
   host exception, or process termination may serve as normal Faraweave error
   control flow.

A profile is a policy about optional limits. It is never a license to skip a
mandatory check.

## 4. Named initial profiles

The initial rewrite defines exactly two profile names:

```text
trusted-local-v1
bounded-v1
```

Profile names are exact, case-sensitive identifiers. A backend must not claim
a profile name it does not implement as specified here.

### 4.1 trusted-local-v1

`trusted-local-v1` is the default profile of the Faraweave CLI: the REPL, the file
runner, and programs built through the emitted-C and native paths when no
other profile is explicitly configured.

In `trusted-local-v1`, every optional limit is explicitly omitted:

```text
max_vector_bytes           omitted
max_live_evaluation_bytes  omitted
max_work_units             omitted
```

Omission is a decision, not an oversight: the trusted local operator already
controls the programs they run and the host they run them on, and an arbitrary
numeric cap would only misclassify legitimate large computations. Programs in
`trusted-local-v1` fail on resources only through the mandatory checks of
section 3, reported as `size_overflow` or `allocation_unavailable`.

Because no limit is configured, reason `profile_limit` is unreachable in a
conforming `trusted-local-v1` execution.

### 4.2 bounded-v1

`bounded-v1` is the deployment-configurable profile for constrained
embeddings and resource-sensitive services. A `bounded-v1` deployment:

- must configure at least one of `max_vector_bytes`,
  `max_live_evaluation_bytes`, or `max_work_units`;
- may configure any combination of the three; and
- must treat every configured limit as an exact nonnegative integer in the
  units of section 5.

There are no universal numeric defaults. This specification deliberately does
not choose concrete values; a number that protects one deployment starves
another. The resolved configuration is supplied by the product or deployment
that selects `bounded-v1`.

### 4.3 Profile identity includes resolved configuration

For agreement, testing, and diagnostics, a profile identity is the pair of the
profile name and its complete resolved configuration:

```text
trusted-local-v1 {}
bounded-v1 {max_vector_bytes = 1024}
bounded-v1 {max_work_units = 7, max_vector_bytes = 1024}
```

Two executions claim the same profile only when both the name and every
resolved limit value are identical. A `bounded-v1` execution with
`max_work_units = 7` and one with `max_work_units = 8` are different profile
identities and carry no mutual agreement obligation.

### 4.4 Fault injection is not a profile

Injected allocation failure and other conformance fault seams (section 10) are
test instrumentation. They are orthogonal to profiles, must not appear in
profile configuration, and must not be enabled in ordinary product execution.

## 5. Limits and units

All limits are nonnegative integers. A configured limit of `0` is a real
limit that permits only zero-cost operations; it is not the same as omitting
the limit. An omitted limit imposes no constraint.

### 5.1 max_vector_bytes: canonical vector payload bytes

`max_vector_bytes` bounds the canonical payload size of each individual typed
vector-payload reservation. This includes a vector result and workspace that
is explicitly reserved as a typed vector payload. It does not apply to an
untyped `reserve_workspace(context, byte_count)` request merely because an
implementation might use that workspace to hold vector-like data.

The canonical payload size of a vector of `n` elements is:

```text
payload_bytes(Vector<Bool>,   n) = n * 1
payload_bytes(Vector<Int>,    n) = n * 8
payload_bytes(Vector<Double>, n) = n * 8
```

The canonical element widths are fixed by this specification: `Bool` is 1
byte, `Int` is 8 bytes, `Double` is 8 bytes. They are accounting units, not a
physical layout mandate; an implementation may pack, align, or pad storage
differently, but it must charge the canonical widths.

Uncharged: vector metadata (type tag, length, capacity fields), allocator
bookkeeping and block overhead, alignment padding, spare capacity beyond the
logical length, and scalar results.

### 5.2 max_live_evaluation_bytes: canonical live reservation bytes

`max_live_evaluation_bytes` bounds the sum of all simultaneously live bytes
admitted through the central resource boundary within one resource context.
Every admitted reservation contributes exactly one live-byte charge:

- a typed vector-payload reservation contributes its canonical payload bytes,
  using the fixed Bool 1, Int 8, and Double 8 widths from section 5.1; and
- an untyped `reserve_workspace(context, byte_count)` reservation contributes
  exactly the requested `byte_count` after that count passes overflow-safe
  sizing and representability checks.

Typed vector workspace is charged by its canonical vector payload size and is
not charged a second time as generic workspace. Generic workspace is charged
by the requested byte count even when its physical allocation is larger or
smaller. Every vector payload and generic workspace reservation admitted in a
resource context therefore participates unambiguously in the live-byte limit.

A reservation is charged when the central resource boundary admits it and
released only when the reserved value or workspace reaches the end of its
defined logical lifetime (section 6.3).

Uncharged: physical allocation size, allocator overhead, alignment padding,
spare capacity, vector metadata, interpreter and compiler bookkeeping,
environments, diagnostic buffers, and formatting buffers. These exclusions do
not exempt any requested generic workspace `byte_count` from the live-byte
charge.

### 5.3 max_work_units: optimizer-independent logical work units

`max_work_units` bounds the total logical work charged within one resource
context. Work units measure the program's canonical semantics, never the
physical execution strategy. Charging events:

```text
one scalar (rank-0) kernel invocation          charges 1
one lifted application over n-element vectors  charges n
iota[n]                                        charges n
an application whose result is empty (n = 0)   charges 0
```

Nested calls charge additively. `inc[iota[3]]` charges 3 for `iota` and 3 for
the lifted `inc`, a total of 6, even when an optimizer fuses both into one
loop or reuses storage. Constant folding, fusion, storage reuse, and any other
optimization must not change the charged totals or the point at which a limit
is exceeded.

Uncharged: parsing, validation (arity, type, shape, resource preflight),
literal construction, canonical formatting and printing, and evaluator or
runtime bookkeeping. Vector literals charge no work; their payload is charged
through the byte limits when they allocate.

### 5.4 Exact-limit semantics

For every configured limit, charges up to and including the exact limit
succeed and the first charge that would exceed the limit fails:

```text
limit L, accumulated usage U, incoming charge c:
  U + c <= L   admitted
  U + c >  L   refused with ResourceError reason profile_limit
```

The comparison `U + c` must itself be computed with overflow-safe arithmetic;
an arithmetic overflow while sizing a request is `size_overflow`, not
`profile_limit`.

## 6. Resource context and reset boundaries

### 6.1 One context per complete program

A complete Faraweave program executed by the file runner, an emitted-C executable,
or a native-built executable runs in exactly one resource context created at
program start and discarded at program end.

For a parameterized program, FARAWEAVE-SPEC-0005's argument-count check and typed
validation or text decoding occur before this context is created. In this
section, program start means the beginning of execution after successful
argument binding; binding itself creates no profile charge.

### 6.2 Independent contexts per submission and invocation

Each REPL submission evaluates in a fresh resource context; usage never
accumulates across submissions. Each invocation of an emitted-C or
native-built executable starts a fresh resource context; usage never persists
across process invocations. Within one context the limits of section 5 apply
to the context as a whole.

### 6.3 Monotonic work, lifetime-scoped live bytes

Within one resource context:

- work-unit usage is monotonic: charges only accumulate and are never
  refunded, including for values that are later discarded; and
- live-byte usage is lifetime-scoped: a reservation's charge is released
  exactly when the reserved vector or workspace reaches the end of its defined
  logical lifetime. For intermediate values, that is when the language
  semantics no longer require the value; for workspace, when the operation
  that reserved it completes; for values bound to the program result, at
  context end.

Release points are defined by logical lifetime, not by physical deallocation.
An implementation that keeps freed storage cached must still release the
charge; an implementation that reuses storage across values must still charge
each logical reservation.

## 7. Validation order and refusal point

Profile checks live inside the resource preflight step fixed by
FARAWEAVE-SPEC-0001 section 11: arity, then type, then shape, then resource
preflight, then domain checks and kernel execution. Profiles do not add,
remove, or reorder validation steps.

Within resource preflight, for each admission request the implementation must
apply this exact order:

1. check representability and compute every vector payload, generic workspace,
   live-byte, and work-unit charge with overflow-safe arithmetic
   (`size_overflow` on failure);
2. when relevant and configured, check `max_vector_bytes`;
3. when relevant and configured, check `max_live_evaluation_bytes`;
4. when relevant and configured, check `max_work_units`; and
5. obtain the complete allocation (`allocation_unavailable` on failure).

Steps 2 through 4 are the normative precedence for simultaneous profile-limit
failures. The first configured limit in that order that would be exceeded
supplies the request's single `profile_limit` reason and structured context;
later limits are not reported for that refusal. A generic workspace request
does not make `max_vector_bytes` relevant unless the request explicitly
declares a typed vector payload as defined in section 5.1.

Admission is transactional across all charges and allocation. A request
refused by any profile limit charges no vector bytes, live bytes, or work
units, performs no allocation, and executes no scalar kernel for the refused
operation. Work units for an admitted application are charged at its
admission, before its kernels execute.

## 8. ResourceError reasons and presentation

### 8.1 Reasons

`ResourceError` carries exactly one of the stable reasons fixed by
FARAWEAVE-SPEC-0001 section 13.4:

```text
size_overflow            representability or overflow-safe sizing failed
profile_limit            a configured profile limit refused the request
allocation_unavailable   the complete allocation could not be obtained
```

### 8.2 Required context

Every `ResourceError` must carry the structured context of FARAWEAVE-SPEC-0001
section 13.4. For reason `profile_limit` the following are always applicable
and therefore required:

```text
profile name
limit kind: max_vector_bytes, max_live_evaluation_bytes, or max_work_units
configured limit value
usage before the refused charge
refused charge amount
```

For `size_overflow` and `allocation_unavailable`, requested element and byte
counts are required when available, and the profile name is reported when a
profile is active.

### 8.3 CLI presentation

When a top-level CLI evaluation, a file run, or an emitted-C or native
executable fails with `ResourceError`:

- the diagnostic is written to standard error;
- the process exits with a nonzero status; and
- no partial result value for the failed evaluation is written to standard
  output.

Conformance tests must not need to parse diagnostic text to recover the
reason or context fields.

## 9. Cross-backend agreement contract

Executions on the evaluator, emitted-C, and native paths that claim the same
profile identity (section 4.3) for the same program must produce:

- identical canonical charge events: the same sequence of charged amounts per
  limit kind, at the same admission points in validation order; and
- identical profile-refusal outcomes: the same first refused request, the same
  `ResourceError` reason, and the same structured context values.

This holds regardless of fusion, constant folding, storage reuse, physical
representation, or any other backend implementation strategy. Canonical charge
events are semantic observables of the profile contract even when an
implementation does not physically perform a distinct allocation per event.

Natural `allocation_unavailable` behavior is exempt from bit-for-bit
agreement: host allocation availability legitimately differs across machines,
moments, and backends. Backends agree on how allocation failure is classified,
reported, and contained (sections 3 and 8), while injected failures
(section 10) provide the deterministic conformance evidence.

## 10. Deterministic conformance testing

### 10.1 No dangerous host allocations

Profile and resource tests must not rely on exhausting host memory or on
allocations large enough to destabilize the test host. Determinism comes from
synthetic inputs and injected seams:

- synthetic element counts and canonical widths drive `size_overflow` cases
  through sizing arithmetic without performing the implied allocation;
- tiny configured caps (for example `max_vector_bytes = 24` or
  `max_work_units = 3`) drive `profile_limit` cases with small real
  allocations; and
- an injected allocation-failure seam drives `allocation_unavailable` cases by
  failing a chosen reservation deterministically, identified by its zero-based
  reservation ordinal within the resource context.

### 10.2 Required coverage

For each limit kind, conformance coverage must include:

1. zero: a configured limit of `0` admits only zero-cost operations and
   refuses the first positive charge;
2. one: a limit of `1` admits exactly one unit and refuses the second;
3. exact limit: accumulated charges equal to the limit succeed;
4. one past: the next charge beyond the exact limit fails with
   `profile_limit`;
5. arithmetic overflow: synthetic counts whose sizing arithmetic overflows
   fail with `size_overflow` before any limit or allocation step;
6. allocation unavailable: an injected failure at a chosen reservation ordinal
   fails with `allocation_unavailable`;
7. multiple producers: at least two distinct allocation-capable operations
   (for example `iota` and a lifted elementwise call, or a vector literal)
   exercise the same seam and the same refusal behavior;
8. no kernel execution: a refused request executes zero scalar kernels for the
   refused operation; and
9. no partial results: a refused or failed request leaves no partial language
   value observable.

Conformance coverage must also exercise simultaneous failures and verify the
normative precedence from section 7. The following cases are minimum required
fixtures; `8` denotes each incoming charge, `7` denotes each configured limit,
and usage before each charge is zero:

| Admission and simultaneously exceeded configured limits | Required singular `limit kind` |
| --- | --- |
| Typed vector payload: vector, live, and work | `max_vector_bytes` |
| Typed vector payload: vector and live | `max_vector_bytes` |
| Typed vector payload: vector and work | `max_vector_bytes` |
| Typed vector payload: live and work | `max_live_evaluation_bytes` |
| Generic workspace only: live | `max_live_evaluation_bytes` |

For each fixture, the `ResourceError` must contain the exact configured limit,
usage-before, and refused-charge fields for only the required winning limit.
The refused admission must leave every usage counter unchanged and perform no
allocation. The generic-workspace fixture must reserve an untyped
`byte_count = 8`, must not consult or report `max_vector_bytes`, and must fail
against `max_live_evaluation_bytes = 7` even if the implementation's physical
workspace allocation would have a different size.

Cross-backend tests must additionally replay the same profile identity and
program on the evaluator, emitted-C, and native paths and compare canonical
charge events and refusal outcomes (section 9).

### 10.3 Reset coverage

Tests must confirm that consecutive REPL submissions and consecutive
executable invocations start from zero usage, and that a complete program
accumulates work monotonically across its expressions within one context.

## 11. Relationship to Anka

Anka is a design inspiration, not a compatibility target. This profile policy
of named profiles, canonical accounting units, exact-limit semantics, and
cross-backend charge agreement is a deliberate Faraweave deployment-policy
choice. No behavior in this specification claims to match Anka's resource
handling, and no Anka behavior constrains it.

## 12. Non-goals and deliberate boundaries

This specification does not define:

- allocator, evaluation-context, or accounting implementation strategy;
- concrete numeric limit values for any deployment;
- a primitive-specific `iota` cap (rejected by FARAWEAVE-SPEC-0001 section 12);
- time, stack-depth, recursion, file-descriptor, or general sandbox limits;
- multidimensional shapes or accounting for future container kinds;
- an equivalence between produced-element work and live memory; and
- Level 1 compatibility after cutover.

Future profiles or limit kinds require a new versioned profile name and a new
specification or amendment; the `-v1` names defined here are frozen to the
contract in this document. FARAWEAVE-SPEC-0006 is that versioned specification for
structural tuples, and FARAWEAVE-SPEC-0007 applies its existing tuple-table event and
ordinal stream to explicit sequential fan-out. A tuple-capable backend must
claim its exact v2 identity rather than reinterpret a v1 configuration.
