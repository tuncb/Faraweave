# Issue #49 — backend-native `exp`

Primitive ID 30 uses signature and implementation IDs 55 with the existing unary Double elementwise plan, Int promotion, and mandatory feature 7 at semantic 1.1. Rust calls `f64::exp` and generated C calls `<math.h>` `exp` inside the strict floating environment, canonicalizing NaNs without observing `errno` or exception flags. Finite results use the backend-native four-ULP envelope while special values, threshold classifications, resources, diagnostics, ownership, and publication behavior remain exact.
