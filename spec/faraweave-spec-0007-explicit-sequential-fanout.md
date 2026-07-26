# FARAWEAVE-SPEC-0007: Explicit Sequential Fan-out

**Status:** Proposed

**Related issue:** [#44](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/44)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Execution profiles:** [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Structural tuples and profile v2:** [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)

**Target:** Faraweave typed analysis, evaluator, runner, emitted strict C11, and
native execution

**Compatibility:** This contract deliberately extends the Level 3 expression
language. It adds the reserved `fanout` construct, brace-delimited branch
templates, and the branch-local `_` placeholder. It does not change ordinary
primitive-call, tuple, vector, parameter, or prefix-spreading behavior.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and do
not override the semantic requirements.

## 2. Outcome and scope

A fan-out evaluates one operand exactly once, lends one immutable borrowed view
of that value to each explicitly delimited branch template, executes the
branches sequentially from left to right, and returns their independently owned
results as one ordered structural tuple.

The semantic name is `fanout`. It does not promise or permit parallel branch
execution. The initial construct has these fixed boundaries:

- at least one branch is required;
- each branch is enclosed in its own `{...}` delimiters;
- each branch body is rooted at a primitive call;
- each branch contains exactly one explicit `_` input placeholder;
- the placeholder may only be consumed through primitive-call argument flow;
- all branches are parsed, resolved, and statically validated before execution;
- the operand is evaluated and charged once;
- the result tuple table is admitted after the operand succeeds and before
  branch 0 begins;
- branches execute in source order and the first dynamic failure wins;
- each successful branch transfers one complete independently owned result into
  its preassigned tuple element;
- the operand remains alive through its final branch use and is never copied per
  branch; and
- the result is a singleton tuple when exactly one branch is supplied.

This specification defines syntax, spans, static and dynamic ordering,
ownership, cleanup, provenance, profile-v2 events, backend agreement, and the
required implementation test plan. Issue #53 implements this contract across
the parser, evaluator, emitted-C, and native surfaces.

## 3. Source grammar and branch boundaries

### 3.1 Productions

FARAWEAVE-SPEC-0002's expression production is extended with
`fanout_expression`:

```text
expression = scalar_literal
           | vector_literal
           | typed_empty_vector
           | parameter_reference
           | tuple_literal
           | fanout_expression
           | bracket_application
           | prefix_application

fanout_expression = "fanout" "[" optional_separator
                    expression separator
                    fanout_branch
                    { separator fanout_branch }
                    optional_separator "]"

fanout_branch = "{" optional_separator
                branch_expression
                optional_separator "}"

branch_expression = scalar_literal
                  | vector_literal
                  | typed_empty_vector
                  | parameter_reference
                  | branch_tuple_literal
                  | branch_bracket_application
                  | branch_prefix_application
                  | input_placeholder

branch_tuple_literal = "[" branch_tuple_interior "]"
branch_tuple_interior = optional_separator
                        [ branch_expression
                          { separator branch_expression } ]
                        optional_separator

branch_bracket_application = primitive_name "[" branch_call_interior "]"
branch_call_interior = optional_separator
                       [ branch_expression
                         { separator branch_expression } ]
                       optional_separator

branch_prefix_application = primitive_name horizontal_separator
                            branch_expression

input_placeholder = "_"
```

`parameter_reference` is available only on the complete-program surfaces of
FARAWEAVE-SPEC-0005. `separator`, `optional_separator`, and
`horizontal_separator` retain their existing meanings. The branch grammar is
the ordinary expression grammar parameterized recursively to admit `_` at an
expression position and to exclude a nested `fanout_expression`. A completed
branch expression is then validated as the primitive-call-rooted template in
section 4.3. Tuple placeholder placement and every other restriction in
sections 4 and 12 remain static checks rather than parser reinterpretation.

The exact ASCII bytes introduced by this grammar are `{`, `}`, and `_`. Braces
are valid only as the branch delimiters of a `fanout_expression`. `_` is valid
only as that branch's input placeholder. A bar `|`, semicolon, comma, or other
punctuation is not a fan-out separator and remains invalid.

### 3.2 Reserved keyword and unique parse

`fanout` is an exact, lowercase, reserved expression keyword. It cannot be a
parameter name or primitive source name. Once the tokenizer recognizes the
standalone word `fanout`, the next byte after the keyword must be the adjacent
`[` of the construct. `fanout [...]`, bare `fanout`, and `fanout(...)` do not
fall back to an unknown primitive or parameter reference; they fail with
`expected_fanout_open` at the first non-adjacent byte or the end insertion
position.

Within `fanout[...]`, the first complete expression is the operand. A
current-depth `{` cannot belong to an ordinary expression and begins the first
branch after the required separator. Each matching `}` completes exactly one
branch. A later current-depth `{` begins the next branch only after a separator.
The final `]` closes the fan-out. Parser recovery must not combine adjacent
branch bodies or infer a branch boundary from primitive arity.

These forms therefore have one parse:

```text
fanout[1 {inc[_]}]
fanout[iota[3] {inc[_]} {add[_ 10]}]
fanout[[1 2] {add _} {equals[_ [1 2]]}]
add fanout[1 {inc[_]} {add[_ 2]}]
add[fanout[1 {inc[_]} {add[_ 2]}]]
```

The fourth source is prefix application whose complete operand is a computed
fan-out tuple. The fifth is adjacent direct application with one fan-out tuple
argument.

### 3.3 Separators and multiline forms

At least one separator is required between the operand and the first branch and
between adjacent branches. Separators may contain spaces, tabs, LF, or CRLF.
Whitespace is optional immediately inside `[` and `]` and inside each pair of
braces. A branch prefix application's `horizontal_separator` still cannot
contain a line terminator, although balanced calls and literals within that
operand may span lines.

```text
fanout[
  iota[3]
  { inc[_] }
  {
    add[_ 10]
  }
]
```

The example has one operand and two branches. Newlines do not create additional
roots while the fan-out bracket remains open.

### 3.4 At least one branch

A fan-out always has one operand and one or more branches. There is no
zero-branch value:

```text
fanout[1]       invalid: expected_fanout_branch before ]
fanout[]        invalid: expected_fanout_operand before ]
```

A one-branch construct is valid and its type and value are a singleton tuple:

```text
fanout[1 {inc[_]}]   : Tuple<Int>, value [2]
```

The language does not collapse that result to its only element.

### 3.5 Structural syntax reasons and winner

The stable fan-out structural reasons are ordered as follows:

```text
expected_fanout_open
expected_fanout_operand
expected_fanout_branch
expected_branch_open
expected_branch_body
expected_branch_close
missing_fanout_close
unexpected_fanout_token
nested_fanout
branch_root_not_primitive_call
placeholder_count
placeholder_owned_position
```

Source-byte, token, and mismatched-delimiter failures retained from
FARAWEAVE-SPEC-0002 still use their existing reasons. Analysis forms every
applicable structural candidate for the complete program. The earliest primary
span wins; candidates beginning at the same byte use the reason order above,
then lower zero-based branch index. An existing offending token has its complete
token span. A missing item before a closer or end of source uses the zero-length
insertion span there.

`branch_root_not_primitive_call`, `placeholder_count`, and
`placeholder_owned_position` are structural analysis failures rather than
runtime type errors. For one branch they are checked in that order after its
brace structure is complete. `placeholder_count` carries the actual lexical
count and required count 1. `placeholder_owned_position` identifies the sole
placeholder. These checks run for branches in source order, but the complete
program remains subject to section 6's global static-category ordering.

## 4. Placeholder scope and legal use

### 4.1 One lexical occurrence per branch

Each branch body contains exactly one `_` token at that branch's lexical level.
It denotes the complete fan-out operand value with its statically derived
structural type. It is immutable, cannot be assigned, moved, retained, or
addressed, and has no name outside that branch.

The occurrence counts are lexical after tokenization, not inferred from control
flow or primitive arity. A placeholder in one branch does not satisfy another
branch. There is no implicit placeholder and no placeholder capture from an
outer construct.

```text
{inc[_]}        one occurrence
{add[_ 1]}      one occurrence
{add[_ _]}      invalid: two occurrences
{inc[1]}        invalid: zero occurrences
```

`_` outside a branch template is invalid syntax. It is not a parameter name,
primitive name, discard pattern, wildcard, tuple hole, or future-binding
promise.

### 4.2 Primitive-call argument flow

The placeholder is legal only where its value flows as an argument of the
branch root call or of a nested primitive call. Every ancestor between `_` and
the branch root must therefore be a primitive application. Both adjacent direct
and prefix application are allowed:

```text
{inc[_]}
{add[_ 1]}
{inc[add[_ 1]]}
{inc inc _}
{add _}
```

The first four examples are structurally valid for a scalar operand; ordinary
static primitive checks may still reject a particular operand type or arity.
The last example uses `_` as the complete operand of prefix application and
therefore follows FARAWEAVE-SPEC-0006's static one-level spreading rule when the
fan-out operand type is `Tuple`.

A tuple literal, tuple constructor, fan-out, vector literal, or any later owned
aggregate constructor may not occur on the ancestor path from `_` to the branch
root. Such a position would require a policy for copying or retaining the
borrowed operand and is rejected as `placeholder_owned_position`:

```text
{inc[[_]]}          invalid: placeholder is a tuple element
{add [1 _]}         invalid: placeholder is a tuple element
{inc[fanout[_ ...]]} invalid independently as nested fan-out and owned use
```

The first applicable structural reason under section 3.5 wins. Ordinary tuple
literals and calls elsewhere in a branch remain legal when they do not contain
`_`, subject to the exactly-one occurrence rule.

### 4.3 Primitive-call root and independently owned result

After removing outer branch whitespace, the complete branch body must be one
adjacent or prefix primitive call. A bare placeholder, literal, parameter,
tuple, vector, or fan-out is not a branch root:

```text
{_}             invalid: branch root is not a primitive call
{1}             invalid: branch root is not a primitive call
{[1 _]}         invalid: branch root is not a primitive call
```

Every successful branch root must return one complete uniquely owned Faraweave
value under the ordinary primitive-result contract. It must not return a view
or alias into any argument. This prevents the fan-out result from retaining the
borrowed operand and lets each result outlive operand cleanup. A future
pass-through primitive, borrowed result, or user function requires a separate
aliasing and copy policy before it may serve as a branch root.

### 4.4 Nested calls and source span

The placeholder token has its own one-byte source span. Nested primitive calls
use the same borrowed operand view; there is no reevaluation at the placeholder.
Since exactly one occurrence exists, one branch cannot consume or inspect the
operand through two separate argument paths.

Primitive lookup, arity, type, conversion, shape, resource, and domain rules are
unchanged. The placeholder introduces a typed argument origin; it does not add
an overload, conversion, destructuring rule, or primitive implementation.

## 5. Stored spans and result provenance

### 5.1 Fan-out and branch spans

A parsed fan-out retains:

- the complete expression span from `f` through the closing `]`;
- the `fanout` keyword span;
- the opening- and closing-bracket spans;
- the complete operand expression span;
- one complete branch span per branch, from `{` through `}`;
- each branch's opening- and closing-brace spans;
- each branch body primitive-call span; and
- its one placeholder span after structural validation.

Leading and trailing separators are excluded from the operand, branch-body, and
placeholder spans. The complete branch span includes its braces and internal
separator whitespace.

### 5.2 Static origins

Runtime values store no source spans. The typed program or emitted immutable
metadata owns all origins.

The fan-out expression's tuple type has one outer-element origin per branch.
Element `i` uses complete branch span `i` as its producer origin. Consequently,
when a fan-out result is later spread by prefix application, an error on
expanded argument `i + 1` uses branch `i` as the primary origin and retains the
complete fan-out expression as related context. When the whole fan-out tuple is
passed through an adjacent direct call, that one argument uses the complete
fan-out expression span, because no expansion occurred.

Within a branch, the placeholder is a forwarding origin:

- when it supplies one scalar, vector, or direct tuple argument, its `_` span is
  that argument's primary origin and the original operand span is related
  producer context;
- when it is the complete prefix operand of tuple type, the operand's existing
  outer-element origins remain the primary origins of the expanded arguments,
  while `_`, the branch, and the complete operand span are related context; and
- a nested primitive result uses that primitive call's ordinary producer span
  when consumed by its parent.

This preserves both the source that produced a tuple element and the explicit
branch use that forwarded it. Backends must not reconstruct origins from runtime
node indexes, canonical text, or payload addresses.

### 5.3 Dynamic diagnostic locations

A failure while evaluating the operand retains the operand operation's ordinary
primary span and adds the complete fan-out span as context. The result-table
`ResourceError` uses the `fanout` keyword as primary and carries the complete
fan-out and operand spans. A branch primitive failure uses its ordinary
primitive/call/argument primary span and carries zero-based branch index,
complete branch span, placeholder span when relevant, operand span, and complete
fan-out span.

Failure cleanup never replaces the selected error or its provenance.

## 6. Complete static validation before execution

### 6.1 Static result type

Typed analysis first obtains the operand's complete structural type without
executing it. Each branch's placeholder has exactly that type. After ordinary
primitive analysis derives branch result types `R0` through `Rn-1`, the fan-out
expression has this static type:

```text
Tuple<R0, R1, ..., Rn-1>
```

Branch result types may differ. A branch result that is itself a tuple remains
one nested tuple element; result-type construction never recursively flattens
it. The initial primitive table returns scalars or homogeneous vectors, but the
rule remains exact if a separately specified primitive later returns a tuple.

### 6.2 Analysis dependencies and candidate order

Fan-out participates in FARAWEAVE-SPEC-0005 and FARAWEAVE-SPEC-0006 whole-program
analysis with this exact order:

1. validate source bytes, tokens, delimiters, headers, fan-out/branch structure,
   nesting, root forms, and placeholder count/placement for the complete program;
2. validate declaration counts and representation;
3. resolve declarations, parameter references, and every primitive name,
   including names in every branch;
4. analyze expression dependencies in left-to-right postorder without executing
   values, obtaining each fan-out operand type before substituting that type at
   its branch placeholder;
5. select the first available primitive arity candidate across all roots;
6. if none exists, select the first type/signature candidate;
7. if none exists, select the first statically knowable shape candidate; and
8. construct every fan-out result tuple type and origin sidecar.

Within one static category, roots are in source order, ordinary calls are in
left-to-right postorder, fan-out operands precede their branches, branches are
in increasing source index, and calls within one branch are in left-to-right
postorder. A candidate requiring the placeholder type is unavailable until the
operand has a valid static type, exactly as a tuple-spreading parent candidate
is unavailable until its operand type exists under FARAWEAVE-SPEC-0006.

An independent arity candidate in another root or branch still wins over a type
candidate. A structural, resolution, or type failure that prevents the operand
type wins over a dependent branch candidate that cannot yet be formed. An
implementation may fuse passes only if it produces the same winner.

### 6.3 All branches before any dynamic work

Every branch, including a branch after one that will predictably fail at
runtime, is parsed, resolved, and statically validated. No argument binding,
resource-context creation, operand evaluation, tuple-table admission, or branch
execution begins until the complete program passes all static phases.

For example, in a valid surrounding program shape, a branch-0 integer overflow
predicted from literals does not hide an unknown primitive, wrong arity, type
error, or static shape mismatch in branch 1. The later static error is returned
before runtime. Once static analysis succeeds, section 7's dynamic order applies
and no later branch is speculatively executed to discover another dynamic
failure.

## 7. Runtime sequence and first dynamic failure

### 7.1 Exact success sequence

After complete static success and ordinary argument binding, one fan-out executes
in this exact order:

1. evaluate the operand expression completely, exactly once;
2. retain its complete unique owner as the shared immutable operand owner;
3. compute and admit the result tuple table for the statically known branch count
   under FARAWEAVE-SPEC-0006 profile-v2 rules;
4. obtain the complete positive-size table allocation;
5. for branch 0, instantiate its placeholder as a borrowed view of the operand
   root and execute the complete primitive-call template;
6. after branch 0 succeeds, move its complete result owner into result element 0
   through an infallible transfer;
7. repeat steps 5 and 6 for each later branch in increasing source order;
8. after the final result transfer and all branch-internal borrowed views end,
   destroy the shared operand owner at its logical last use; and
9. publish the complete owned result tuple to its parent or root slot.

The result table has `branch_count` immediate slots and is admitted before any
branch operation. Branch-result transfer adds no allocation, charge, work, copy,
or failure point. An implementation must prepare all owner metadata needed for
all slots as part of the one table attempt; it may not introduce a fallible step
between a successful branch result and transfer of that result.

The operand expression is not cloned, reevaluated, reformatted, or charged per
branch. Each placeholder borrow ends before the next branch begins. The operand
owner itself remains unchanged and alive through all branches.

### 7.2 Dynamic winner order

After static success, the first dynamic failure is selected in this sequence:

1. operand dynamic failure;
2. result-table sizing, profile-limit, or allocation failure;
3. branch 0's left-to-right postorder dynamic failure;
4. branch 1's dynamic failure; and
5. each later branch in source order.

Within an operand or branch primitive call, ordinary shape, resource, domain,
and lowest lifted-result-index rules remain unchanged. A branch is executed to
completion before the next branch begins. No parallel, speculative, interleaved,
or out-of-order execution may expose a different charge, allocation ordinal,
error, or result.

A failed branch produces no result owner. No later branch begins. Work charged
by the operand and earlier successful branches is monotonic and is not refunded.

### 7.3 Scalar, vector, empty-vector, and tuple operands

The same sequence applies to every operand kind:

- a scalar is borrowed as one scalar value in each branch;
- a vector, including an empty vector, is borrowed with its one existing payload
  and is never copied per branch;
- a tuple is borrowed with its existing table and transitive payload owners and
  is never copied or destructured merely because it is a fan-out operand; and
- nested tuples remain nested.

Ordinary primitive semantics determine each branch result. A lifted branch over
an empty vector charges zero primitive work and returns its independently owned
empty-vector result under the existing zero-size reservation rules. Two branches
that produce equal values still own distinct logical results; equality does not
coalesce ownership or charges.

## 8. Transactional ownership and cleanup

### 8.1 Operand failure

If operand evaluation fails, its own operation performs ordinary cleanup. No
fan-out result table exists, no branch begins, and the fan-out returns that exact
failure. Any monotonic work already charged inside the operand remains charged.

### 8.2 Result-table refusal or allocation failure

After operand success, a result-table sizing overflow, profile refusal, or
allocation failure creates no table reservation and starts no branch. The
operand owner is then destroyed. The selected `ResourceError` is returned
unchanged. Because at least one branch is required, this table is positive size
and every admitted table attempt receives one profile-v2 allocation ordinal.

### 8.3 Branch failure

If branch `i` fails:

1. the branch's ordinary primitive machinery destroys every branch-local
   temporary and selects its one failure;
2. already transferred results `i - 1` through 0 are destroyed in reverse result
   order, each with FARAWEAVE-SPEC-0006 reverse transitive cleanup;
3. after all initialized result slots are empty, the result tuple-table
   reservation is released;
4. the shared operand owner is destroyed after no placeholder view remains; and
5. the original branch failure is returned with no partial tuple.

Uninitialized slots are never validated or destroyed as values. The table's
construction state and initialized prefix count are internal checked data, not a
public partial `Tuple`. Cleanup is allocation-free, infallible, iterative for
nested values, and cannot replace the winning failure.

### 8.4 Success ownership and later destruction

On success, every result has moved exactly once into its corresponding table
slot and no branch temporary remains an owner. The shared operand is destroyed
before the result is published, so the result cannot alias or retain it. The
result is thereafter an ordinary FARAWEAVE-SPEC-0006 tuple: evaluator detachment,
formatting, root publication, reverse destruction, and live-byte release follow
that specification.

Tuple result elements remain in branch order. Destruction later runs from the
last branch result toward branch 0 and releases each child's transitive payload
before the outer table. A nested tuple branch result remains one child and is not
flattened during transfer, formatting, spreading, or cleanup.

## 9. Interaction with spreading and direct calls

### 9.1 Placeholder use inside a branch

The placeholder obeys the call syntax at its exact use:

```text
fanout[[1 2] {add _}]       prefix use: spread operand outer tuple once
fanout[[1 2] {add[_]}]      direct use: preserve operand as one tuple argument
fanout[(1 2) {inc[_]}]      direct vector argument: ordinary vector lifting
fanout[Int() {inc[_]}]      direct empty-vector argument: ordinary empty lifting
```

For the current primitive table, the first example succeeds with result `[3]`.
The second is statically invalid because `add` receives one direct tuple
argument. No fallback spread occurs after that type or arity result. The vector
examples use one borrowed vector payload and produce independent vector results.

Prefix expansion is based only on the placeholder's static type and is exactly
one level. A nested tuple element remains one argument. Direct application never
spreads. Fan-out does not add tuple-aware primitive signatures or destructuring.

### 9.2 Fan-out result consumed by another call

A fan-out expression has ordinary tuple type. FARAWEAVE-SPEC-0006 therefore applies
without special cases:

```text
add fanout[1 {inc[_]} {add[_ 2]}]
```

The fan-out produces `[2 3]`; the outer prefix `add` evaluates that operand once
and spreads its outer elements once. In contrast:

```text
add[fanout[1 {inc[_]} {add[_ 2]}]]
```

supplies one preserved tuple argument to the direct call and is rejected by the
current primitive table after ordinary arity-first checking. The direct call
does not flatten or inspect branch results to reinterpret its argument list.

### 9.3 One branch and nested result tuples

A one-branch result remains `Tuple<R0>`. Prefix consumption spreads it to one
argument; direct consumption preserves one singleton tuple argument. If a
separately specified primitive can return a tuple from a branch, the fan-out
outer table stores that complete tuple as one element. Prefix consumption of the
fan-out result spreads only the fan-out's outer branch sequence, not the nested
branch tuple.

## 10. Profile-v2 charges, ordinals, and fault behavior

### 10.1 Required profile identity

Every `fanout_expression` is tuple-producing. It requires `trusted-local-v2` or
`bounded-v2` exactly as FARAWEAVE-SPEC-0006 requires for tuple expressions. Selecting
`trusted-local-v1` or `bounded-v1` returns
`ProfileError(unsupported_value_kind)`, `value_kind = Tuple`, at the earliest
fan-out expression before argument binding, context creation, operand execution,
or any charge.

A tuple-capable implementation defaults to `trusted-local-v2`. Fan-out does not
add a profile name, limit key, canonical byte unit, or resource-error reason.

### 10.2 Work charges

The fan-out construct itself charges zero work for:

- borrowing the operand;
- table sizing and construction;
- branch dispatch;
- placeholder substitution;
- result transfer; and
- cleanup.

The operand expression charges its ordinary work once. Each executed branch
primitive and nested primitive charges ordinary work in dynamic execution order.
A later failure does not refund operand or completed-branch work. An unstarted
branch charges no work.

### 10.3 Live bytes and table admission

For `n` branches, the result table uses the existing canonical event:

```text
fanout_result_tuple_table_bytes(n) = n * 16
```

It is one ordinary tuple-table reservation subject first to
`max_tuple_table_bytes`, then `max_live_evaluation_bytes`, and it charges no work.
The checked admission occurs after operand success and before branch 0. The
operand's vector/tuple reservations remain live during that request and every
branch. Each successful branch result's own vector or tuple reservations become
live through ordinary branch operations and remain live after transfer. Moving
a result into the table does not charge its transitive payload again.

On success, operand live charges release after the final transfer; the table and
branch-result charges remain live as the published tuple. On failure, section 8
fixes reverse result, table, then operand release order. Physical storage reuse
or coalescing may not alter these logical events.

### 10.4 Allocation ordinals

Profile v2 assigns ordinals in this exact semantic order:

1. positive admitted allocations performed while evaluating the operand;
2. the positive fan-out result-table attempt;
3. positive admitted allocations in branch 0;
4. positive admitted allocations in each later branch in source order.

Because `n >= 1`, an admitted fan-out result table always has positive canonical
size and receives one ordinal. A size overflow or profile-refused table receives
no ordinal and prevents all branch attempts. Zero-size vector or tuple results
inside a branch retain FARAWEAVE-SPEC-0006's no-ordinal rule. One result-table
ordinal covers all fallible table and adoption metadata needed to hold all
branch slots; no transfer-time ordinal is permitted.

An injected failure at the table ordinal returns
`ResourceError(allocation_unavailable)` before branch work and then destroys the
operand. A failure at a branch allocation ordinal stops that branch, destroys
the completed result prefix, releases the table, and destroys the operand. No
later ordinal occurs. The error carries profile name, ordinal, requested branch
count, canonical table bytes when the table is the producer, and section 5's
source context.

### 10.5 Cross-backend resource observability

Evaluator, runner, emitted-C, and native execution claiming the same exact v2
profile identity must emit the same canonical work and live-byte events, request
the same allocation ordinals, select the same injected fault, and perform the
same logical releases. Constant folding, fusion, stack placement, table
coalescing, storage reuse, or physical allocation elision may not change any of
those observations.

Natural host allocation availability may differ only as FARAWEAVE-SPEC-0004 already
allows. Injected allocation failures and all profile-limit outcomes are exact
cross-backend requirements.

## 11. Runtime values, presentation, and atomic output

The fan-out result is an ordinary valid `OwnedValueArena` tuple under
FARAWEAVE-SPEC-0006. Public validation, type queries, tuple arity, borrowed element
access, canonical formatting, and destruction require no fan-out-specific
runtime tag.

Canonical formatting preserves branch order:

```text
fanout[1 {inc[_]}]                    [2]
fanout[1 {inc[_]} {add[_ 2]}]         [2 3]
fanout[(1 2) {inc[_]} {add[_ 10]}]    [(2 3) (11 12)]
```

The source on the left is illustrative Faraweave input; the bytes on the right are
the resulting canonical values. A nested tuple result remains bracketed within
its outer element.

The public evaluator returns either the complete owned tuple or one structured
failure and performs no I/O. Runner and artifact surfaces retain
FARAWEAVE-SPEC-0005's execute-all, format-all, publish-once behavior. An operand,
table, branch, later-root, or formatting failure publishes zero stdout bytes and
performs complete reverse cleanup. Only the existing external `OutputError`
after publication begins may expose a byte prefix.

## 12. Initial nesting boundary

A fan-out may appear anywhere an ordinary expression is accepted, including as
a root, tuple element, adjacent call argument, or prefix operand. Several
fan-outs may be siblings when none contains another.

The initial language rejects a `fanout_expression` lexically contained anywhere
inside another fan-out's operand or branch body. The outer construct reports
`nested_fanout` at the inner `fanout` keyword and retains the outer fan-out span
as context:

```text
fanout[fanout[1 {inc[_]}] {inc[_]}]    invalid nested fan-out
fanout[1 {inc[fanout[2 {inc[_]}]]}]    invalid nested fan-out
[fanout[1 {inc[_]}] fanout[2 {inc[_]}]] valid sibling fan-outs
```

This boundary avoids nested placeholder capture, interleaved partial result
tables, and a second level of branch-failure ordering in the initial construct.
It is a static structural rejection before operand evaluation. A later nesting
specification must define placeholder scope, provenance, table/ordinal order,
and cleanup composition before removing this restriction.

## 13. Deliberate differences from Anka

[Anka](https://github.com/tuncb/anka) is a design cue, not a compatibility
target. Faraweave deliberately differs as follows:

- the construct is named `fanout`, not `executor` or `fork`, because execution is
  sequential and carries no parallel promise;
- every branch is enclosed in its own braces; Faraweave does not use Anka bars,
  adjacency, word folding, or inferred primitive arity to group branches;
- each branch contains exactly one explicit `_` placeholder rather than an
  implicit current value, repeated holes, or connected-tuple behavior;
- a branch root is one primitive call, not a bare identity, block, user function,
  callable value, curried expression, or partial application;
- placeholder flow borrows one immutable operand owner and cannot place that
  borrow into an owned tuple or pass-through result;
- branches run left to right, never in parallel or in an unspecified executor
  order;
- the result is Faraweave's existing structural tuple with explicit profile-v2
  table accounting, not an Anka connected tuple; and
- ordinary Faraweave one-level prefix spreading and direct-call preservation remain
  visible rather than importing Anka's placeholder, currying, block, or
  connected-tuple application rules.

No Anka parser, executor, ownership, failure, or resource behavior constrains
this contract.

## 14. Complete valid and invalid examples

### 14.1 Valid structure

```text
fanout[1 {inc[_]}]
fanout[1 {inc[_]} {add[_ 2]}]
fanout[(1 2 3) {inc[_]} {equals[_ (1 2 3)]}]
fanout[Int() {inc[_]} {add[_ 2]}]
fanout[[1 2] {add _} {equals _}]
fanout[inc 1 {inc[_]} {inc[add[_ 1]]}]
add fanout[1 {inc[_]} {add[_ 2]}]
add[fanout[1 {inc[_]} {add[_ 2]}]]
```

A structurally valid example may still fail ordinary static primitive checks.
In particular, `equals _` spreads a two-element tuple and is valid for the
initial binary primitive; a tuple of another arity has an `ArityError` after
static expansion. The final direct-call example preserves one tuple and is
rejected by the current primitive table.

### 14.2 Invalid structure and required outcome

| Source shape | Required failure |
| --- | --- |
| `fanout [1 {inc[_]}]` | `expected_fanout_open` at the space. |
| `fanout[]` | `expected_fanout_operand` before `]`. |
| `fanout[1]` | `expected_fanout_branch` before `]`. |
| `fanout[1 inc[_]]` | `expected_branch_open` at `inc`. |
| `fanout[1 {}]` | `expected_branch_body` before `}`. |
| `fanout[1 {inc[_]]` | Mismatched branch close at `]`. |
| `fanout[1 {inc[_]}` | `missing_fanout_close` at end of source. |
| `fanout[1 {inc[_]}{inc[_]}]` | Missing separator before the second `{`. |
| `fanout[1 {_}]` | `branch_root_not_primitive_call`. |
| `fanout[1 {inc[1]}]` | `placeholder_count`, actual 0. |
| `fanout[1 {add[_ _]}]` | `placeholder_count`, actual 2. |
| `fanout[1 {inc[[_]]}]` | `placeholder_owned_position` at `_`. |
| `fanout[fanout[1 {inc[_]}] {inc[_]}]` | `nested_fanout` at the inner keyword. |
| `_` | Placeholder outside branch syntax. |

Missing and mismatched delimiter spans follow FARAWEAVE-SPEC-0002. The table fixes
the fan-out-specific structural classification; it does not permit recovery
that executes a valid prefix of the source.

ASCII bar is still an invalid byte. For example, `fanout[1 | inc[_]]` does not
use a bar as a branch delimiter and reports the ordinary invalid-byte failure.

### 14.3 Static and dynamic ordering examples

```text
fanout[9223372036854775807
       {inc[_]}
       {unknown[_]}]
```

The unknown primitive is a static failure. It wins before the operand or branch
0 can execute, even though branch 0 would overflow dynamically.

```text
fanout[9223372036854775807
       {inc[_]}
       {add[_ 1]}]
```

After static success and table admission, branch 0 returns
`DomainError(integer_overflow)`. Branch 1 never begins.

```text
fanout[iota[3]
       {inc[_]}
       {add[_ 10]}]
```

The operand vector is allocated and charged once. The table is admitted next.
Branches run left to right and produce two independently owned vectors. The
canonical result is `[(2 3 4) (11 12 13)]`.

## 15. Backend and implementation conformance boundary

A conforming implementation may choose a flat node opcode, branch-range arena,
preallocated result-slot table, generated labels, or equivalent plain-data
representation. It must preserve:

- one parsed operand index and one ordered branch range;
- one placeholder site and input-type substitution per branch;
- complete static validation before execution;
- one operand owner and temporary checked borrows;
- one result table admitted before branch work;
- one initialized-prefix count during private construction;
- sequential branch execution and exact failure cleanup;
- branch-span result origins without runtime spans; and
- all profile-v2 semantic events independent of physical representation.

The evaluator and code generators consume the same validated branch order and
static primitive selections or equivalent immutable semantic data. Generated
strict C11 must not add runtime overload lookup, host recursion for nested tuple
cleanup, `sizeof`-derived charges, reference counting, or a parallel runtime.

## 16. Non-goals and fixed follow-ups

This specification does not add:

- parallel, concurrent, speculative, or reordered branch execution;
- zero-branch fan-out;
- implicit branch grouping, Anka bars, or adjacency-based branch inference;
- bare identity, pass-through, literal, tuple-constructor, or vector-constructor
  branch roots;
- repeated, optional, named, destructuring, or pattern placeholders;
- copying or retaining the shared operand in a result;
- tuple destructuring or recursive flattening;
- tuple-aware primitive signatures;
- first-class callable values, callable-expression targets, user functions,
  closures, blocks, currying, or partial application;
- nested fan-out;
- runtime reference counting, shared ownership, or copy-on-write;
- a new profile, limit, charge unit, allocation-error reason, or ordinal stream;
  or
- optimization that changes an observable table admission, allocation ordinal,
  work/live event, failure, provenance, branch order, or lifetime.

Each requires separate specification and acceptance. In particular, bare
identity and tuple-constructor branches require an explicit result-aliasing or
copying policy; nesting requires composed placeholder, provenance, and cleanup
rules; and parallel execution requires a different deterministic resource and
failure contract.

## 17. Requirement-to-test plan

Issue #44 records the normative contract and plan only. Implementation Issue
#53 and any prerequisite implementation issues must add executable tests and
exact identifiers to `tests/spec-traceability.tsv`. Supported target-native
journeys are Linux x64, Windows x64, and macOS arm64. Strict C11 means
GCC/Clang-compatible `-std=c11 -Wall -Wextra -Werror -pedantic-errors` and
Windows MSVC `/std:c11 /W4 /WX` where applicable.

| Plan ID | Normative requirement | Required implementation evidence |
| --- | --- | --- |
| `FAN-001-GRAMMAR` | Sections 3 and 14 exact keyword, brackets, braces, separators, branch boundaries, one-or-more count, and structural reasons/spans. | Parser fixtures cover compact/multiline LF/CRLF forms, one/many branches, nested ordinary expressions, every invalid table row, missing/mismatched delimiters, bars/punctuation, and unique flat operand/branch ranges with exact spans. |
| `FAN-002-PLACEHOLDER` | Section 4 branch-local exactly-one `_`, primitive-call ancestor path, root restriction, and no owned placement. | Static fixtures cover zero/one/two occurrences, each legal direct/prefix/nested-call position, tuple/vector/aggregate retention attempts, bare roots, scope isolation between branches, `_` outside fan-out, and exact reason/count/span. |
| `FAN-003-STATIC-ALL-BRANCHES` | Section 6 complete program and all-branch validation before runtime. | Multi-branch/root fixtures hide unknown names, arity, type, and static shape defects after predicted operand/branch dynamic failures; direct evaluator, run, emit-c, and build select the same static winner with zero context, charge, or operand execution. |
| `FAN-004-STATIC-TYPE-ORDER` | Operand-type dependency, placeholder substitution, branch ordering, and `Tuple<R...>` result without flattening. | Typed-analysis fixtures combine operand errors, independent/dependent branch candidates, branch postorder, unlike branch types, singleton and nested tuple result types, and cross-root arity/type/shape precedence. |
| `FAN-005-OPERAND-ONCE` | Sections 7 and 9 one complete operand owner and immutable no-copy borrows. | Instrumented evaluator/C/native probes count operand calls, vector/tuple allocations, payload identities, borrow windows, and charges across one/many branches; prove one evaluation, no per-branch operand payload copy, no mutation, and no surviving borrow. |
| `FAN-006-TABLE-BEFORE-BRANCH` | Result table admitted after operand success and before branch 0. | Event traces and tiny limits prove operand events first, one `n * 16` table request next, no branch work on table overflow/profile/allocation failure, and one-branch positive table behavior. |
| `FAN-007-SEQUENTIAL-FIRST-FAILURE` | Left-to-right complete branches and first dynamic failure. | Domain, dynamic-shape, resource, and injected-fault combinations in each branch position prove stable winner, no later branch start, no interleaving/speculation, ordinary lowest kernel index, and matching evaluator/C/native traces. |
| `FAN-008-TRANSFER-CLEANUP` | Independently owned result transfer, initialized-prefix state, reverse result/table/operand cleanup, and success ownership. | Instrumented nested vector/tuple results fail at operand, table, every branch before/after allocations, formatting, and later roots; assert each owner/reservation releases once in exact order, no partial public tuple, no alias, no transfer failure, and ASan/UBSan cleanliness. |
| `FAN-009-OPERAND-KINDS` | Scalar, vector, empty-vector, tuple, and nested-tuple behavior without implicit destructuring/copy. | Differential journeys cover each kind, vector lifting, zero-work empty results, tuple prefix placeholder spreading, direct tuple rejection, nested tuple type failures, and independently owned equal results. |
| `FAN-010-SPREAD-DIRECT` | Section 9 placeholder and outer-consumer one-level spreading versus direct preservation. | Static/runtime fixtures cover `add _`, `add[_]`, outer `add fanout[...]`, direct `add[fanout[...]]`, singleton results, nested tuple branch results, exact arity/type winners, and branch/placeholder/operand origins. |
| `FAN-011-PROFILE-EVENTS` | Section 10 v2-only identity, zero construct work, live lifetimes, fixed table charge, no transitive double charge, and release events. | Bounded exact-limit/one-past/overflow traces cover every limit with scalar/vector/tuple operands and vector/tuple results; v1 refusal precedes binding/context; evaluator/C/native event sequences and usage-before values match without host `sizeof`. |
| `FAN-012-ALLOCATION-ORDINALS` | Operand, table, then branch positive-admission ordinal sequence and deterministic faults. | Fault injection targets every ordinal with zero-size/profile/overflow exclusions, proves table fault before branch, branch fault cleanup/no-later ordinal, one semantic table attempt despite physical layout, and exact structured context across backends. |
| `FAN-013-PROVENANCE` | Section 5 construct/operand/branch/placeholder/result origins without runtime spans. | Parser/IR/emitted metadata tests assert every stored span; static/dynamic diagnostics select operand, `_`, forwarded tuple child, branch result, or whole direct tuple exactly; moved runtime values remain source-independent. |
| `FAN-014-NESTING-BOUNDARY` | Section 12 sibling composition and static rejection of lexical nested fan-out. | Parser/analysis fixtures cover nesting in operand and deep branch calls, sibling fan-outs in tuples/roots, fan-out under ordinary direct/prefix calls, exact inner-keyword error/context, and zero runtime work on rejection. |
| `FAN-015-ATOMIC-OUTPUT` | Complete tuple publication and existing program-level output transaction. | Runner/C/native programs combine earlier roots with operand, table, branch, later-root, and formatting failures; assert nonzero status, exact structured failure, zero stdout, and complete cleanup. Existing short-write/flush remains the sole publication-prefix exception. |
| `FAN-016-STRICT-C-NATIVE` | Section 15 portable flat lowering and backend agreement. | Deterministic emitted bytes, strict C11 compilation, native success/error/fault runs, generated-source inspection for sequential control flow/no runtime overload/no reference counting/no host recursion/no `sizeof` charge, and differential event/result comparison. |
| `FAN-017-REGRESSION-PLATFORMS` | Unchanged ordinary calls, tuples, profiles, parameters, and supported targets. | Complete Release, strict/no-exceptions, sanitizer, and Linux/Windows/macOS suites retain existing behavior; the fan-out corpus agrees across evaluator, CLI, emitted C, and native. |

No one surface substitutes for another. Public C++ tests establish ownership and
structured data; analyzer tests establish complete static validation; evaluator
tests establish value and event order; runner tests establish diagnostics and
atomic output; emitted strict C11 and native tests establish portable sequential
cleanup; fault seams establish every allocation boundary; and the platform
matrix establishes target-independent behavior.
