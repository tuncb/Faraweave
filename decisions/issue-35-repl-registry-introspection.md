# Issue 35: REPL registry introspection

`.internal` delegates to a registry-owned diagnostic writer so the REPL cannot drift into a second primitive catalog. The writer validates and orders the production descriptors by stable IDs before emitting type, conversion, lifting, structural, plan, and implementation data. Its output is explicitly human-readable and unstable, while allocation, validation, write, and flush failures remain recoverable.
