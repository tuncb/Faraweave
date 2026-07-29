# Issue #57 — backend-native `trunc`

Primitive ID 38 uses signature and implementation IDs 63 with the existing unary Double elementwise plan, Int promotion, semantic 1.1, and mandatory feature 7. Rust calls `f64::trunc` and generated C calls `<math.h>` `trunc` inside the strict floating environment, preserving exact finite toward-zero integral results, signed zero, and infinities while canonicalizing NaN without observing `errno` or exception flags. Large Int inputs are converted to Double before the call, while resources, diagnostics, ownership, cleanup, and publication remain exact.
