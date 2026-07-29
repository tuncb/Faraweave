# Issue #45 — `none_of` Boolean reduction

Implementation is based on dependency commit `ccdde0b4444ec92bbcf86aded0b70cdfbe9fade9`, which contains #40–#44 and must wait for those predecessors before final integration. Primitive 26 appends signature/implementation ID 47 and application-plan ID 8, returning scalar Bool with true as the empty identity and retaining `none_of` diagnostics and resource events. The full operand cardinality is admitted before optional first-true short-circuiting; no result allocation or dependency is added.
