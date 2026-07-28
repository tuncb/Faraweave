# Issue #48 — backend-native `sqrt`

Primitive ID 29 uses signature and implementation IDs 35 with the existing unary Double elementwise plan, including Int promotion and feature 7 at semantic 1.1. Rust calls `f64::sqrt` and generated C calls `<math.h>` `sqrt` inside the strict floating-environment boundary, then normalizes NaNs without observing `errno` or exception flags. Finite results use the backend-native one-ULP policy while special values, resource accounting, diagnostics, and publication behavior remain exact.
