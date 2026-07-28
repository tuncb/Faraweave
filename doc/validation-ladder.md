# Validation ladder

Run each tier from a clean checkout with Rust 1.97.1 and Python 3.11 or newer.
The tiers are cumulative: a later tier does not turn an unavailable
host-specific prerequisite into a pass.

## Focused

Use the narrowest affected Rust test, then run:

```sh
cargo fmt --all -- --check
python tools/validation/contracts.py focused
```

## Review

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
python tools/validation/contracts.py review
```

Changes that can affect execution, resources, emitted C, or native builds also
run:

```sh
cargo build --workspace --all-targets --all-features --release
python tools/validation/c11_journey.py
```

## Full and final QA

Final QA runs in an isolated worktree created from the current `origin/main`
with the reviewed issue commit merged into it:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --all-targets --all-features --release
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features --release
python tools/validation/contracts.py full
python tools/validation/contracts.py package <host-target>
```

`contracts.py full` includes static workflow, cutover, specification, corpus,
and traceability checks; the strict C11/interpreter/generated/native journey;
and release provenance. The package target is exactly one of `linux-x64`,
`windows-x64`, or `macos-arm64` and must match the host.

## Platform exclusions

- Windows x64 requires Visual Studio's x64 C11 environment and `rc.exe`; it
  runs PE identity, long-path, archive, and native journeys. ASan/UBSan and
  `/dev/full` are Linux exclusions.
- Linux x64 requires a strict C11 compiler plus `readelf` and `ldd`; the C11
  journey runs ASan/UBSan and the full contract tests exercise `/dev/full`.
  PE identity, Windows long paths, and macOS archive execution are exclusions.
- macOS arm64 requires the system strict C11 compiler and runs native and
  archive journeys. PE identity, Windows long paths, Linux `/dev/full`, and
  the Linux sanitizer gate are exclusions.

The GitHub Actions matrix runs all three hosts and invokes
`python tools/validation/contracts.py package ${{ matrix.target }}` after the
full contract on each host. A local run records other-host checks as assigned
to that matrix, never as locally passed.

## FWIR v1 acceptance run

On 2026-07-28 the complete command block above passed on Windows AMD64 with
`rustc 1.97.1 (8bab26f4f 2026-07-14)`, Python 3.11.15, and Visual Studio
`cl.exe` 14.51.36231. `contracts.py full` reported the strict C11/native and
release-provenance journeys passed, and `contracts.py package windows-x64`
reported the host package contract passed.

The run also passed `python tools/validation/contracts.py review`,
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`,
and `git diff --check`. Linux x64 ASan/UBSan and `/dev/full`, macOS arm64
native/archive, and the other-host package contracts were excluded locally and
are assigned to the three-host Main CI matrix.
