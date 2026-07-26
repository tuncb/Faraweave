# FARAWEAVE-SPEC-0006: Structural Tuples, Prefix Spreading, and Profile v2

**Status:** Proposed

**Related issue:** [#43](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/43)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Execution profiles:** [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Explicit sequential fan-out:** [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Target:** Faraweave typed analysis, evaluator, runner, emitted strict C11, and
native execution

**Compatibility:** This contract deliberately extends the Level 2 language. In
particular, it supersedes the former rejection of whitespace between a primitive
name and `[` when the bracket begins a tuple operand of prefix application. It
does not alter adjacent bracket-call syntax or parenthesized vector syntax.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and do
not override the semantic requirements.

## 2. Outcome and scope

This specification adds one immutable structural value kind, `Tuple`, and one
call-preparation rule:

- `[]`, `[1]`, and `[1 2.5 true]` are tuple expressions;
- tuples have fixed arity, ordered heterogeneous elements, and arbitrary
  structural nesting subject only to checked representability and available
  storage;
- primitive-name prefix application spreads an operand whose **static type** is
  `Tuple` exactly one level;
- adjacent bracket application preserves every source argument, including a
  tuple, as one argument;
- tuple types and values use flat owned arenas rather than recursive object
  ownership;
- tuple element tables have one backend-independent canonical charge; and
- tuple-capable execution uses the new `trusted-local-v2` or `bounded-v2`
  profile identity.

This specification also defines construction, ownership, validation,
formatting, diagnostic provenance, deterministic failure order, and the
conformance plan. FARAWEAVE-SPEC-0007 separately applies these accepted tuple and
profile-v2 rules to explicit sequential fan-out. Neither specification
implements its requirements.

## 3. Tuple source grammar

### 3.1 Productions and disambiguation

FARAWEAVE-SPEC-0002's expression production is extended with `tuple_literal`:

```text
expression = scalar_literal
           | vector_literal
           | typed_empty_vector
           | parameter_reference
           | tuple_literal
           | bracket_application
           | prefix_application

tuple_literal = "[" tuple_interior "]"
tuple_interior = optional_separator
                 [ expression { separator expression } ]
                 optional_separator
```

`parameter_reference` is present only on complete-program surfaces governed by
FARAWEAVE-SPEC-0005. `separator` and `optional_separator` retain their
FARAWEAVE-SPEC-0002 meanings. Tuple elements therefore require whitespace or a line
terminator between siblings; commas and all other unaccepted punctuation remain
invalid.

An immediately adjacent `primitive_name "["` always begins
`bracket_application`, never a tuple. A `[` in expression position begins a
tuple. These forms consequently have unique parses:

```text
add[1 2]       adjacent bracket application with two arguments
add [1 2]      prefix application whose operand is one tuple expression
[1 2]          tuple expression
add[[1 2] 3]   adjacent bracket application with two arguments; argument 1 is a tuple
```

Whitespace does not become generally optional before a bracket. A syntactic
form such as a parameter name followed by a bracket retains the call/reference
rules of FARAWEAVE-SPEC-0005, and only a valid primitive-name prefix target can form
prefix application.

The reserved header spelling remains distinct. `parameters[n Int]` is a header,
and malformed `parameters [n Int]` remains FARAWEAVE-SPEC-0005
`expected_header_open`; it is not recovered as a primitive prefix call whose
operand is a tuple. Header recognition and its phase-1 diagnostic precedence
occur before ordinary expression parsing.

### 3.2 Prefix grammar and associativity

The prefix target remains a primitive name. FARAWEAVE-SPEC-0002's existing grammar
continues to apply:

```text
prefix_application = primitive_name horizontal_separator expression
horizontal_separator = H { H }
```

Its operand is a complete expression, so prefix application remains
right-associative. This specification does not add callable expressions,
left-associative juxtaposition, implicit arity-driven grouping, or a line
terminator inside `horizontal_separator`.

Inside a tuple sibling list, expression-boundary selection is deterministic and
does not consult primitive arity or overloads. Scalar/vector/tuple literals and
adjacent bracket calls end before the next current-depth separator. On
complete-program surfaces, a bare name matching a declared parameter is parsed
as one `parameter_reference` element before considering prefix syntax; the next
current-depth separator begins the next tuple element. A lowercase name absent
from the parameter header followed by horizontal whitespace and an expression
forms `prefix_application` and is resolved as a primitive later, so unknown
primitive calls retain their ordinary diagnostic. Because a parameter may not
collide with a primitive source name, these cases do not overlap. The prefix
operand recursively uses the same rules and then the following current-depth
separator resumes the containing tuple list.

Consequently, with `parameters[x Double]`, `[x x]` contains two parameter
references, `[inc x]` contains one prefix-call element, `[x inc x]` contains a
parameter element followed by one prefix-call element, and `[unknown x]`
contains one prefix call that later fails primitive lookup. A line terminator
cannot be the separator inside a prefix application and therefore always ends
the current element at tuple depth. This is syntactic boundary selection, not
arity-driven grouping.

The following are therefore valid and unambiguous:

```text
add [1 2]
add[1 2]
inc [1]
inc add [1 2]
add [1
     2]
```

The final example has a horizontal separator between `add` and `[`, while line
terminators inside the balanced tuple are ordinary separators. `inc add [1 2]`
means `inc (add [1 2])` semantically; the inner tuple is spread for `add`, and
the resulting non-tuple value supplies one argument to `inc`.

### 3.3 Empty, singleton, heterogeneous, and nested forms

All of these are valid tuple expressions:

```text
[]
[1]
[1 2.5 true]
[1 [2 3]]
[(1 2) [true] add[3 4]]
```

A tuple may contain any expression accepted on that surface. Unlike a vector,
a tuple may be empty without a type annotation, may contain applications and
parameter references, and may contain unlike or nested types. Parenthesized
vectors remain homogeneous rank-1 values and retain all FARAWEAVE-SPEC-0002 rules.

There is no singleton grouping form: `[1]` is a one-element tuple. There is no
trailing separator token distinct from whitespace; whitespace before `]` is
already permitted by `optional_separator`.

### 3.4 Source spans

A tuple expression span begins at `[` and ends immediately after its matching
`]`. It includes internal separators and nested delimiters but excludes leading
or trailing separators outside the tuple. The opening and closing bracket each
retain their own one-byte span. Each element retains its complete expression
span in source order.

Missing and mismatched close diagnostics use FARAWEAVE-SPEC-0002's delimiter rules.
A missing sibling separator points to the first byte of the following element.
The complete tuple span and opening-bracket span are related context.

For `add [1 2]`, the prefix call retains the primitive-name span, horizontal
separator span, complete tuple operand span, both tuple element spans, and the
complete prefix-call span. For `add[1 2]`, it retains the adjacent call's
ordinary argument spans. Runtime values never store any of these spans.

## 4. Canonical values and structural types

### 4.1 Canonical value formatting

A valid tuple formats as ASCII `[` followed by each canonical element format in
source order, separated by exactly one ASCII space, followed by `]`. There is no
leading or trailing interior space and no line break:

```text
[]
[1]
[1 2.5 true]
[1 [2 3]]
[(1 2) true]
[Int()]
```

`Int()` above is source notation for a typed empty vector; its value formats as
`()`, so the value of `[Int()]` formats as `[()]`. Scalar and vector elements use
the canonical formats of FARAWEAVE-SPEC-0001 and FARAWEAVE-SPEC-0003. Nesting is
preserved and never flattened by formatting.

Formatting is independent of the source spelling and whitespace. Two equal
valid structural values have byte-identical canonical formats. Tuple equality
is not added by this statement; it defines presentation only.

### 4.2 Public structural type notation

The public diagnostic notation is exact:

```text
Bool
Int
Double
Vector<Bool>
Vector<Int>
Vector<Double>
Tuple<>
Tuple<Int>
Tuple<Int, Double, Bool>
Tuple<Int, Tuple<Int, Int>>
Tuple<Vector<Int>, Tuple<Bool>>
```

`Tuple<...>` lists immediate element types in order. A comma is followed by
exactly one ASCII space. Nested tuple and vector notation is recursive in
meaning but must be produced without host-stack recursion. This notation is not
Faraweave source syntax.

Structural type equality requires the same kind and, for tuples, the same arity
and pairwise-equal ordered element types. Tuple element names, source spans,
physical node indexes, arena capacities, and allocation identities do not
participate.

### 4.3 Flat type arena

Every owned structural type is represented semantically by three plain-data
parts:

```text
TypeArena {
  nodes[]
  child_indexes[]
  root_index
}
```

A scalar node stores one of `Bool`, `Int`, or `Double`. A vector node stores one
scalar element type. A tuple node stores a checked `first_child` and
`child_count` selecting a contiguous range in `child_indexes`; each selected
index names an immediate element-type node. Nodes are in postorder: every child
index is lower than the tuple node that refers to it, and `root_index` names the
last node. Child-index ranges appear in tuple-node postorder and do not overlap.
Every node and child-index entry is reachable exactly once from the root.

An empty tuple has one tuple node with `child_count = 0`; a scalar or vector type
has one node and no child indexes. Implementations may choose integer widths,
capacity fields, and physical block layout, but they must expose the same
checked graph, structural equality, ownership, and presentation. A recursive
node object, borrowed diagnostic pointer into a typed program, or a tuple forced
into a container-plus-scalar-element pair is not conforming.

A diagnostic that stores an actual or accepted tuple type owns its complete
`TypeArena`. It does not borrow a program, runtime value, primitive descriptor,
or temporary analysis arena. Primitive descriptors remain scalar/vector-only
until a separate tuple-aware-signature specification.

Type-arena indexes, edge counts, and byte sizes are checked before arithmetic,
conversion, allocation, or indexing. There is no language-defined tuple arity
or nesting-depth cap. Unrepresentable storage is `ResourceError(size_overflow)`;
unavailable analysis storage is `ResourceError(allocation_unavailable)`. Both
identify the complete tuple-producing expression and occur during static type
analysis before argument binding or execution.

## 5. Flat owned value arena

### 5.1 Semantic representation

An owned Faraweave value is represented semantically by:

```text
OwnedValueArena {
  nodes[]
  child_indexes[]
  root_index
  owned_vector_payloads[]
  tuple_reservations[]
}
```

Each value node is a plain tagged record:

- a scalar node stores one valid Bool, Int, or Double payload;
- a vector node stores its scalar element type, logical length, and one owned
  vector-payload handle; and
- a tuple node stores a checked `first_child` and `child_count` selecting its
  contiguous ordered element range in `child_indexes`, plus the identity of its
  one logical tuple-table reservation.

Value nodes are postorder; every child index is lower than its owning tuple
node. Child-index ranges follow tuple-node postorder, do not overlap, and every
node and child-index entry is reachable exactly once from `root_index`. A node
therefore has exactly one owner path. Aliases, cycles, shared children, orphan
nodes, overlapping child ranges, and a root other than the final node are
invalid owned values.

The arena is flat in ownership, not necessarily one physical allocation. An
implementation may use one block, several adopted blocks, or offset-adjusted
segments. It must preserve the single rooted tree, source order, checked
indexes, one logical reservation per tuple table, and one owned handle per
vector payload. Physical layout cannot change charges, allocation-failure
ordinals, validation, or cleanup order.

A forgeable raw pointer to caller memory is not a conforming public tuple edge.
The public representation must make child-range and payload-handle membership
checkable before access, for example through offsets into owned spans or
non-forgeable allocator handles. Host `sizeof`, pointer width, allocator header,
alignment, and spare capacity are never language data or accounting units.

### 5.2 Move, borrow, and copy policy

`OwnedValueArena` is a unique owner. Moving it transfers the node arena, all
vector payload handles, and all tuple reservation identities exactly once, then
leaves the source as an empty non-value owner. A moved-from owner may be
destroyed or assigned but must not be validated, formatted, applied, or returned
as a Faraweave value.

A borrowed value view consists of an immutable owner reference and a checked
node index. It is valid only while that owner remains alive and unchanged.
Spreading creates borrowed views of immediate tuple children; it does not copy,
reallocate, retain, or acquire their vector or tuple payloads.

There is no implicit or language-observable tuple copy, reference counting, or
copy-on-write. The initial public tuple boundary need not expose cloning. If an
implementation exposes an explicit clone helper, the clone must be a complete
independent owner, allocate and charge every cloned vector payload and tuple
table in the same semantic order as fresh construction, and fail
transactionally. Such a helper is not used by tuple literals or spreading.

### 5.3 Construction and ownership transfer

A tuple constructor receives ordered uniquely owned child values. After the
outer tuple table is admitted and allocated, it moves each child owner into the
new arena from element 0 upward, preserving child order and existing live
reservation identities. Moving a nested vector or tuple does not create a new
charge for its transitive payload. The outer tuple acquires exactly one new
tuple-table reservation for its immediate elements.

After each successful move, the corresponding child temporary is empty. After
all moves, only the result owner can release those child payloads. No success or
failure path may retain a second owning handle.

### 5.4 Validation and public invalid-value behavior

Public validation returns success or one structured invalid-value result. It
must not normalize, repair, partially destroy, format, or dereference an
unvalidated external pointer. Before visiting a node, it checks checked table
counts, that `root_index` names a node, and that it names the final node, in that
order. Validation then visits nodes in rooted left-to-right preorder and returns
the first failure. At each node it first checks the container tag. It then uses
this exact tag-specific order, preserving the existing scalar/vector winner
order:

- scalar: inactive vector payload, inactive tuple fields, recognized scalar
  type, inactive scalar fields, then canonical scalar bits;
- vector: inactive scalar fields, inactive tuple fields, recognized element
  type, inactive or missing vector payload/count fields, payload-handle
  membership and unique ownership, then vector elements from index 0 upward;
  and
- tuple: inactive scalar fields, inactive vector payload, checked child count
  and `first_child + child_count` arithmetic, child-range bounds and range
  ownership, postorder child indexes and unique parentage, tuple reservation
  presence, unique ownership, and exact logical element count, then child nodes
  from element 0 upward.

`inactive_tuple_field` means a scalar or vector node exposes a noncanonical
tuple range or reservation field. On a tuple node, the existing
`inactive_scalar_field` check precedes `inactive_vector_payload`. Implementations
whose private tagged union has no inactive physical fields still expose and
validate the equivalent canonical public record at this boundary. Postorder and
unique-parent checks reject cycles and aliases before following a bad edge.

After the rooted walk, orphan nodes, orphan child-index entries, orphan vector
payload handles, and orphan tuple reservations are checked in that order. The
stable tuple-related public `ValueInvariant` names are:

```text
invalid_value_root
nonfinal_value_root
inactive_tuple_field
invalid_tuple_range
overlapping_tuple_range
invalid_tuple_child_index
non_postorder_tuple_child
aliased_tuple_child
invalid_vector_payload_handle
aliased_vector_payload
orphan_value_node
orphan_tuple_edge
orphan_vector_payload_handle
missing_tuple_reservation
aliased_tuple_reservation
invalid_tuple_reservation_count
orphan_tuple_reservation
```

An empty node table or out-of-range root is `invalid_value_root`; a valid root
index that is not the final node is `nonfinal_value_root`. Checked tuple-range
arithmetic or bounds failure is `invalid_tuple_range`. Each other name maps to
the same-named check above; the first duplicate handle or parent encounter wins.

Existing scalar/vector invariant names retain their meanings. The invalid result
carries the invariant, the zero-based path of tuple element indexes from the
root, and the offending node or edge index when representable. The root path is
empty. No invalid value may reach primitive selection or a scalar kernel.

The public validation result is a discriminated `valid`, `invalid_value`, or
`resource_error` result. Checked traversal-index or worklist sizing overflow is
`ResourceError(size_overflow)`; inability to obtain a required validation
worklist is `ResourceError(allocation_unavailable)`. These host-validation
failures have no profile name or allocation ordinal because validation creates
no execution resource context. They carry the current node index and tuple path
when available, do not invent a `ValueInvariant`, and leave the input untouched.
An implementation that validates the canonical arena with no fallible worklist
cannot produce these two results for representable input.

The flat postorder representation permits a previously validated owner to be
destroyed without following untrusted edges. Destruction of a valid owner is
infallible. Public APIs that
accept caller-formed records must validate before ownership transfer; if
validation fails, the caller retains its input and Faraweave releases nothing.

The public `destroy_value` boundary distinguishes the moved-from empty owner
from a claimed value. Empty is a successful no-op. For a claimed caller-formed
owner it validates first and returns a structured `invalid_value` or
`resource_error` without changing any field or releasing any claimed handle on
failure. On validation success it performs infallible destruction and resets the
record to the empty-owner state. An internal path holding a previously validated
unique owner may use the allocation-free destruction pass directly. The public
operation must therefore evolve from an unreported `void` release if necessary;
silently dereferencing or partially freeing a malformed arena is not conforming.

### 5.5 Deterministic destruction

A valid owner is destroyed in reverse element order, depth first. For each tuple,
element `count - 1` is released first and element 0 last; each child's transitive
payload is released before the containing tuple-table reservation. A vector
payload is released when its vector node is visited. The root is released last.

The required result is one release per live reservation. The traversal must use
the flat postorder arena or another iterative bounded-index traversal and must
not allocate or fail. Physical deallocation may be cached or coalesced, but the
logical live-byte release event occurs at the required point.

An owner may outlive the resource context only as a returned public evaluator
result. At context end, every still-live result reservation is released in
reverse root order and reverse element order, and its identity is marked
detached from accounting without deallocating the owned payload. The returned
arena retains that inactive identity for ownership and validation. Later
destruction releases the physical payload but emits no second live-byte event.
An intermediate or runner/artifact owner destroyed while its context is active
performs the logical release during destruction. Thus logical charge lifetime
always ends exactly once even when physical API ownership lasts longer.

### 5.6 Public structural queries

Public tuple inspection is explicit and borrowed:

- `value_type` validates the owner and returns an independently owned structural
  `TypeArena`;
- `value_tuple_arity` validates the owner and returns the immediate tuple count;
  and
- `value_tuple_element` validates the owner, checks the zero-based immediate
  index, and returns the immutable borrowed view from section 5.2.

The latter two return `ValueAccessError(container_mismatch)` for a valid scalar
or vector and `ValueAccessError(index_out_of_bounds)` for a valid tuple index
outside its count. Validation failure retains its invariant/path, and checked
type-arena construction failure is an explicit `resource_error`. None of these
operations transfers, retains, copies, formats, charges, or releases a payload.

The existing homogeneous `value_element_type`, `value_rank`, and
`project_scalar` helpers do not reinterpret tuples. After value validation, a
valid tuple returns
`ValueAccessError(container_mismatch)` without visiting an element. Existing
scalar/vector outcomes remain unchanged. Their result structs must grow a
closed access-error field where the current validation-only result cannot
express this distinction. A tuple's immediate count is available only through
`value_tuple_arity`; legacy scalar/vector `value_length` likewise returns
`container_mismatch` for Tuple. This prevents a structural tuple from silently
becoming a homogeneous rank-1 container.

## 6. Profile-v2 resource contract

### 6.1 Versioned profile names and configuration

Tuple-capable execution defines exactly these new names:

```text
trusted-local-v2
bounded-v2
```

`trusted-local-v2` omits every optional limit:

```text
max_vector_bytes           omitted
max_tuple_table_bytes      omitted
max_live_evaluation_bytes  omitted
max_work_units             omitted
```

`bounded-v2` must configure at least one of those four exact nonnegative integer
limits and may configure any combination. Zero is a real limit. Profile identity
is the exact name plus all four resolved present/omitted values, serialized in
the order above when an ordered form is required.

Configuration validation retains FARAWEAVE-SPEC-0004's fail-closed rules: a profile
name and limit key are exact and case-sensitive; an unknown or duplicate key,
negative value, non-integer value, value not representable by the canonical
counter, or `bounded-v2` with all four limits omitted is invalid configuration
before source analysis. Omitted and configured zero are distinct. No default
numeric limit is supplied for `bounded-v2`.

`max_vector_bytes`, `max_live_evaluation_bytes`, and `max_work_units` retain
their FARAWEAVE-SPEC-0004 units and events. For scalar/vector-only programs, v2
produces the same charge amounts and admission sequence as the corresponding v1
configuration, apart from profile name and the additional omitted tuple limit.

`trusted-local-v1` and `bounded-v1` remain frozen and do not acquire tuple
charges or tuple support. A tuple-capable implementation defaults to
`trusted-local-v2`. Explicit selection of a v1 profile for a statically valid
program containing a tuple-producing expression fails before argument binding
or resource-context creation with `ProfileError(unsupported_value_kind)`,
`value_kind = Tuple`, and the earliest tuple-producing expression span. Direct
tuple-construction or evaluation entry points without source omit the span. This
failure creates no charge or allocation ordinal.

`ProfileError` is structured data distinct from `ResourceError`. For
`unsupported_value_kind`, it contains exactly `reason`, selected `profile_name`,
`value_kind`, and optional `expression_span`. The public evaluator returns it
without I/O. Runner, `emit-c`, and `build` report it as a source diagnostic at
the span, exit nonzero, and publish no stdout or destination. A source-less
direct embedding returns the structured result with no span. A generated/native
artifact containing tuples cannot claim a v1 identity, so this error occurs
before such an artifact is published rather than at artifact startup.

### 6.2 Canonical tuple-table bytes

The canonical immediate element-table charge is:

```text
tuple_table_bytes(n) = n * 16
```

where `n` is the tuple's immediate element count and 16 is the fixed canonical
`tuple_element_slot_bytes` unit. The multiplication is checked before any limit
comparison or allocation. The unit is an accounting decision, not a physical
layout mandate.

`max_tuple_table_bytes` bounds each individual tuple-table reservation. It does
not bound the sum of tuple tables, type-arena storage, vector payloads, generic
workspace, or a tuple's transitive payloads. Those live reservations participate
in `max_live_evaluation_bytes` through their own canonical charges.

An empty tuple has a zero-byte table charge, requires no physical allocation,
and receives no allocation ordinal. It still has one logical zero-byte tuple
reservation identity so ownership and validation remain uniform. Metadata,
value/type nodes, edge indexes used only for implementation bookkeeping,
alignment, allocator overhead, and spare capacity are uncharged.

The implementation must arrange the empty tuple's root node and zero-byte
reservation inline or in already-prepared result/arena bookkeeping; constructing
`[]` does not introduce a hidden fallible allocation. Failure to prepare general
program, result-slot, or analysis bookkeeping occurs at that enclosing boundary,
not as an empty tuple-table request, and has no tuple allocation ordinal.

### 6.3 Exactly-once transitive accounting

Each nonempty tuple table contributes its canonical bytes to
`max_live_evaluation_bytes` from admission until its logical release. A nested
vector payload retains its existing vector live charge. A nested tuple table
retains its own table charge. Moving either into an outer tuple transfers the
reservation identity without releasing or charging it again. The outer tuple
adds only its own immediate table charge.

For example, `[(1 2) [true false]]` has three independent live reservations
after construction:

```text
Vector<Int> payload with 2 elements     16 canonical bytes
Tuple<Bool, Bool> immediate table        32 canonical bytes
outer Tuple<Vector<Int>, Tuple<...>>     32 canonical bytes
```

The vector remains subject to `max_vector_bytes`; each tuple table is separately
subject to `max_tuple_table_bytes`; all three sum to 80 live bytes. No transitive
payload is charged as part of either outer table.

Tuple construction, movement, validation, formatting, spreading, and cleanup
charge zero work units. Primitive calls made while evaluating tuple elements or
consuming spread elements retain their ordinary work charges. FARAWEAVE-SPEC-0007
uses the same rule for fan-out: its result tuple table is charged once, while
operand/branch primitive work and branch payloads retain their own charges.

### 6.4 Admission order

One tuple-table request applies this exact order:

1. validate representability and compute `n * 16`, prospective live usage, and
   every relevant counter with overflow-safe arithmetic;
2. if configured, check `max_tuple_table_bytes`;
3. if configured, check `max_live_evaluation_bytes`;
4. recognize that tuple-construction work is zero, so `max_work_units` cannot
   refuse this request; and
5. for a positive byte count, obtain the complete allocation.

A vector request retains v1 order: representability, `max_vector_bytes`, live,
work, allocation. Generic workspace retains representability, live, work when
applicable, allocation. No one request is both a vector payload and a tuple
table.

The first failing step is the only reported failure. Overflow is
`ResourceError(size_overflow)`. A configured limit is
`ResourceError(profile_limit)` with the profile name, exact limit kind, limit,
usage before, and refused charge. Allocation failure is
`ResourceError(allocation_unavailable)` with tuple element count, canonical byte
count, profile name, and allocation ordinal. A refused request changes no usage,
creates no reservation, and moves no child owner.

### 6.5 Allocation-failure ordinals

V2 uses one zero-based allocation ordinal per resource context. An ordinal is
assigned immediately before each positive-size allocation attempt admitted
through the central resource boundary, in dynamic request order. The sequence
includes typed vector payloads, positive-size tuple tables, and positive-size
generic workspace. It excludes:

- size-overflow and profile-refused requests;
- zero-byte vector, tuple, or workspace reservations;
- work-only admissions;
- parser, type-arena, diagnostic, and formatting host bookkeeping; and
- physical allocator activity used only to coalesce or move an already-accounted
  logical reservation.

One positive tuple-table attempt must obtain, transactionally, all fallible
storage uniquely required to publish that containing tuple, including its
immediate table and any owner metadata required to adopt the already-built child
arenas. A backend may satisfy that semantic attempt with several physical
allocator calls, but they share the one canonical ordinal and any failure before
commit reports that ordinal. Optional later coalescing or movement of an already
published reservation is not a semantic allocation attempt and must not change
success, fail, or add an injected ordinal.

An injected failure at ordinal `k` makes that admitted attempt fail with
`allocation_unavailable`; no later attempt occurs. Backend optimization,
coalescing, or allocation elision must preserve the semantic ordinal sequence.
Natural host failures may differ as allowed by FARAWEAVE-SPEC-0004, but injected
failures and all profile events must agree across backends.

For `[[1] [2 3]]`, with no other allocation-producing expressions, the positive
inner tables receive ordinals 0 and 1 and the outer table receives ordinal 2.
For `[[] [1]]`, the empty inner table receives no ordinal, the singleton inner
table receives ordinal 0, and the outer table receives ordinal 1.

## 7. Tuple construction and runtime failure order

### 7.1 Static validity first

The complete program is parsed, resolved, and statically validated before
argument binding or root execution as required by FARAWEAVE-SPEC-0005. A tuple
element that is statically invalid prevents all runtime element evaluation and
tuple admission.

Static type analysis builds each tuple's ordered structural type from its element
types. It never evaluates an element to determine that type. Tuple arity and
prefix spreading are therefore value independent.

### 7.2 Runtime construction sequence

After static success, a tuple literal executes in this exact order:

1. evaluate element 0 completely;
2. evaluate each later element completely in increasing source order;
3. after every element succeeds, compute and admit the outer tuple table under
   section 6.4;
4. allocate the complete positive-size table, if any;
5. move child owners into the result from element 0 upward; and
6. publish one complete owned tuple value to its parent or root slot.

No tuple table is admitted before the final element succeeds. Element evaluation
may itself construct nested tuples, vectors, or call results and emits their
ordinary charge events before the outer table event. Successful child work is
not refunded if a later element or outer table fails.

If element `i` fails, already completed elements `i - 1` through 0 are destroyed
in reverse order. If sizing, a profile limit, or allocation fails after all
elements succeed, all elements are destroyed in reverse order. No ownership
move has begun. Moving into an admitted table is an infallible record transfer;
an implementation must not place another fallible allocation between the first
move and result publication.

A construction failure returns no partial tuple or root and leaves no tuple-table
live charge. Child live charges are released by cleanup; monotonic work charges
remain until the resource context ends.

### 7.3 Root formatting and publication

The public evaluator returns owned structured values and performs no formatting.
Runner and artifact surfaces execute every root before formatting, then retain
FARAWEAVE-SPEC-0005's one pending output batch and publish-last order.

For the public evaluator, successful roots remain live through complete program
execution. Immediately before the result crosses the API boundary, the resource
context ends and detaches those result reservations as section 5.5 specifies;
the returned physical owners remain valid. Failed evaluation returns no owner
and completes reverse-root cleanup before discarding the context.

Before emitting bytes for one root, formatting validates that complete root
value using section 5.4. Roots are considered in source order. Within a tuple,
the first invalid node in left-to-right preorder wins. Validation failure is
`FormattingError(invalid_value)` with the root position/span, exact invariant,
and tuple element path. No bytes for any root are published.

The public `format_value` operation follows the same validate-then-convert order,
does not mutate or take ownership of its input, and returns either one complete
owned string or an empty string plus the structured error; it never returns a
partial formatted value. A validation `resource_error` maps to
`FormattingError(conversion_failure)`, carries that `ResourceError` as related
structured context, and carries no invented `ValueInvariant`.

After validation, canonical conversion proceeds iteratively in element order.
Unavailable or unrepresentable formatting bookkeeping is
`FormattingError(conversion_failure)` for the current root. Formatting buffers
and traversal stacks are host presentation bookkeeping and have no profile
charge or allocation ordinal. Every root owner remains live until all roots
format successfully or failure cleanup completes.

On any execution or formatting failure, completed roots are destroyed in reverse
root order using section 5.5. On success they remain alive through the complete
stdout write and flush, then are destroyed in reverse root order. An unavoidable
`OutputError` may expose a byte prefix exactly as FARAWEAVE-SPEC-0005 permits; it does
not change value ownership or release order.

## 8. Static one-level prefix spreading

### 8.1 Semantic argument sequence

For `primitive_name horizontal_separator operand`, typed analysis first obtains
the operand's complete static type:

```text
static type Tuple<T0, ..., Tn-1>  -> semantic arguments T0, ..., Tn-1
any scalar or vector type         -> one semantic argument of that type
```

Expansion is exactly one level. A tuple element whose type is itself `Tuple`
remains one tuple argument. The rule runs before primitive arity validation,
overload/signature selection, `Int -> Double` conversion selection, static or
dynamic shape validation, resource preflight, domain validation, and primitive
execution. It does not inspect the primitive descriptor table to decide whether
to spread.

Consequences for the initial primitive table include:

```text
add [1 2]       two Int arguments; same selected add semantics as add[1 2]
add []          zero arguments; ArityError supplied_count = 0
add [1]         one argument; ArityError supplied_count = 1
inc [1]         one Int argument; succeeds
add [1 [2 3]]   two arguments; TypeError on argument 2 with Tuple<Int, Int>
```

`add [1 2]` and `add[1 2]` have the same semantic argument types, values,
conversion, shape, kernel, and result behavior after call preparation. They do
not have identical resource event sequences: the prefix form constructs and
charges a tuple before spreading, while the adjacent direct form does not.
Observable allocation failure and profile limits therefore remain truthful.

### 8.2 Runtime borrow and last use

The operand evaluates exactly once. For a tuple operand, call preparation borrows
its immediate child views in element order. For a non-tuple operand, it borrows
one view of the operand root. Primitive application never owns or releases those
views.

The operand owner remains alive through arity, type, shape, resource, domain, and
primitive-result construction. It is destroyed at its logical last use only
after the call has either produced its complete owned result or selected its
failure. A vector or nested tuple element is never copied or reallocated for
spreading.

Tuple construction failure wins before any outer call validation because there
is no operand to spread. Once construction succeeds, the expanded call follows
the ordinary order: arity, type/overload/conversion, shape, resource, domain or
kernel execution. The first failure in that sequence wins, then operand cleanup
runs without replacing it.

### 8.3 Direct calls and public primitive application

An adjacent bracket application never spreads implicitly. Each source argument
remains one semantic argument regardless of static type:

```text
inc[[1 2]]      one supplied argument, then TypeError on argument 1
add[[1 2]]      one supplied argument, then ArityError
add[[1 2] 3]    two supplied arguments, then TypeError on argument 1
```

The actual type context owns and presents the complete structural tuple type.
Until a separate specification adds tuple-aware primitive signatures, every
tuple reaching an argument position that survives arity checking fails ordinary
type validation. No primitive descriptor, `apply_primitive`-style public direct
API, scalar kernel, or vector kernel performs spreading. A caller of a direct
public primitive API that passes a tuple receives the same ordinary arity-first,
then lowest-argument type behavior as an adjacent bracket call.

The direct public operation first checks supplied arity without reading an
argument value. If that arity exists, it validates argument owners from position
1 upward before deriving their structural types. The first malformed owner
produces `ValueError(invalid_value)`; no later owner is inspected and no kernel
runs. This public-boundary error contains primitive identity, one-based argument
position, exact `ValueInvariant`, tuple path, and node/edge index when available.
It is distinct from `TypeError` because no valid actual structural type exists.
The operation then performs ordinary type, shape, resource, and domain steps.
Consequently an arity error wins without inspecting even a malformed tuple,
while a valid tuple at an arity-compatible position reaches structural type
rejection. Source-produced values are valid by construction, so `ValueError` is
specific to public caller data. The initial table has no tuple-aware signature,
so direct application never reaches a profile resource check for that tuple;
source or direct tuple construction under v1 is rejected separately by section
6.1.

### 8.4 Static analysis candidate order

Tuple-dependent call arity requires an operand type, while that operand may
contain calls whose types require signature selection. The whole-program static
contract is therefore dependency-aware:

1. complete FARAWEAVE-SPEC-0005 phases 1 through 3;
2. analyze expression dependencies in left-to-right postorder, deriving tuple
   structures and every statically available primitive error candidate without
   executing values;
3. select the winning primitive arity candidate across all roots in source and
   postorder traversal order;
4. if none exists, select the winning type/signature candidate in that order;
5. then select the winning statically knowable shape candidate in that order.

An outer prefix call's arity candidate becomes available only after its operand
has a valid static type and expansion count. A child type failure that prevents
that type therefore wins over the unavailable parent candidate. An arity
candidate in another independently analyzable root still wins over a type
candidate, preserving FARAWEAVE-SPEC-0005's program-wide category precedence. An
implementation may use several passes or a dependency worklist, but it must
produce this result and must not execute an operand to classify spreading.

## 9. Diagnostic provenance without runtime spans

### 9.1 Static origin sidecar

Every typed expression has one complete expression span. In addition, every
statically tuple-typed expression has an analysis-owned outer-element origin
array with exactly one entry per immediate structural element. This sidecar is
stored in the parsed/typed program or emitted immutable metadata, not in
`OwnedValueArena`.

Origin selection is exact:

1. a tuple literal element uses that element expression's complete span;
2. a tuple-producing construct with separately spelled result producers uses
   the complete producer span assigned to that result position by the
   construct's specification;
3. a forwarding construct that cannot identify a narrower producer uses its
   complete expression span for each forwarded outer element.

The first rule gives `[1 add[2 3]]` the spans of `1` and `add[2 3]`. The second
allows FARAWEAVE-SPEC-0007 fan-out to use each branch span. A future tuple-producing
construct must define rule 2 or explicitly accept rule 3. Runtime tuple values
remain source-independent and can move without span repair.

### 9.2 Expanded-call diagnostics

Call diagnostics retain the primitive-name span, complete call span, and complete
operand span. Each expanded argument receives:

- its one-based semantic position;
- its static structural type;
- the sidecar origin span for that outer element; and
- the complete operand span as related context.

`ArityError` reports expanded supplied count and carries the ordered origin spans
that exist. Empty spreading carries an empty origin list. `TypeError` and
`ShapeMismatch` use the failing expanded argument's origin as primary and retain
primitive, call, and operand context. `ResourceError` and `DomainError` retain
the primitive and call spans plus argument/index context required by the earlier
specifications; when they identify an argument, they use the same origin.

A direct bracket-call tuple argument uses the complete tuple expression span as
that one argument's origin. It does not substitute one of the tuple's element
spans because no expansion occurred.

Emitted C contains the same static origin records needed by runtime diagnostics.
Backends must not reconstruct origins from canonical value text, node indexes,
or runtime payload addresses.

## 10. Deep nesting and checked traversal

Tuple source, types, and values have no language-defined maximum arity or nesting
depth. Finite source size, checked index/count/byte representability, selected
profile limits, and available storage are the only bounds.

All Faraweave-owned operations whose work can grow with tuple depth must use flat
linear passes, explicit checked worklists, or another strategy with no reliance
on host call-stack recursion. This includes:

- delimiter and tuple parsing;
- structural type construction, equality, and presentation;
- value construction and ownership transfer;
- public validation;
- canonical formatting;
- failure cleanup and normal destruction; and
- emitted-C/native equivalents.

Parser/type-analysis worklist overflow is a static `ResourceError`; value
construction follows section 6; validation reports an invalid representation or
explicit unavailable bookkeeping without touching the value; formatting follows
section 7.3. Destruction is allocation-free and infallible. A host stack overflow,
uncaught exception, process termination, silent depth cap, truncated traversal,
or leaked suffix is not conforming.

Conformance deep-nesting fixtures must be generated without embedding an
implementation limit in this specification. They must substantially exceed
ordinary recursive test depths and run under reduced host-stack settings where
supported.

## 11. Compatibility and frozen boundaries

### 11.1 Amendment to FARAWEAVE-SPEC-0002

This specification supersedes only these FARAWEAVE-SPEC-0002 statements:

- the expression list now includes `tuple_literal`;
- square brackets in expression position may delimit tuples;
- bracket nesting is no longer only application nesting;
- vector literals still contain scalar literals only, but tuple literals contain
  expressions;
- `add [1 2]` is no longer the normative whitespace-before-bracket invalid
  example; it is prefix application to a tuple; and
- prefix syntax supplies one **operand**, then the static rule in section 8 may
  produce zero or more semantic arguments.

Adjacent `add[1 2]`, zero-argument `add[]`, right associativity, logical records,
source bytes, scalar/vector literals, delimiters, and all unrelated invalid forms
retain their earlier meanings. `add[]` is an adjacent zero-argument call; `[]`
is an empty tuple; `add []` evaluates and spreads an empty tuple before reaching
the same supplied arity of zero.

### 11.2 Relationship to FARAWEAVE-SPEC-0004

FARAWEAVE-SPEC-0004's `trusted-local-v1` and `bounded-v1` names are frozen. This
specification does not append tuple semantics to them. V2 incorporates all v1
mandatory safety, context/reset boundaries, exact-limit rules, `ResourceError`
reasons, transactional admission, and cross-backend agreement, then adds the
fixed tuple-table unit, limit, profile identities, and ordinal rules in section
6.

FARAWEAVE-SPEC-0007 sequential fan-out uses v2's existing tuple-table event rather
than inventing a backend-sized charge. Every other future tuple producer must do
the same. A future new
resource kind or changed canonical event requires another versioned profile; it
must not silently mutate v1 or v2.

### 11.3 Deliberate Anka differences

Anka remains design inspiration, not a compatibility target. Faraweave uses brackets
for structural tuples and keeps concise prefix syntax, but Faraweave deliberately
requires static one-level spreading, primitive-name targets, right-associated
complete operands, explicit adjacent direct calls, versioned resource profiles,
flat ownership, and structured provenance. It does not import Anka currying,
blocks, trains, placeholders, connected tuples, word folding, or executor
semantics. FARAWEAVE-SPEC-0007 adds a narrowly scoped branch-local placeholder and
sequential construct without importing those broader Anka rules.

## 12. Non-goals and fixed follow-ups

This specification does not add:

- tuple destructuring, mutation, equality, ordering, lifting, indexing, or
  pattern matching;
- tuple-aware primitive signatures;
- explicit spread markers mixed with direct arguments;
- recursive flattening;
- callable values, callable-expression targets, partial application, user
  functions, closures, or blocks;
- vector or tuple program parameters;
- fan-out syntax or branch semantics within this tuple specification;
- runtime reference counting or shared ownership; or
- optimization that removes an observable tuple reservation, profile refusal,
  allocation ordinal, failure, or lifetime event.

Each requires separate specification and acceptance. FARAWEAVE-SPEC-0007 is the
separate specification for explicit sequential fan-out; the other exclusions
remain unchanged. In particular, a compiler
may elide a tuple used only by immediate spreading only after proving and
preserving the exact v2 charge, fault-injection, diagnostic, ownership, and
failure-order observations as if construction occurred.

## 13. Requirement-to-test plan

Issue #43 records the normative contract and plan only. Implementation issues
must add executable tests and exact identifiers to `tests/spec-traceability.tsv`.
Supported target-native journeys are Linux x64, Windows x64, and macOS arm64.
Strict C11 means GCC/Clang-compatible `-std=c11 -Wall -Wextra -Werror
-pedantic-errors` and Windows MSVC `/std:c11 /W4 /WX` where applicable.

| Plan ID | Normative requirement | Required future evidence |
| --- | --- | --- |
| `TUP-001-GRAMMAR` | Sections 3 and 11 tuple grammar, adjacency split, sibling boundaries, separators, delimiters, and Level 2 amendment. | Parser fixtures for empty/singleton/heterogeneous/nested/multiline tuples, tuple elements that are calls/vectors/parameters, `[x x]`, `[inc x]`, `[x inc x]`, unknown prefix targets, LF/CRLF boundaries, all missing/mismatched/separator errors, `add[1 2]`, `add [1 2]`, `add[]`, `add []`, and the retained malformed-header whitespace rejection. Assert unique flat shapes and exact spans. |
| `TUP-002-TYPES` | Section 4 structural type arena, equality, ownership, and exact notation. | Public type tests for every scalar/vector/tuple example, unlike order/arity/nesting, postorder/range/alias/orphan invalid arenas, owned diagnostic lifetime after program destruction, checked synthetic overflow, and iterative deep presentation. |
| `TUP-003-VALUES` | Section 5 flat value arena, invariants, and structural query boundary. | Public tests inject every tuple/root/edge/payload/reservation invariant plus existing scalar/vector failures at root and nested paths; prove first-preorder winner, no unsafe access, no normalization, and complete valid empty/singleton/heterogeneous/deep values. Exercise owned `value_type`, tuple arity/borrow, every access error, and unchanged scalar/vector queries. |
| `TUP-004-MOVE-CLEANUP` | Unique move, borrow, transfer, public destruction, result detachment, and reverse iterative release. | Instrumented ownership tests move nested vectors/tuples, fail before/after table admission, and count every payload/table release exactly once; invalid public destruction changes/releases nothing; moved-from destruction is a no-op; returned evaluator owners detach accounting once; borrowed views never outlive owners. ASan/UBSan and reduced-stack stress remain clean. |
| `TUP-005-FORMAT` | Sections 4.1 and 7.3 canonical iterative formatting and failure order. | Direct formatter and runner/C/native fixtures assert exact bytes for empty/singleton/heterogeneous/nested/vector tuples; inject nested invalid values and conversion-workspace failure; assert first root/path, zero stdout, reverse cleanup, and deep reduced-stack success. |
| `TUP-006-PROFILE-IDENTITY` | Section 6.1 exact v2 names/configurations and v1 refusal. | Configuration tests cover omitted/zero/partial/all limits, malformed/unknown/duplicate settings, v2 default, scalar-vector v1/v2 charge equivalence, and v1 tuple refusal before context/arguments. |
| `TUP-007-TABLE-CHARGE` | Sections 6.2 through 6.4 fixed 16-byte slot, individual/live limits, no transitive double charge, and zero work. | Instrumented exact-limit/one-past/overflow tests for empty through nested tuples and vectors-in-tuples; compare exact event sequences, usage-before, refusal kind, transfer, and release across evaluator/C/native without `sizeof`. |
| `TUP-008-ALLOCATION-ORDINAL` | Section 6.5 positive admitted allocation sequence. | Fault injection at every vector/tuple/workspace ordinal, including the two nested examples; prove zero-size/profile/overflow exclusions, one canonical ordinal across any required physical suballocations, optimizer independence, no later attempt, exact context, and transactional cleanup across all backends. |
| `TUP-009-CONSTRUCTION` | Section 7 left-to-right elements, admit-after-elements, infallible transfer, and failure cleanup. | Side-effect/fault probes establish each element once, child failure before outer admission, outer failure after all children, reverse cleanup, monotonic work, no root/partial stdout, and matching C/native event traces. |
| `TUP-010-STATIC-SPREAD` | Section 8 static exactly-one-level expansion before call validation. | Typed-analysis fixtures for empty/singleton/heterogeneous/nested literal and computed tuples, non-tuple operands, overload-table additions, and dependency-aware cross-root errors. Assert expansion never inspects runtime values. |
| `TUP-011-SPREAD-RUNTIME` | Operand once, borrowed outer children, ordinary expanded failure order, and logical last use. | Evaluator/runner/emitted/native differential journeys for `add [1 2]`, `inc [1]`, empty/singleton errors, nested type failure, vector child borrowing, domain/resource failure, and operand release after complete result/failure. |
| `TUP-012-DIRECT-PRESERVATION` | Adjacent and public direct application preserve tuple arguments. | Structured arity/type tests for `inc[[1 2]]`, `add[[1 2]]`, `add[[1 2] 3]`, and caller-supplied tuple values; assert no primitive descriptor/kernel sees or spreads a tuple. |
| `TUP-013-PROVENANCE` | Section 9 static sidecars and exact literal/computed/direct origins. | Parser/IR/emitted metadata tests assert whole call/operand and per-element spans; argument-specific evaluator/C/native failures select literal child, nested child, future producer, forwarding fallback, or direct whole-tuple span exactly; runtime value records contain no spans. |
| `TUP-014-STATIC-ORDER` | Section 8.4 dependency-aware candidate precedence. | Multi-root and nested programs combine unavailable parent arity, child type errors, independent arity errors, shape errors, argument failures, and predicted runtime failures; direct/emit/build select the same exact static winner before binding. |
| `TUP-015-ATOMIC-OUTPUT` | Sections 7 and 8 transactional execution/format/publication. | Programs with earlier successful roots and later tuple element, allocation, expanded call, or formatting failure publish zero stdout on runner/C/native; short-write/flush remain only the existing OutputError exception. |
| `TUP-016-DEEP-NESTING` | Section 10 no language depth cap and no host recursion promise. | Generated deeply nested source/type/value fixtures run parser, analyzer, validation, formatting, normal cleanup, and every failure cleanup under reduced stacks on supported targets; synthetic count/index overflow fails explicitly rather than truncating. |
| `TUP-017-STRICT-C-NATIVE` | Flat generated representation and backend agreement. | Deterministic emitted bytes; strict C11 compilation; native success/error/fault runs; inspect generated source for iterative traversal, no runtime overload selection for spread, and no `sizeof`-derived canonical charge. |
| `TUP-018-REGRESSION-PLATFORMS` | Unchanged vectors, adjacent calls, v1 behavior, and supported targets. | Full Release/strict/no-exceptions/sanitizer suites plus Linux/Windows/macOS tuple corpus; existing parenthesized vectors, headerless programs, profiles, CLI, emitted C, and native paths remain green except the explicitly superseded source form. |

No one surface substitutes for another. Public C++ tests establish owned data and
structured errors; evaluator tests establish semantic values; runner tests
establish source diagnostics and atomic output; emitted strict C11 and native
tests establish portable ownership and accounting; fault tests establish every
reservation boundary; and the platform matrix establishes target-independent
behavior.
