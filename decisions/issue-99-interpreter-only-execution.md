# Issue #99 — interpreter-only execution

Faraweave now executes source and verified FWIR exclusively through the Rust `VerifiedProgram` interpreter; the C emitter, compiler-driven builder, their public APIs, and their CLI commands are removed. Validation retains interpreter, argument, resource, diagnostic, verification, packaging, and release coverage without requiring a C compiler. Atomic CLI artifact publication remains a private CLI concern rather than a native-builder API.
