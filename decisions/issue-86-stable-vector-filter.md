# Issue #86 — stable unary-predicate vector filter

Primitive 39 appends Bool, Int, and Double filter identities 64–66 plus application plan 11, reusing the verified `OPRF` link for exact total `T -> Bool` predicates. The plan records `SubsetOfOperand(1)`, operand-cardinality work, and `WorkThenResult`, so the interpreter commits work before discovery and admits only the exact stable output while the input remains live. The post-#99 product is interpreter-only; no alternate execution backend, dependency, or unsafe code is added.
