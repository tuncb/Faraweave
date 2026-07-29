# Issue #36 — container-wide application plans

Semantic FWIR 1.1 adds mandatory feature 5, mandatory section 17, and a stable application-plan identity whose registry descriptor records operand consumption, result cardinality, and work admission. Existing 1.0 artifacts omit the section and reconstruct plans from validated implementation IDs, preserving canonical bytes and semantics. Whole-vector operands are distinct from elementwise lifting and permit no implicit container conversion; downstream operations append their own semantic IDs without this issue reserving concrete operations.
