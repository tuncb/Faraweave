# Faraweave examples

Each non-header line is an expression. Running a file evaluates its expressions
in order and prints one result per line.

| Example | What it demonstrates |
| --- | --- |
| [`basic-math.faraweave`](basic-math.faraweave) | Prefix and bracket calls, arithmetic, numeric promotion, predicates, and Boolean logic |
| [`vectors.faraweave`](vectors.faraweave) | Vector literals, scalar broadcasting, elementwise calls, sorting, queries, and typed empty vectors |
| [`tuples.faraweave`](tuples.faraweave) | Heterogeneous and nested tuples, plus one-level tuple spreading |
| [`fanout.faraweave`](fanout.faraweave) | Reusing one operand across several left-to-right branches |
| [`reductions.faraweave`](reductions.faraweave) | Numeric and Boolean reductions, folds, and seed-inclusive scans |
| [`math-functions.faraweave`](math-functions.faraweave) | Host-math scalar and vector functions |
| [`parameters.faraweave`](parameters.faraweave) | Declaring and using typed command-line inputs |
| [`bindings.faraweave`](bindings.faraweave) | Naming an evaluate-once value and borrowing it from later expressions |
| [`rewrite.faraweave`](rewrite.faraweave) | A compact tour of scalar and vector expressions |

Run an example from the repository root:

```sh
cargo run -- run examples/basic-math.faraweave
```

The parameter example expects an `Int`, a `Double`, and a `Bool`, in that
order. The `--` separates program arguments from Faraweave's own options:

```sh
cargo run -- run examples/parameters.faraweave -- 4 2.5 true
```

In Faraweave source, `#` begins a line comment. The checked-in runnable files
remain concise, with longer explanations here.
