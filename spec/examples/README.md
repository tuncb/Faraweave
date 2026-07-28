# Canonical FWIR v1 examples

The three `.hex` files contain whitespace-separated canonical artifact bytes:

- `fwir-v1-empty.hex` is the smallest valid program.
- `fwir-v1-scalar-true.hex` demonstrates source provenance, a constant node,
  ownership, and a root.
- `fwir-v1-complete.hex` covers all 16 semantic-1.0 sections and all six node
  opcodes. Semantic-1.1 operation-reference records have constructed codec
  coverage because issue #38 adds no executable source consumer.

Convert an example to a binary artifact with
`bytes.fromhex(Path(name).read_text())`; whitespace is not part of the
artifact. Exact lengths, FNV-1a identities, supported public surfaces, and
positive/negative requirement evidence are recorded in
`../../tests/fixtures/fwir-v1-corpus.tsv` and
`../../tests/fixtures/fwir-v1-conformance.tsv`.
