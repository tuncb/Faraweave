# Issue #55 — backend-native `floor`

Primitive ID 36 uses signature and implementation IDs 42 with the existing unary Double elementwise plan, Int promotion, semantic 1.1, and mandatory feature 7. Rust calls `f64::floor` and generated C calls `<math.h>` `floor` inside the strict floating environment, preserving exact finite directed-integral results, signed zero, and infinities while canonicalizing NaN without observing `errno` or exception flags. Large Int inputs are converted to Double before the call, while resources, diagnostics, ownership, cleanup, and publication remain exact.
