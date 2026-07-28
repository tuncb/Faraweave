# Typed FWIR semantic and trust-boundary contract

**Status:** Accepted for implementation

**Authority:** This document is the normative semantic contract for the
in-memory Faraweave Intermediate Representation (FWIR). It is accepted by
[issue #2](https://github.com/tuncb/Faraweave/issues/2) and its
[decision record](../decisions/issue-2-typed-fwir-semantic-boundary.md).

**Scope:** FWIR as the immutable, verified input to all execution backends.
Exact serialized bytes, field widths, numeric identity assignments, and a
public format compatibility promise are deferred to their owning issues.

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** are normative. Examples and the
traceability notes are non-normative unless they restate a requirement.

## 1. Authority and semantic boundary (`FWIR-SEM-001`)

The source pipeline is:

```text
UTF-8 source
  -> parse and structural validation
  -> name and declaration resolution
  -> whole-program static analysis and typed lowering
  -> RawProgram verification
  -> VerifiedProgram
       -> direct interpretation
       -> deterministic strict-C11 generation
       -> native build
```

`VerifiedProgram` is the only backend semantic input. A backend must not
consult the parser AST, source call syntax, source primitive names for
dispatch, or a primitive overload table; all information needed to execute,
diagnose, account, clean up, and return every current construct is present in
the verified program and the separately supplied execution policy.

The canonical semantics are results, structured failures and their winner
order, source provenance, logical ownership, allocation/work/live accounting,
release events, and root order. Host storage layout, register allocation,
generated symbol spelling, and other physically unobservable choices are not
canonical.

## 2. Abstract program records (`FWIR-SEM-002`)

FWIR is a closed, finite, plain-data program. Its abstract records are:

| Record | Required semantic content |
| --- | --- |
| Module | Semantic contract version, required feature set, source-unit metadata, and checked ranges for every table. |
| Parameter | Declaration-order slot, scalar type, declaration origin, and source name for diagnostics only. |
| Type | Scalar, homogeneous vector, or ordered structural tuple. |
| Constant | Canonical scalar or homogeneous vector payload; tuple values are constructed by nodes. |
| Node | Node kind, complete result type, leaf cardinality when applicable, ordered edge range, origin, and kind-specific decisions. |
| Edge | Producer, semantic argument position, value access, conversion, ownership mode, and argument origin. |
| Root | Source-order result node and root origin. |
| Origin | Source unit and one-based, half-open byte span, with the derived line and column data needed by current diagnostics. |
| OperationReference | Stable primitive, signature, and implementation identities plus the source origin of one closed registered built-in selected by a higher-order consumer. |
| Feature | A mandatory semantic capability used by the module. |
| Ownership | Owner, borrow, transfer, and logical release relationships. |

Tables are indexed or ranged abstractly. Every reference and count is checked,
but this contract does not choose an integer width, byte order, Rust layout,
discriminant, or serialized order.

The module must not contain host pointers, borrowed source strings, `usize`
values as a portability promise, backend handles, allocator identities, or
parser nodes. Diagnostic names are retained data and never execution
identities.

## 3. Parameters and roots (`FWIR-SEM-003`)

Parameters form an ordered table in declaration order. Each parameter has
exactly one of `Bool`, `Int`, or `Double`, and a parameter-reference node stores
its resolved slot; execution borrows the bound scalar without cloning,
charging, mutation, lookup by name, or consume-on-read behavior.

All parameter declarations and every root are statically valid before any
argument is decoded, validated, or bound. Every declared parameter is required
even when unused or when the module has zero roots. Typed binding checks count
first, then parameters in slot order; a parameter value must be a scalar of the
declared type, and a noncanonical NaN is rejected as an invalid typed value
before container or scalar-type mismatch at that position.

Roots execute in table order. A successful program returns the complete ordered
root sequence; a root failure releases the already completed root prefix in
reverse order and returns no partial program result. Formatting and external
publication happen only after all roots execute successfully and remain
outside the FWIR node graph.

## 4. Types, values, and cardinality (`FWIR-SEM-004`)

The semantic type algebra is:

```text
Scalar = Bool | Int | Double
Type   = Scalar(Scalar)
       | Vector(Scalar)
       | Tuple(Type0, Type1, ..., TypeN-1)
```

Tuples are ordered, immutable, heterogeneous, and structural. Tuple arity and
nested element types are part of the type; a nested tuple remains one element.
Implementations must traverse arbitrarily deep valid tuple types and values
iteratively or with an explicit checked worklist rather than relying on host
call-stack depth.

Every scalar/vector result has exactly one stable cardinality class:

```text
StaticScalar
StaticVector(n)
DynamicVector
```

- A scalar literal or parameter reference is `StaticScalar`.
- A vector literal, including a typed empty vector, is `StaticVector(n)`.
- `iota` is `DynamicVector` even when its bound is a source literal.
- An elementwise application with scalar semantic operands is `StaticScalar`.
- An elementwise application with vectors is `DynamicVector` if any vector
  operand is dynamic; otherwise it is `StaticVector(n)` after known lengths
  agree.

A tuple result has no additional outer leaf-cardinality class: its fixed arity
is in `Type::Tuple`, and its immediate element nodes retain their own types and
cardinalities. This distinction prevents a tuple from being mistaken for a
homogeneous vector.

Runtime values are exactly Bool, signed 64-bit Int, IEEE-754 binary64 Double,
homogeneous vectors of those scalars, and structural tuples of values. Double
constants and values preserve exact bits; authored `nan` and produced NaNs use
the canonical quiet-NaN bits required by the current public value contract.

## 5. Constants and features (`FWIR-SEM-005`)

A scalar constant stores a Bool value, exact signed 64-bit Int, or exact
binary64 bits. A vector constant stores its scalar element type, checked
length, and ordered canonical scalar payload. Empty vector constants retain
their authored element type.

Executing a vector `Constant` performs one canonical semantic vector admission
and produces an owned result; it is not an uncharged borrow of the module
payload. Bool elements have a canonical accounting width of 1 byte and Int and
Double elements each have a width of 8 bytes. For length `n`, the request has
zero work and a byte charge computed by checked `n * width`, with the stable
producer descriptor `vector_literal`; overflow is the request's
`SizeOverflow`. A positive request is one semantic allocation attempt and its
reservation follows the owned result until failure cleanup or logical last
use, when the live bytes are released. A zero-length vector still performs the
zero-byte admission and produces an owned, typed empty result, but has no live
charge, allocation attempt/ordinal, or reservation release. These accounting
facts are part of `FWIR-SEM-005` and follow the canonical request order and
events in `FWIR-SEM-014`; a backend may elide physical storage only while
preserving them exactly.

Tuple literals are node constructions, not opaque tuple constants, because
their element evaluation, provenance, ownership transfer, table admission,
allocation ordinal, and cleanup are observable. Compact parser forms such as a
deep tuple or unary chain lower to ordinary semantic nodes and do not create
backend-specific constant or node kinds.

The mandatory feature set is the union of semantic capabilities required by
the module, including tuple/profile-v2, fan-out, and any identity or node family
not guaranteed by the base contract version. A missing required feature is
malformed program data; an execution profile that does not permit a known
required value kind is a source-program `ProfileError`. Unknown advisory
metadata may be ignored only when its declaration explicitly says it has no
semantic effect.

Semantic minor 1 adds mandatory feature `ApplicationPlans` (numeric ID 5) for
container-wide signatures. Its registry and typed-program rules are normative
in [the container-wide application-plan contract](container-wide-application-plans.md);
semantic 1.0 programs retain their previous feature set and behavior.

## 6. Nodes, edges, and evaluation order (`FWIR-SEM-006`)

The abstract node kinds needed by the current language are:

| Kind | Semantics |
| --- | --- |
| Constant | Produce the referenced immutable scalar/vector constant. |
| ParameterBorrow | Borrow one already bound parameter slot. |
| TupleConstruct | Evaluate ordered children and construct one structural tuple. |
| SelectedApply | Apply a lowering-selected primitive signature and implementation to explicit semantic operands. |
| FanOut | Evaluate one operand once, execute ordered branch regions with an explicit operand borrow, and produce one tuple. |

Prefix-spread preparation may be represented as an explicit node or as
tuple-element access edges on `SelectedApply`; either representation must carry
the same owner, element order, origins, and releases. Fan-out branch regions,
placeholder sites, and the final result structure must be explicit rather than
reconstructed from source syntax.

Edges have one of these semantic value accesses:

```text
WholeValue
TupleElement(index)
FanOutOperandBorrow
```

and one of these ownership modes:

```text
OwnedInput
ImmutableBorrow
InfallibleTransfer
```

Every semantic operand position is explicit and one-based for diagnostics.
Operands and roots preserve source order. An implementation may use a flat
postorder arena, checked ranges, and an iterative execution stack, but forward
or cyclic data dependencies, unreachable executable nodes, and ambiguous
ownership are not valid programs.

## 7. Complete lowering decisions (`FWIR-SEM-007`)

Lowering must record every backend-relevant decision exactly once:

1. parameter slot for every parameter reference;
2. complete result type and leaf cardinality for every node;
3. primitive identity, accepted signature identity, and selected scalar-kernel
   implementation identity for every application;
4. ordered semantic operands after any one-level prefix expansion;
5. one conversion class per semantic operand;
6. scalar broadcast or vector-lift mode and result element type;
7. the first static vector shape anchor, ordered static agreements, and ordered
   dynamic shape checks;
8. source-owner grouping needed to evaluate a spread operand once and release
   it once;
9. fan-out operand, branch order/ranges, placeholder substitution, result tuple
   type, table preadmission, and result-slot transfer;
10. ownership/borrow edges and logical release points;
11. required diagnostic origins and stable semantic descriptors; and
12. required module features.

Container-wide signatures additionally record one stable application-plan ID
whose registry meaning fixes whole-vector versus elementwise operand
consumption, scalar/vector result-cardinality behavior, and deterministic work
admission. Backends consume that verified identity directly.

The stable conversion classes for the current language are:

```text
Identity
PromoteIntToDouble
```

Conversions are selected during lowering. There is no Double-to-Int,
Bool/numeric, container, tuple, or runtime fallback conversion.

Exact signatures win over signatures requiring conversion; otherwise the
candidate with the fewest `PromoteIntToDouble` operands wins, with registry
order as the deterministic tie-breaker. The stable numeric values for
primitive, signature, and implementation identities are owned by issue #3;
the semantic distinction and requirement to record them are fixed here.

Backends dispatch the selected implementation identity directly. They must not
repeat arity validation, overload search, conversion selection, result-type
inference, cardinality inference, static anchor selection, or tuple-spread
classification during normal execution.

## 8. Shape, lifting, and kernel semantics (`FWIR-SEM-008`)

Elementwise applications preserve written semantic operand order. Scalars
broadcast over vectors; vectors never broadcast by length, and every vector
operand must have the same length.

The first `StaticVector(n)` semantic operand is the static anchor. Every other
static vector is checked in argument order during lowering; the first unequal
one is the static `ShapeMismatch`. Dynamic vectors neither establish nor
replace a static anchor. At execution every dynamic vector is checked in
semantic argument order against the static anchor, or the first dynamic vector
becomes the runtime anchor when no static vector exists.

After dynamic shape success, a selected vector result is admitted before
element kernels execute. Element kernels run at indexes `0..n` in increasing
order. The first domain failure is returned with the lowest failing result
index, no partial vector result is published, and admitted result bytes are
released without refunding monotonic work.

Every ordinary `SelectedApply` charges exactly one work unit for a scalar
result and exactly `n` work units for a lifted vector result of length `n`,
including zero work for an empty vector. The work is part of the result
admission: a scalar result makes a zero-byte, one-work request, while a lifted
result makes one combined vector-byte and `n`-work request. That request follows
`FWIR-SEM-014` and commits before physical allocation/materialization and before
the first scalar or element kernel; after it commits, a domain failure retains
all of the charged work.

`iota` receives one scalar Int. A bound at or below zero yields an empty Int
vector; a positive bound yields `1..=bound`. Its length conversion, vector
admission, work charge, allocation, and construction are dynamic even for a
literal bound. Its one result-admission request charges exactly its result
length in work units, including zero for an empty result, and the canonical Int
vector byte charge; the admission commits before construction begins.

`length` receives one whole Bool, Int, or Double vector and returns its
cardinality as a scalar Int. It admits exactly one work unit before checked
host-cardinality conversion, allocates no result container, and does not copy
the borrowed input; an unrepresentable cardinality is structured
`SizeOverflow`. Empty and dynamically sized vectors use the same plan, while
scalar and tuple operands are rejected during static signature selection.

`sort` receives one whole Bool, Int, or Double vector and returns a newly owned
vector with the same element type and cardinality. Its output bytes and exactly
one work unit per input element are admitted together while the immutable
input remains live; the copied output then sorts in place without another
semantic allocation. Bool and Int ascend ordinarily, while Double uses the
complete `f64::total_cmp` bit order, including `-0.0 < 0.0` and deterministic
NaN placement.

`sum` receives one whole Int or Double vector and returns a scalar of the same
element type. It admits exactly one work unit per input element before
reducing left-to-right: checked Int addition reports the first failing
zero-based element index, while Double invokes the strict binary64 addition
contract without reassociation. Empty vectors return typed zero, overflow
retains admitted work, and no result allocation is performed.

`all_of` receives one whole Bool vector and returns a scalar Bool. It admits
exactly one work unit per input element before inspecting any element, then
returns false at the first false element or true after the complete input;
empty input therefore returns true. Physical short-circuiting does not change
work, allocation, observer, or ownership traces, and no result allocation is
performed.

`any_of` receives one whole Bool vector and returns a scalar Bool. It admits
exactly one work unit per input element before inspecting any element, then
returns true at the first true element or false after the complete input;
empty input therefore returns false. Physical short-circuiting does not change
work, allocation, observer, or ownership traces, and no result allocation is
performed.

These exact units, combined result requests, and commit points are part of
`FWIR-SEM-008`, not backend cost estimates. Together with `FWIR-SEM-005` and
`FWIR-SEM-014`, they prevent an interpreter, generated runtime, or other
backend from selecting different work-limit failures, allocation ordinals, or
resource events for the same verified program and execution policy.

Integer arithmetic is checked. `div[Int Int]` truncates toward zero, rejects a
zero divisor with structured `DivisionByZero`, and rejects `Int::MIN / -1`
with the existing `IntegerOverflow`; a lifted failure retains the converted
operands and lowest failing result index. Binary64 division is successful for
zero divisors and follows the same strict behavior as other binary64
arithmetic: canonical NaN, signed zero and infinity, and gradual underflow.
These kernel semantics belong to the selected implementation identity; a
backend must not infer them from the primitive source name.

The identities reserved by the
[backend-native math v1 policy](backend-native-math-v1.md) are the sole narrow
exception to exact finite-result bit parity. Their portable special values,
operation-specific finite envelopes, direct Rust/C calls, mandatory feature,
floating-state isolation, and exact surrounding nonnumeric behavior are
normative parts of `FWIR-SEM-008`.

## 9. Direct calls and one-level tuple spreading (`FWIR-SEM-009`)

Direct bracket calls preserve every syntactic argument as one semantic
argument. A direct tuple argument is never spread:

```text
add[[1 2]]  -> one tuple argument -> arity failure
```

A prefix call has one syntactic operand. If its statically derived type is a
tuple, lowering expands exactly its immediate elements, in tuple order, into
semantic arguments; an empty tuple becomes zero arguments, and a nested tuple
element remains one tuple argument. A non-tuple prefix operand remains one
semantic argument.

Spreading is value-independent. The tuple-producing operand executes exactly
once and remains the sole owner while its immediate elements are borrowed.
Each element edge carries its own type, cardinality metadata when applicable,
and element origin; the complete tuple operand origin remains related context.

The selected application completes or selects its failure before the spread
owner is released. Borrowed elements are never cloned, moved, independently
charged, or independently released. On application success or failure,
application-local temporaries are cleaned first and the tuple owner is released
once at its logical last use; cleanup cannot replace the winning failure.

## 10. Tuple construction and ownership (`FWIR-SEM-010`)

An ordinary tuple construction executes its child nodes completely from element
0 upward. If child `i` fails, successfully completed children `i-1..0` are
released in reverse order and no outer table is admitted.

After all children succeed, the tuple table is checked and admitted for the
fixed element count. If sizing, profile, live-limit, or allocation fails, all
children are released in reverse order and the original table failure wins.
After successful admission, children transfer into slots `0..n-1` without
copying, charging, allocation, or another failure point.

Each nonempty tuple table has the current canonical immediate charge of
`n * 16` bytes and zero construction work. Empty tuple tables have zero charge
and no allocation ordinal. Moving a vector or tuple child transfers its
existing reservation identity without double charging any transitive payload.

Destroying a tuple releases immediate elements from last to first, recursively
in semantic order but through an iterative implementation, then releases the
outer table. Logical release is observable even if physical storage is stack
allocated, pooled, fused, or elided.

## 11. Sequential fan-out (`FWIR-SEM-011`)

A fan-out has exactly one operand and one or more ordered branches. Each branch
has one primitive-call root and exactly one placeholder site; the placeholder
is branch-local and must not occur in an owned tuple/vector aggregate.
Lexically nested fan-out remains invalid.

Static analysis derives the operand's complete type without values, substitutes
that type at every branch placeholder, validates every branch, and produces
`Tuple<R0, R1, ..., Rn-1>` without flattening nested branch result types. A
placeholder in prefix position follows section 9's one-level spreading; a
placeholder in direct position remains one complete argument.

After complete program validation and argument binding, fan-out executes in
this exact order:

1. evaluate the operand completely, exactly once;
2. retain its one owner;
3. size, admit, and obtain the complete positive result table for all branches;
4. execute branch 0 with an immutable borrow of the operand;
5. after success, transfer its complete result into slot 0 infallibly;
6. repeat for later branches in source order;
7. end every branch borrow, release the operand owner after the final transfer;
8. publish the complete owned tuple.

The table is preadmitted before branch 0. The one attempt must prepare all
storage and owner metadata required for every result slot, so no fallible step
may occur between a successful branch result and its transfer. Fan-out itself
charges zero work; the operand and branches retain their ordinary work.

If operand evaluation fails, no table or branch exists. If table sizing,
profile admission, or allocation fails, no branch starts and the operand is
released. If branch `i` fails, branch-local temporaries are cleaned, transferred
results `i-1..0` are released in reverse order, the result table is released,
then the operand is released; the original branch failure wins and no later
branch begins.

The operand is never reevaluated, cloned, copied per branch, mutated, moved into
a branch result, or retained by the result. Each placeholder borrow ends before
the next branch. Equal branch values still have independent result ownership.

## 12. Provenance and diagnostics (`FWIR-SEM-012`)

Origins are immutable sidecars; runtime `Value` records contain no source
ownership. Every origin identifies a source unit and a valid one-based,
half-open byte span with enough information to reproduce the current
one-based line and column.

The program retains:

- complete parameter-header, declaration, declaration-name, and reference
  origins, including related earlier-declaration origin where required;
- complete expression origins for every lowered node;
- primitive-name and complete-call origins;
- ordered source-argument and semantic-argument origins;
- tuple-element origins, using the element expression;
- complete spread-operand origin plus each immediate element origin;
- fan-out keyword/complete origin, operand origin, branch origins, and
  placeholder origins; and
- root origins.

For a forwarding tuple producer without a narrower immediate-element producer,
the complete producer origin is used for that element. Fan-out uses each branch
origin for the corresponding result element. A direct tuple argument uses the
complete tuple origin, while a spread argument uses its immediate element
origin.

Together with selected semantic descriptors, these origins must reproduce all
currently structured fields:

```text
kind, primary location/span, primitive, argument_position,
expected_arity, actual_arity, expected_types, actual_types,
expected_shape, actual_shape, resource context, domain context,
argument context, parameter context, and post-cleanup usage
```

Expected signature data comes from the recorded registry identity, not an
execution-time overload search. Domain errors retain selected parameter/result
types, converted scalar operands, and optional lowest element index. Resource
errors retain producer identity/name, request sizes, profile/limit context,
usage-before, refused charge, and allocation ordinal.

## 13. Source and static failure precedence (`FWIR-SEM-013`)

No `RawProgram` is produced until the complete source passes these ordered
phases:

1. execution-configuration validation;
2. source bytes, tokens, literals, delimiters, header placement, tuple/fan-out
   structure, nesting, branch root, and placeholder count/placement;
3. checked declaration, table-count, index, and internal representability;
4. declaration validation, parameter resolution, and primitive-name resolution
   for every root and branch;
5. execution-profile compatibility with statically visible required features,
   including the earliest tuple-producing expression;
6. dependency analysis without values;
7. primitive arity candidate selection across the complete program;
8. type/signature/conversion candidate selection across the complete program;
9. statically knowable shape candidate selection across the complete program;
10. complete typed lowering and `RawProgram` verification;
11. argument count and ordered typed/text argument validation;
12. execution.

Within phases 2 through 5, the earliest source-ordered candidate wins, using
the existing construct-specific reason tie-breakers. Name resolution covers all
roots and branches before feature/profile preflight and primitive arity, so an
unknown primitive is never hidden by a predicted runtime or profile failure.
Once names resolve, a v1 tuple-profile refusal wins before arity, type, shape,
argument, or execution work, as on the current source and backend surfaces.

Dependency analysis visits roots in source order and expressions in
left-to-right postorder. A fan-out operand precedes its branches; branches are
in source order and each branch is left-to-right postorder. A prefix-spread
arity candidate is unavailable until the operand has a valid tuple structure,
and a fan-out branch candidate using `_` is unavailable until the operand type
exists.

From all available candidates, the first arity error wins; only when none
exists may a type/signature error win, and only when none exists may a static
shape error win. A prerequisite child type error wins over a dependent parent
candidate that cannot be formed, while an independently available arity error
in another root or branch still wins over that type error. Implementations may
fuse passes only if this exact winner is preserved.

Static analysis never executes a primitive or uses runtime arguments.
Value-predictable `iota` length, profile refusal, allocation failure, dynamic
shape mismatch, integer overflow, formatting, and output remain dynamic.

## 14. Dynamic failure precedence and releases (`FWIR-SEM-014`)

After static success, dynamic work stops at the first failure in this order:

1. arguments are checked count-first, then left-to-right, before any root;
2. roots execute left-to-right;
3. an expression's children execute left-to-right and complete before the
   parent operation;
4. tuple literals evaluate all children before outer-table admission;
5. fan-out evaluates the operand, preadmits its table, then executes branches
   left-to-right;
6. an application checks recorded dynamic shapes in semantic argument order;
7. result sizing/admission precedes kernel work;
8. lifted kernels execute in increasing element index;
9. complete roots are formatted left-to-right after all execution succeeds;
10. publication occurs once after complete formatting.

One resource request selects failure in this exact order:

1. checked element-count/byte-size and prospective live/work arithmetic;
2. producer-specific byte limit (`max_vector_bytes` or
   `max_tuple_table_bytes`);
3. `max_live_evaluation_bytes`;
4. `max_work_units`;
5. positive allocation attempt and injected allocation failure;
6. admission commit and observer event;
7. physical allocation availability.

Tuple requests have zero work, and zero-byte requests receive no allocation
ordinal. Allocation ordinals are zero-based and monotonic for positive
admitted semantic allocation attempts. A natural host-allocation failure after
semantic admission refunds the refused result's live charge without changing
the ordinal stream. Work is monotonic and never refunded; live bytes release at
logical last use; allocation-attempt count is monotonic.

The vector-constant rules in `FWIR-SEM-005` and the ordinary
`SelectedApply`/`iota` work rules in `FWIR-SEM-008` are canonical inputs to this
request sequence. Backends must not split, combine, defer, omit, or add those
requests in a way that changes a refusal winner, committed usage, allocation
ordinal, admission/refusal/release event, or post-cleanup usage.

On failure, cleanup runs in reverse ownership order, is allocation-free and
infallible, and cannot replace the selected failure. The returned usage
snapshot is taken after cleanup, so live bytes reflect releases while work and
allocation attempts remain. Only an external output-device failure after
publication starts may expose an output prefix.

## 15. Canonical semantics and execution policy (`FWIR-SEM-015`)

The module records required semantic features, types, operations, conversions,
origins, ownership, and order. It must not bake in a caller's execution
profile, resource limits, allocation-failure ordinal, observer, compiler,
target, filesystem path, or output destination.

Execution policy is supplied when a verified module is instantiated or run:

```text
ExecutionProfile
ResourceLimits
AllocationFailureInjection
optional ResourceObserver
```

Policy validity is checked before source/IR work on source-facing APIs. A known
module that requires tuples or fan-out may be incompatible with a valid v1
profile; that is `ProfileError`, not malformed IR. Profile v2 preserves the
canonical 16-byte immediate tuple-slot charge, semantic allocation ordinals,
work/live accounting, and release events independently of host `sizeof` or
physical allocation strategy.

An optimization or backend policy may alter physical execution only after
proving identical values, structured diagnostics, dynamic winner order,
logical admissions, ordinals, work, live/peak accounting, releases, formatting,
and output transaction behavior. Version 1 FWIR performs no optimization that
requires such a proof.

## 16. Trust levels and `VerifiedProgram` (`FWIR-SEM-016`)

There are three trust boundaries:

1. **Source producer:** the Faraweave parser/analyzer/lowerer is trusted to
   report source semantic failures but its produced records still pass the
   verifier.
2. **Raw producer:** a builder or future decoder yields `RawProgram`; its
   indexes, ranges, identities, metadata, and ownership are untrusted.
3. **Verified consumer:** interpreters, emitters, inspectors, and serializers
   receive only an immutable `VerifiedProgram`.

The public API must make `VerifiedProgram` impossible to construct without
successful verification. Verification must be iterative, checked, deterministic,
and complete before argument binding, resource creation, backend dispatch, or
publication.

The verifier checks in this category order:

1. supported semantic version and mandatory features;
2. checked table counts/ranges and representability;
3. parameter, type, constant, and origin record invariants in table/index order;
4. node and edge references, postorder/region order, reachability, and cycles;
5. node-kind/result type/cardinality/identity/conversion/shape consistency;
6. ownership, borrow lifetime, transfer, fan-out, and release consistency;
7. roots and feature completeness.

Within a category, the lowest table/index and then the field order defined by
this contract wins. The verifier returns a `MalformedProgram` category with a
stable invariant reason, record kind/index, and field/path when available.
Unsupported versions/features and unknown primitive/signature/implementation
identities are malformed/unsupported FWIR, never `UnknownPrimitive`,
`ArityError`, `TypeError`, `ShapeMismatch`, `DomainError`, or another
source-program failure.

Invalid source returns its source semantic error and produces no partial raw or
verified program. A malformed raw program returns no `VerifiedProgram` and is
never executed "defensively" by letting backend runtime checks rediscover its
invariants.

## 17. Construct completeness (`FWIR-SEM-017`)

Every current parser expression maps without preserving parser-only variants:

| Parser construct | FWIR lowering |
| --- | --- |
| Scalar literal | Constant node with exact scalar payload. |
| Homogeneous vector literal / typed empty vector | Constant node with element type, payload, and `StaticVector(n)`. |
| Tuple literal | Ordered `TupleConstruct`; each element is lowered independently. |
| DeepTuple | Iteratively expanded ordinary tuple types/constructions; no backend-only opcode is required. |
| UnaryChain | Ordered ordinary `SelectedApply` nodes; no backend-only chain opcode is required. |
| Parameter(index) | `ParameterBorrow` with checked slot. |
| Direct Call | `SelectedApply` with one semantic edge per source argument. |
| Prefix Call | `SelectedApply` with one direct edge or explicit one-level tuple-element borrow edges. |
| Placeholder | One `FanOutOperandBorrow` edge in its validated branch region. |
| Fanout | `FanOut` with operand, preadmission, branch regions/roots, transfers, result type, and releases. |
| UnresolvedName | Source resolution failure; never valid FWIR. |
| OperationReference (`@name`) | A parser-only reference accepted only in an explicitly declared operation-reference argument position; no current executable source construct declares such a position. |

The current public value variants map exactly to section 4's values. Parser
spans and call syntax are consumed by lowering; the smaller semantic origin and
spread/edge records replace them. Primitive descriptors are consumed by
lowering; the selected stable identities, conversions, result metadata, and
diagnostic descriptor references replace them.

This mapping is complete for the current language. Adding a new source
construct, type, conversion, ownership mode, or dynamic operation requires a
new mandatory feature and an amendment to this contract before a backend may
accept it.

### 17.1 Stable built-in operation references

`@name` is a reserved, adjacent source token consisting of `@` followed by one
lowercase built-in name. It is never a prefix call and a bare primitive name is
never reinterpreted as a reference. An operation reference is not a
first-class value: it is valid only in an argument position that its consuming
higher-order primitive explicitly declares, and issue #38 introduces no such
executable primitive, so every current source placement is rejected.

The consumer supplies an arity plus parameter and result scalar constraints.
Lowering considers only closed registered `Elementwise` descriptors, applies
the ordinary identity-before-Int-promotion cost rule, rejects unknown names,
unsupported structural behavior, incompatible signatures, and equal-cost
ambiguity, then records the selected primitive, signature, and implementation
IDs and the reference origin. Backends dispatch the recorded implementation
identity; they never retain or look up `name` at runtime.

Every operation-reference record must resolve to one registry descriptor whose
three stable identities agree and whose behavior is `Elementwise`; its origin
must be valid. A module with any such record requires semantic version 1.1 and
mandatory feature `6=OperationReferences`. Semantic 1.0 programs and their
meaning are unchanged, feature 5 is defined by issue #36, and this amendment
does not reserve primitive, signature, or implementation IDs for fold or scan.

## 18. Compatibility without a physical encoding (`FWIR-SEM-018`)

Semantic compatibility means that a consumer understands the contract version,
every mandatory feature, every type/node/edge kind, and every referenced
semantic identity, and can preserve all canonical observations. It does not
mean that Rust enum discriminants, native structs, `usize`, pointers, host
endianness, or current table layouts are portable.

An incompatible major semantic version, unknown mandatory feature, unknown
node kind, or unknown primitive/signature/implementation identity is rejected
before execution. Additive advisory metadata may be ignored only when declared
non-semantic. A consumer must not guess an unknown operation from its source
name or reinterpret unknown fields as defaults.

The accepted [FWIR v1 encoding](fwir-v1-encoding.md) owns magic bytes, version
field widths, integer widths, endianness, ordering, duplicate-field rules, and
unknown-field handling. Faraweave is the authoritative producer; producer
metadata is untrusted, and third-party production support is not promised.

## 19. Requirement-to-evidence map (`FWIR-SEM-019`)

The identifiers below are the traceability keys for this contract. Each maps
to executable final-product evidence; the physical-format table separately
maps every wire field and invariant in
`tests/fixtures/fwir-v1-conformance.tsv`.

| Requirement | Final evidence |
| --- | --- |
| `FWIR-SEM-001` | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential` |
| `FWIR-SEM-002` | `rust:src/typed_program.rs::valid_fixtures_cover_every_node_and_edge_family`<br>`rust:src/typed_program.rs::verifier_category_winners_follow_the_normative_order` |
| `FWIR-SEM-003` | `rust:tests/parity_contracts.rs::typed_public_api_parameter_contract`<br>`rust:tests/cli_contracts.rs::cli_parameters_and_diagnostics_contract` |
| `FWIR-SEM-004` | `rust:tests/parity_contracts.rs::s16_empty_singleton_promotion_and_shape_contracts`<br>`rust:tests/parity_contracts.rs::deep_structural_values_and_types_format_and_drop_iteratively` |
| `FWIR-SEM-005` | `rust:tests/parity_contracts.rs::canonical_binary64_format_boundaries`<br>`rust:tests/resource_contracts.rs::typed_api_rejects_noncanonical_nan_without_normalizing_it`<br>`rust:tests/resource_contracts.rs::resource_observer_reports_commit_refusal_and_cleanup_order` |
| `FWIR-SEM-006` | `rust:src/parser.rs::parses_literals_calls_tuples_parameters_and_fanout`<br>`rust:tests/parity_contracts.rs::deep_unary_programs_use_iterative_parse_analysis_and_evaluation` |
| `FWIR-SEM-007` | `rust:src/semantic_registry.rs::production_registry_is_complete_and_numeric_lookups_are_checked`<br>`rust:src/c_emitter.rs::every_selected_id_emits_direct_dispatch_without_type_redispatch` |
| `FWIR-SEM-008` | `rust:tests/parity_contracts.rs::checked_arithmetic_has_no_partial_result`<br>`rust:tests/parity_contracts.rs::div_integer_faults_and_strict_binary64_are_exact`<br>`rust:tests/parity_contracts.rs::length_accepts_all_vector_types_empty_and_dynamic_cardinalities`<br>`rust:tests/parity_contracts.rs::sort_covers_exhaustive_small_bools_integer_edges_and_total_double_order`<br>`rust:tests/parity_contracts.rs::sum_int_overflow_reports_the_first_reduction_step_and_operands`<br>`rust:tests/parity_contracts.rs::sum_double_is_left_to_right_strict_and_preserves_special_value_bits`<br>`rust:tests/parity_contracts.rs::all_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_false_position`<br>`rust:tests/parity_contracts.rs::any_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_true_position`<br>`rust:tests/resource_contracts.rs::vector_tuple_and_work_limits_cover_zero_exact_and_one_past`<br>`rust:tests/resource_contracts.rs::div_admission_precedes_domain_and_failure_cleanup_is_exact`<br>`rust:tests/resource_contracts.rs::length_charges_constant_work_borrows_input_and_has_no_result_allocation`<br>`rust:tests/resource_contracts.rs::sort_admits_owned_output_with_input_live_and_cleans_up_refused_output`<br>`rust:tests/resource_contracts.rs::sum_charges_full_work_before_reduction_and_allocates_no_result`<br>`rust:tests/resource_contracts.rs::all_of_work_and_observer_trace_are_independent_of_the_decisive_position`<br>`rust:tests/resource_contracts.rs::any_of_work_and_observer_trace_are_independent_of_the_decisive_position`<br>`rust:src/lowering.rs::exact_ir_golden_digests_cover_every_source_construct`<br>`rust:tests/backend_native_math_policy.rs::backend_native_math_rust_reference_vectors_meet_policy`<br>`rust:tests/backend_native_math_policy.rs::backend_native_math_special_values_and_rounding_are_exact`<br>`command:strict-c11-journey` |
| `FWIR-SEM-009` | `rust:tests/parity_contracts.rs::tup_structural_format_spread_and_direct_preservation`<br>`rust:src/evaluator.rs::lifting_and_tuples_are_canonical` |
| `FWIR-SEM-010` | `rust:tests/resource_contracts.rs::tuple_allocation_ordinals_exclude_empty_tables_and_cleanup_failures`<br>`rust:tests/resource_contracts.rs::live_limit_observes_children_before_outer_tuple_admission`<br>`rust:tests/parity_contracts.rs::deep_structural_values_and_types_format_and_drop_iteratively` |
| `FWIR-SEM-011` | `rust:tests/parity_contracts.rs::fan_stable_id_matrix`<br>`rust:src/lowering.rs::fan_out_prefix_placeholder_borrows_prepare_and_preserves_elements`<br>`rust:src/c_emitter.rs::public_generated_c_matches_direct_ir_for_success_and_failure_corpus` |
| `FWIR-SEM-012` | `rust:tests/resource_contracts.rs::parameter_header_reason_and_span_contract_is_structured`<br>`rust:tests/golden_corpus.rs::authored_section_15_and_16_failure_golden_corpus`<br>`rust:tests/cli_contracts.rs::cli_parameters_and_diagnostics_contract` |
| `FWIR-SEM-013` | `rust:tests/resource_contracts.rs::profile_configuration_precedes_source_and_backend_analysis`<br>`rust:src/lowering.rs::whole_program_static_precedence_is_arity_then_type_then_shape` |
| `FWIR-SEM-014` | `rust:tests/resource_contracts.rs::refusal_precedence_is_vector_then_live_then_work_then_allocation`<br>`rust:tests/resource_contracts.rs::failure_usage_is_post_cleanup_and_work_remains_monotonic`<br>`rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential` |
| `FWIR-SEM-015` | `rust:tests/parity_contracts.rs::resource_profiles_limits_and_ordinals`<br>`rust:tests/resource_contracts.rs::generated_runtime_embeds_profile_and_verified_primitive_selection` |
| `FWIR-SEM-016` | `rust:src/typed_program.rs::identity_result_root_and_feature_invariants_are_rejected`<br>`rust:tests/fwir_conformance.rs::deterministic_mutation_corpus_is_rejected_without_panic_or_partial_program` |
| `FWIR-SEM-017` | `rust:src/lowering.rs::exact_ir_golden_digests_cover_every_source_construct`<br>`rust:src/evaluator.rs::evaluates_complete_primitive_surface` |
| `FWIR-SEM-018` | `rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact`<br>`rust:tests/fwir_conformance.rs::canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral` |
| `FWIR-SEM-019` | `python:tools/validation/contracts.py::validate_product_cutover`<br>`rust:tests/fwir_conformance.rs::traceability_references_complete_executable_evidence_sets` |
| `FWIR-SEM-020` | `python:tools/validation/contracts.py::validate_product_cutover`<br>`command:contracts-review` |

The former parameter, tuple, and fan-out requirement families are represented
respectively by `FWIR-SEM-003/004/007/008/012/013/014`,
`FWIR-SEM-004/006/009/010/012/014`, and
`FWIR-SEM-006/011/012/013/014`. Their deleted document tree and
documentation-only tests are not restored; the current Rust behavior suites
above are the active compatibility evidence.

## 20. Non-goals (`FWIR-SEM-020`)

FWIR v1 does not include an optimizer, arbitrary node sharing, reference
counting, user functions, control flow, parallel fan-out, nested fan-out,
tuple-aware primitive signatures, multidimensional arrays, third-party
production guarantees, or a new execution backend. It does not expose raw
in-memory layout as an ABI.

No implementation issue may weaken results, diagnostics, precedence,
provenance, resource events, ownership, releases, formatting, or publication
in order to simplify a backend. Any extension requires an explicit semantic
feature, compatibility rule, decision record, and conformance evidence.
