# Issue 87: UTF-8 strings

String values are unnormalized, length-delimited UTF-8; literals use double quotes with a small explicit escape set, while command-line parameters remain exact raw Unicode arguments. Equality compares bytes, ordering is unsigned byte lexicographic, scalar `length` counts Unicode scalar values, and vector `length` counts elements. Scalar payload bytes and String-vector descriptors plus payload are charged explicitly, and feature 10 with semantic/physical 1.4 appends signatures and implementations 67–73 without changing earlier identities or artifact bytes.
