# Container-wide application-plan contract

**Status:** Accepted for implementation by issue #36.

This additive contract extends semantic FWIR 1.1 without defining any
aggregate, reduction, sort, fold, scan, filter, or multidimensional value.
The requirements below supplement the semantic and physical FWIR v1
specifications.

## Registry representation (`FWIR-PLAN-001`)

Every semantic signature records each operand's scalar element type and one
consumption mode:

```text
Elementwise
WholeVector
```

`Elementwise` retains scalar broadcast and same-length lifting. `WholeVector`
requires a homogeneous vector value of the recorded element type, does not
broadcast, does not participate in elementwise shape anchoring, and permits
only identity conversion; this makes a container-wide call impossible to
reinterpret as scalar lifting.

Every registry row also selects a stable application-plan ID. A plan records
one result-cardinality rule and one resource-work rule:

```text
Result = Elementwise
       | Scalar
       | DynamicVector
       | PreserveOperand(one-based position)
       | OperandPlusOne(one-based position)

Work   = Constant(n)
       | ResultCardinality
       | OperandCardinality(one-based position)
```

`PreserveOperand` and `OperandPlusOne` may reference only a `WholeVector`
operand. Checked overflow of `OperandPlusOne`, result byte sizing, prospective
live bytes, work, allocation ordinal, admission commit, physical allocation,
kernel execution, and cleanup retain the failure order in `FWIR-SEM-014`.
Work is admitted in full before implementation execution, so short-circuiting
or host algorithm choices cannot change resource observations.

## Typed-program and backend boundary (`FWIR-PLAN-002`)

`SelectedApply` records the lowering-selected primitive, signature,
implementation, and application-plan IDs. The verifier reconstructs the
registry row from those IDs and validates operand consumption, element types,
conversion, result type, result cardinality, lift/container mode, shape
sidecars, resource plan references, and required features before any backend
runs.

The interpreter and C emitter consume the verified implementation and
application-plan IDs directly. They must not use a source name to recover
container semantics, repeat overload selection, treat a whole vector as a
sequence of scalar calls, or derive a different work request.

Application-plan IDs are a distinct stable identity domain. IDs `1` and `2`
mean respectively the existing elementwise/result-cardinality plan and the
existing dynamic-vector/result-cardinality plan used by `iota`; downstream
operations append plan meanings without renumbering or reinterpreting them.
Issue #36 allocates no new primitive, signature, or implementation ID.

## Version and encoding compatibility (`FWIR-PLAN-003`)

Mandatory feature ID `5=ApplicationPlans` requires semantic minor 1 and
physical format minor 1. With that feature, mandatory section
`17=APPL` contains one 8-byte record per SelectedApply in ascending node order:
`node:u32`, `application_plan_id:u16`, and `reserved:u16`. Every SelectedApply
has exactly one nonzero plan record and the reserved field is zero.

`NODE.a7` remains zero in every v1 minor. Semantic/format 1.0 artifacts omit
section 17, retain their exact canonical bytes, and keep their previous
behavior. A 1.0 decoder path reconstructs plan ID 1 or 2
from the already validated implementation identity before constructing
`RawProgram`; re-encoding therefore remains byte-identical.

Unknown mandatory feature IDs, unknown application-plan IDs, a plan that does
not match the selected implementation, feature 5 at format or semantic minor
0, section 17 without feature 5, and missing/misordered/duplicate plan records
with feature 5 are rejected deterministically before allocation for execution
or backend dispatch. Existing ownership, cleanup, provenance, diagnostics,
resource observer order, and 1.0 canonical examples are unchanged.

## Vector length (`FWIR-PLAN-005`)

Primitive ID `21=length` has three whole-vector signatures: IDs 37, 38, and 39
accept respectively Bool, Int, and Double vectors and use the matching
implementation IDs. All three select application-plan ID 3, whose result is a
scalar Int and whose work rule is `Constant(1)`; no implicit scalar or element
conversion is permitted.

The implementation borrows the already materialized vector, admits the one
work unit, converts its host cardinality to Int with a checked conversion, and
does not allocate or copy a result container. Empty vectors return zero,
unrepresentable cardinality returns structured `SizeOverflow`, and the input is
released according to its existing ownership edge after success or failure.
Tuple and scalar operands fail during static signature selection.

## Deterministic vector sort (`FWIR-PLAN-006`)

Primitive ID `22=sort` has whole-vector Bool, Int, and Double signatures and
matching implementation IDs 40, 41, and 42. All three select application-plan
ID 4: `PreserveOperand(1)` produces a newly owned vector of the input
cardinality and element type, while `OperandCardinality(1)` admits exactly
`n` work units for input length `n`, independent of host algorithm or
comparison count.

Bool order is `false < true`, Int order is ascending, and Double order is the
total order defined by Rust `f64::total_cmp`. Equivalently, binary64 bits map
to an unsigned key by complementing every negative encoding and setting the
sign bit of every nonnegative encoding; keys compare ascending, so negative
NaNs precede negative infinity, `-0.0 < 0.0`, and positive NaNs follow positive
infinity. Current verified values admit only the canonical positive quiet NaN,
but backends implement the complete key rule and preserve every copied element
bit.

The output vector byte charge and all `n` work units are admitted together
before storage allocation or copying, while the immutable input remains live.
Allocation refusal therefore releases only the completed input; after output
storage succeeds, copying and in-place sorting add no semantic allocation or
failure point. Empty vectors perform the canonical zero-byte, zero-work
request without an allocation ordinal, and tuple or scalar operands fail
during static signature selection.

## Numeric vector sum (`FWIR-PLAN-007`)

Primitive ID `23=sum` has whole-vector Int and Double signatures and matching
implementation IDs 43 and 44. Both select application-plan ID 5, whose
`Scalar` result rule returns the input element type and whose
`OperandCardinality(1)` rule admits exactly one work unit per input element
before reduction begins.

Int reduction starts at zero and performs checked addition in increasing
element index. The first overflow returns structured `IntegerOverflow` with
the zero-based failing index and the accumulator/current-element operands;
later elements are not evaluated. Double reduction starts at positive zero
and invokes the strict binary64 addition operation once per element in the
same order, without reassociation, parallel reduction, or a host reduction
routine, thereby preserving signed zero, canonical NaN, infinities,
subnormals, and cancellation.

The scalar result has no byte admission or allocation attempt. A successful
or overflowing nonempty reduction retains all admitted work, a work refusal
occurs before the first arithmetic operation, and existing input ownership
releases after success or failure. Empty Int and Double vectors admit zero work
and return respectively Int zero and positive Double zero; Bool, scalar, and
tuple operands fail during static signature selection.

## Boolean all reduction (`FWIR-PLAN-008`)

Primitive ID `24=all_of` has one whole-vector Bool signature and matching
signature/implementation ID 45. It selects application-plan ID 6, whose
`Scalar` result rule returns Bool and whose `OperandCardinality(1)` rule admits
exactly one work unit per input element before inspection begins.

The result is true exactly when every element is true, with true as the empty
identity. An implementation may physically stop after the first false element,
but the complete cardinality charge commits first; the decisive position
therefore cannot affect work-limit outcomes, allocation ordinals, resource
observer events, or ownership cleanup. The scalar result has no byte admission
or allocation attempt, while scalar, numeric-vector, and tuple operands fail
during static signature selection.

## Boolean any reduction (`FWIR-PLAN-009`)

Primitive ID `25=any_of` has one whole-vector Bool signature and matching
signature/implementation ID 46. It selects application-plan ID 7, whose
`Scalar` result rule returns Bool and whose `OperandCardinality(1)` rule admits
exactly one work unit per input element before inspection begins.

The result is true exactly when at least one element is true, with false as the
empty identity. An implementation may physically stop after the first true
element, but the complete cardinality charge commits first; the decisive
position therefore cannot affect work-limit outcomes, allocation ordinals,
resource observer events, or ownership cleanup. The scalar result has no byte
admission or allocation attempt, while scalar, numeric-vector, and tuple
operands fail during static signature selection.

## Evidence (`FWIR-PLAN-004`)

Registry unit tests cover stable plan lookup and changed operand/plan
meanings. Typed-program tests cover plan reconstruction, feature/minor rules,
and exact malformed fields; codec tests cover byte-identical 1.0 round trips,
explicit 1.1 round trips, unknown plan rejection, and reduced-stack deep
graphs. The full Rust and strict-C11 journeys retain cross-backend value,
failure, resource, and allocation-refusal parity for all existing plans.
Issue #40 additionally maps `FWIR-PLAN-005` to
`vector_length_records_container_plan_for_static_dynamic_and_empty_vectors`,
`length_container_plan_roundtrips_and_dispatches_by_verified_identity`,
`length_charges_constant_work_borrows_input_and_has_no_result_allocation`, and
the strict C11 journey.
Issue #41 maps `FWIR-PLAN-006` to
`vector_sort_records_preserved_container_plan_for_static_dynamic_and_empty_vectors`,
`sort_container_plan_roundtrips_and_dispatches_by_verified_identity`,
`sort_covers_exhaustive_small_bools_integer_edges_and_total_double_order`,
`sort_admits_owned_output_with_input_live_and_cleans_up_refused_output`, the
randomized generated-C differential corpus, and the strict C11 journey.
Issue #42 maps `FWIR-PLAN-007` to
`vector_sum_records_scalar_container_plan_for_static_dynamic_and_empty_vectors`,
`sum_container_plan_roundtrips_and_dispatches_by_verified_identity`,
`sum_int_overflow_reports_the_first_reduction_step_and_operands`,
`sum_double_is_left_to_right_strict_and_preserves_special_value_bits`,
`sum_charges_full_work_before_reduction_and_allocates_no_result`, and the
strict C11 success/failure journeys.
Issue #43 maps `FWIR-PLAN-008` to
`all_of_records_scalar_container_plan_for_static_dynamic_and_empty_vectors`,
`all_of_container_plan_roundtrips_and_dispatches_by_verified_identity`,
`all_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_false_position`,
`all_of_work_and_observer_trace_are_independent_of_the_decisive_position`, and
the strict C11 success journey.
Issue #44 maps `FWIR-PLAN-009` to
`any_of_records_scalar_container_plan_for_static_dynamic_and_empty_vectors`,
`any_of_container_plan_roundtrips_and_dispatches_by_verified_identity`,
`any_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_true_position`,
`any_of_work_and_observer_trace_are_independent_of_the_decisive_position`, and
the strict C11 success journey.
