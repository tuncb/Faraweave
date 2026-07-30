# Issue 100: release 0.2.0

Faraweave 0.2.0 is prepared only after issues #86–#99 are closed and their shipped changes are present on `main`; #94 is recorded as superseded by the interpreter-only cutover in #99. `Cargo.toml` remains the canonical version source for runtime identity, packages, and provenance, while the tag-trigger workflow independently requires the exact annotated `v0.2.0` tag. The reusable release and packaging workflows stay version-derived so later releases need only update the release-specific gate, contracts, and notes.

Per-target provenance fragments use the tool-enforced `<target>.fragment.json` name throughout production, merge, verification, and cleanup.
