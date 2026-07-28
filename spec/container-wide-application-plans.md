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

## Evidence (`FWIR-PLAN-004`)

Registry unit tests cover stable plan lookup and changed operand/plan
meanings. Typed-program tests cover plan reconstruction, feature/minor rules,
and exact malformed fields; codec tests cover byte-identical 1.0 round trips,
explicit 1.1 round trips, unknown plan rejection, and reduced-stack deep
graphs. The full Rust and strict-C11 journeys retain cross-backend value,
failure, resource, and allocation-refusal parity for all existing plans.
