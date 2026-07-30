# Issue 98: explicit connected bindings

Placeholder-bearing connected templates lower to one owned `ConnectedBinding` and binding-only whole/element borrows into exactly one selected call. Feature 8 and semantic/physical FWIR 1.2 make that ownership observable without changing canonical 1.0/1.1 bytes. Automatic completion remains separate syntax sugar, and neither form creates currying or callable values.
