# Issue #40 — vector length

Primitive ID 21 appends `length` with Bool-, Int-, and Double-vector signature and implementation IDs 37–39, all using application-plan ID 3 for a scalar Int result and one constant work unit. The operation borrows its whole-vector operand, reads its existing cardinality without materializing a copy or result container, and checks conversion to Int after work admission. Tuples and scalars remain static type errors, while an unrepresentable host cardinality is a structured size-overflow resource failure.
