# Issue #14 — FWIR compatibility and conformance

The checked-in canonical corpus is identified by exact length and FNV-1a bytes, while a machine-readable table maps every v1 field and compatibility rule to executable positive and negative evidence. Hostile artifacts are generated deterministically from those canonical bytes so the corpus stays reviewable without storing redundant binaries. Public backend-gating, cross-surface parity, strict C11, native, sanitizer, and host-specific journeys remain separate tests because each boundary must fail independently.
