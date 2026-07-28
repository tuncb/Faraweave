# Issue #11 — canonical FWIR v1 encoder

The encoder preflights every count, section length, absolute offset, and whole-artifact size before fallibly reserving the sorted string pool and final byte vector. Writer failures are explicit, while the atomic-publication seam exposes the complete artifact only after encoding succeeds and delegates the indivisible commit to its callback. The issue #10 scalar example is corrected from impossible zero-based offsets 0..4 to the verified semantic contract's canonical one-based offsets 1..5, without transforming origins in the encoder.
