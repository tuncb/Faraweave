# Issue #5 — source-to-typed-IR lowering

Lowering accepts parsed source structure and static types only, records every selected semantic identity, conversion, cardinality, origin, shape plan, and ownership release, then publishes only a verified `TypedProgram`. Whole-program analysis selects name, arity, type, and shape failures before construction, while fan-out substitutes operand types through immutable placeholder borrows without runtime values. Deep syntax is lowered iteratively, and every raw arena insertion remains fallible through the existing builder refusal seam.
