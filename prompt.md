# Rewrite Bennu as Faraweave in Rust

You are the principal Rust language-runtime and release engineer responsible for completing an end-to-end rewrite of Bennu as Faraweave.

Do not stop after analysis, scaffolding, a partial parser, or a test plan. Continue until the complete implementation, tests, documentation, validation tooling, CI workflows, and release packaging are present and locally validated as far as the current machine permits.

## Repositories and source authority

Target repository:

`D:\Faraweave`

Bennu repository:

`D:\bennu`

All implementation changes belong in `D:\Faraweave`.
## Product identity

This is a clean Rust implementation named Faraweave:

- Cargo package and library crate: `faraweave`
- Executable: `faraweave` / `faraweave.exe`
- Product name: `Faraweave`
- Initial version: exactly `0.1.0`
- `faraweave --version` must print exactly `faraweave 0.1.0\n`
- `Cargo.toml` is the canonical product-version source.
- Commit `Cargo.lock`.
- Pin Rust 1.97.1 in `rust-toolchain.toml`, including rustfmt and clippy.

Mechanically rename public Bennu branding to Faraweave, including help text, diagnostics, environment variables, generated-code identifiers, package names, scripts, archive names, and Windows metadata. Preserve diagnostic schemas and behavior while changing their product prefix consistently.

The syntax is extension-agnostic. Use `.faraweave` for new public examples, but retain tests proving that arbitrary extensions, including existing `.bennu` fixtures, work.

Faraweave 0.1.0 is the first Faraweave release of the selected current Bennu main behavior. Do not port the behavior of the historical Bennu v0.1.0 tag.

## Required investigation

Before implementing, completely inspect at the selected main commit:

- `AGENTS.md`
- `README.md`
- `CMakeLists.txt` and `CMakePresets.json`
- all files under `include/`, `src/`, `spec/`, `tests/`, `examples/`, `tools/`, `cmake/`, and `.github/workflows/`
- the validation ladder and decision records relevant to current behavior
- every test fixture and traceability entry

Create `doc/porting-manifest.md` in Faraweave recording:

- the exact selected source commit
- every source implementation module and its Rust destination
- every source test executable, registered contract test, and named unit case
- the equivalent Faraweave test or an explicitly justified Rust/toolchain adaptation
- platform-specific tests and their supported hosts
- intentional branding, ABI, harness, and packaging differences

No source test may silently disappear.

## Functional parity

Implement all behavior present at the selected main commit, including:

- tokenizer, parser, flat/postorder representation, typed lowering, and deterministic source spans
- scalar types Bool, signed 64-bit Int, and IEEE-754 binary64 Double
- homogeneous rank-1 vectors, typed empty vectors, structural tuples, tuple spreading where supported, and explicit sequential fan-out
- program parameter headers, typed parameter binding, exact runner argument decoding, and argument precedence
- every primitive and overload present in the selected source, including at least:
  `inc`, `dec`, `neg`, `abs`, `add`, `sub`, `mul`, `equals`, `not_equals`,
  `not`, `and`, `or`, `odd`, `even`, `is_positive`, `is_negative`,
  `less_than`, `greater_than`, and `iota`
- exact-first overload selection and only the specified Int-to-Double promotion
- scalar broadcasting, equal vector-length rules, singleton-vector behavior, typed empty behavior, and deterministic validation precedence
- checked Int64 arithmetic with structured overflow errors and no partial result
- exact binary64 rules, canonical NaNs, signed zero, infinities, gradual underflow, deterministic formatting, and protection/restoration of the floating-point environment where required by the specification
- canonical scalar, vector, tuple, type, error, and diagnostic formatting
- source evaluation, typed public evaluation APIs, REPL, `run`, `emit-c`, and `build`
- deterministic self-contained strict C11 emission
- native C compilation with explicit `--cc`, `CC`, and platform fallback precedence
- path alias protection, publish-last replacement, cleanup after failures, and transactional stdout
- Unicode paths, paths containing spaces, Windows long-path behavior, unreadable/unwritable files, and output-device failures
- trusted-local and bounded execution profiles
- vector, tuple-table, live-byte, work-unit, representability, allocation-ordinal, ownership, borrowing, cleanup, and fault-injection contracts
- exact static and runtime error ordering and all-or-nothing program publication

The shipped Faraweave executable must be a standalone Rust implementation. It must not invoke, embed, link against, compile, or require the Bennu C++ implementation. Bennu may only be used as a development-time differential oracle.

## Rust architecture

Translate Bennu's data-oriented constraints into idiomatic Rust:

- Prefer plain structs, enums, slices, vectors, and module-level functions.
- Keep data representation and transformations visibly separated.
- Use explicit `Result` and error values for recoverable failures.
- Do not panic, unwrap, or expect on user input, filesystem input, allocation/resource refusal, compiler output, or other recoverable conditions.
- Keep ownership, allocation, resource charging, and execution order observable and deterministic.
- Avoid trait-object hierarchies, unnecessary generics, hidden cloning, and abstraction without a concrete benefit.
- Use safe Rust by default.
- Any required `unsafe` must be narrowly isolated, documented with invariants, and covered by focused tests. Likely candidates include floating-point environment control and Windows process/resource integration.
- Use checked sizing and `try_reserve`/`try_reserve_exact` through an explicit allocation seam so deterministic allocation-failure tests remain possible.
- Do not weaken resource accounting just because Rust normally manages memory automatically.
- Keep dependencies minimal and explain each nontrivial dependency in `doc/decisions/`. Be brief in explanations

A reasonable module decomposition is:

- error and source locations
- host allocation/storage seam
- execution profiles and resource accounting
- types and values
- primitive registry, overload resolution, and kernels
- application and lifting
- tokenizer, parser, lowering, and evaluator
- C11 emitter and generated runtime
- native builder and process launch
- CLI output, paths, REPL, and runner
- platform-specific floating-point and Windows metadata support

Adjust this only when a different Rust structure provides clearer ownership or parity.

## Tests and validation

Port the complete intent of the Bennu suite. At the authored source snapshot, Bennu contained 158 named unit cases and, after a fresh configure, 58 registered Windows tests and 63 registered Linux tests. Rediscover and record the exact topology at the newly selected main commit rather than trusting only these counts.

Requirements:

- Preserve stable semantic test IDs such as S16, CUTOVER, PARG, TUP, FAN, SHARED, ISSUE54, and resource-profile identifiers.
- Port every unit, CLI, evaluator, C-emission, native-build, public API, tuple, fan-out, resource, workflow, release, documentation, and platform contract.
- Replace CMake/doctest-specific checks with Rust-native equivalents where appropriate, but preserve their intent and record the mapping.
- Port fixtures and expected output byte-for-byte except for deliberate Faraweave branding/version normalization.
- Retain specification traceability and negative tests that prove missing, duplicate, or stale traceability entries fail.
- Differentially test evaluator output, CLI output, emitted C output, compiled C behavior, native builds, exit codes, stderr, and filesystem atomicity.
- Add golden differential corpora generated from the selected source commit. Record provenance; do not commit source binaries or host-specific absolute paths.
- Do not mark parity tests ignored or skipped merely to make the suite pass.
- Do not delete or weaken a test when Rust makes the implementation easier.
- Do not leave `todo!`, `unimplemented!`, placeholder branches, fake success paths, or "future work" for required behavior.

Use Cargo as the product build system. CMake must not remain necessary to build Faraweave. Python, PowerShell, and shell scripts may remain for cross-platform contract, packaging, and release verification when they are the clearest option.

Recreate the focused/review/full/strict/sanitize/QA validation taxonomy with checked-in Rust-oriented entry points and area selections. Document exact commands in `doc/validation-ladder.md`.

At minimum, local validation must include:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features`
- `cargo build --workspace --all-targets --all-features --release`
- the complete Release contract suite
- emitted strict-C11 compilation and execution
- native-build journeys
- all platform tests applicable to the current host

Use an appropriate Rust memory and undefined-behavior validation path, such as Miri for compatible focused modules and platform sanitizer jobs where supported. Document justified exclusions instead of pretending Rust has a direct CTest sanitizer equivalent.

## CI

Implement `.github/workflows/main.yml` with the same operational guarantees as Bennu Main CI:

- pull requests targeting main
- pushes to main
- manual dispatch
- concurrency cancellation for superseded PR runs
- minimal permissions
- pinned third-party actions by full commit SHA
- fail-fast disabled
- matrix:
  - Ubuntu 24.04 x64
  - Windows 2022 x64
  - macOS 15 arm64
- explicit OS and architecture verification
- pinned Rust toolchain verification
- formatting, clippy with warnings denied, Release build, full tests, contract tests, C11 journeys, and platform checks
- an unconditional PR Gate job that fails unless every required matrix entry succeeds

Port the workflow self-tests and negative mutation tests so accidental weakening of triggers, permissions, matrix targets, checkout pinning, commands, or the PR gate is detected.

## Packaging and release

Port all applicable packaging, provenance, and release guarantees, but do not copy Bennu's historical immutable-v0.1.0 asset checks or URLs.

Create Faraweave archives:

- `faraweave-v0.1.0-linux-x64.tar.gz`
- `faraweave-v0.1.0-windows-x64.zip`
- `faraweave-v0.1.0-macos-arm64.tar.gz`

Each archive must have a deterministic layout containing exactly the target executable and LICENSE unless the documented Faraweave contract deliberately adds another required file.

Implement and test:

- execution of the extracted artifact on its target
- version verification
- archive and executable SHA-256 provenance
- deterministic, newline-terminated release manifest JSON
- source-commit and annotated `v0.1.0` tag gating
- no overwrite of an existing release or asset
- upload, remote-byte re-download, exact comparison, and publication as the final mutation
- GitHub artifact attestations for all archives and the manifest
- Windows PE product name, version, filename, and long-path manifest
- Linux compatibility-floor and dependency inspection adapted to a Rust binary; do not retain C++-only `libstdc++` expectations
- macOS arm64 execution and archive verification

Implement:

- a Faraweave initial-release workflow for `v0.1.0`
- a generic future-release candidate/publish workflow based on the current Bennu state machine
- offline positive and negative workflow/release-state-machine tests

The future workflow must not reject `0.1.0` merely because Bennu's historical workflow did. That exclusion belongs only to the Bennu repository.

## Documentation

Replace the placeholder README with complete Faraweave documentation covering:

- language behavior and all primitives
- syntax examples
- REPL, run, emit-c, and build
- program parameters
- errors and transactional behavior
- execution profiles
- build and validation commands
- supported platforms
- packaging and release verification
- deliberate differences from Anka
- provenance from the selected Bennu main commit
- intentional Rust-only implementation differences

Port the normative specifications and relevant decision records, updating product names without changing semantics. Add `AGENTS.md` with the Rust/data-oriented constraints and validation ladder future work must follow.

## Completion rules

Do not declare completion until:

1. Every source implementation area and test is accounted for in the porting manifest.
2. All required language, CLI, library, C-emission, native-build, resource, and platform behavior is implemented.
3. No shipped code depends on `D:\bennu` or its study worktree.
4. Formatting and clippy pass with warnings denied.
5. All applicable local tests pass in debug and Release configurations.
6. The C11 and native differential journeys pass.
7. CI and release workflows have positive and negative contract coverage.
8. README, specifications, validation documentation, and release instructions match the implementation.
9. `git status` and the complete target diff contain only intended Faraweave changes.
10. There are no ignored parity tests, unfinished placeholders, or silent scope reductions.

Do not push, create tags, publish releases, or create external pull requests unless the user separately authorizes those mutations.

At the end, report:

- architecture and implementation summary
- exact selected Bennu source commit
- parity-manifest totals
- test counts by category and platform
- exact commands executed and their results
- any platform jobs that could not be run locally
- justified parity adaptations
- remaining genuine blockers, if any

Do not describe unexecuted CI as passing.
