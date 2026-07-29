# Issue #44 — `any_of` Boolean reduction

Implementation is based on dependency commit `6df9bd34d2dd25744d929fbbb38b8f76f7b8a43e`, which contains #40–#43 and must wait for those predecessors before final integration. Primitive 25 appends signature/implementation ID 46 and application-plan ID 7, returning scalar Bool with false as the empty identity. The full operand cardinality is admitted before optional first-true short-circuiting, so work and observer traces are position-independent; no result allocation or dependency is added.
