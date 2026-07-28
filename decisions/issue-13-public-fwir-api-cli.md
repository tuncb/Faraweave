# Issue #13 — public FWIR API and CLI

FWIR uses explicit source and artifact APIs plus `compile-ir`, `inspect-ir`, `run-ir`, `emit-c-ir`, and `build-ir`; extensions never select a mode. Artifact consumers complete bounded decoding and semantic verification before arguments or backends, and named compilation retains logical source provenance for diagnostics. File-producing commands reject lexical and canonical input/output aliases and publish only completed artifacts atomically. Inspection is deterministic, non-executable text with exact binary64 bits and canonical bytes.
