# Issue #42 — numeric vector sum

Implementation is based on dependency commit `2d0778cb0bc10cf5dd8ce0add774898c048c6401`, which contains #40 and #41 and must wait for both predecessors before final integration. Primitive 23 appends Int and Double signature/implementation IDs 43–44 and plan 5, returning a scalar while charging the operand cardinality before execution. Int reduces left-to-right from zero with checked overflow at the first failing index, Double reduces from positive zero through the existing strict addition operation, and neither path allocates a result or adds a dependency.
