# Issue 90: immutable bindings

`let name = expression` is a source-level declaration resolved lexically before typed lowering, and declarations do not become program roots. Semantic FWIR 1.3 represents each binding, borrow, and ownership move explicitly under mandatory feature 9; physical FWIR 1.3 assigns their stable node and access tags. The interpreter aliases borrowed values and transfers owned values without copying containers.
