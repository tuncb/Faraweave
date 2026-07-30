# Issue #93 — typed-empty trivia

Typed empty vector parentheses accept the same spaces, tabs, newlines, and `#` comments as other delimited interiors. Trivia does not alter the vector type, value, lowering, or canonical no-trivia FWIR; non-trivia content and closing-delimiter diagnostics keep their existing behavior.
