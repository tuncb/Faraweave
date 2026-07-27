# Issue #7 — direct evaluation IR cutover

All public direct-evaluation surfaces now compile source to a verified program and execute only that program, while expression-only validation remains a pre-compilation surface check. Typed and textual argument validation derives parameter metadata from verified IR, and formatting remains a distinct phase after successful execution. The recursive AST evaluator and its runtime overload-selection entry point are deleted so static analysis is the sole semantic-selection authority.
