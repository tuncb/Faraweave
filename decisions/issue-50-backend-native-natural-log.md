# Issue #50 — backend-native natural `log`

Primitive ID 31 uses signature and implementation IDs 37 with the existing unary Double elementwise plan, Int promotion, semantic 1.1, and mandatory feature 7. Rust calls `f64::ln` and generated C calls `<math.h>` `log` inside the strict floating environment, canonicalizing domain NaNs without observing `errno` or exception flags. Finite results use the backend-native four-ULP envelope while signed-zero boundaries, special values, resources, diagnostics, ownership, cleanup, and publication remain exact.
