# Validation ladder

Every tier uses Rust 1.97.1 and stores no host-specific absolute paths.

| Tier | Command and intent |
| --- | --- |
| `tier.focused` | `cargo test <case-or-module>` plus the affected `area.*` contract selection |
| `tier.review` | `cargo fmt --all -- --check`; clippy with warnings denied; all debug tests |
| `tier.full` | all debug and Release tests, Release build, `python tools/validation/contracts.py full` |
| `tier.strict` | `python tools/validation/c11_journey.py`: emitted C with GCC/Clang `-std=c11 -Wall -Wextra -Werror -pedantic-errors`, or MSVC `/std:c11 /W4 /WX /fp:strict`, followed by evaluator/generated/native differential execution |
| `tier.sanitize` | the Linux branch of `tools/validation/c11_journey.py` recompiles and executes the public generated-C journey with ASan+UBSan |
| `tier.qa` | clean isolated checkout, full tier, package extraction/version/hash checks, and every host-applicable journey |

Canonical commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features --release
cargo build --workspace --all-targets --all-features --release
python tools/validation/contracts.py full
```

The pinned stable Rust 1.97.1 toolchain does not distribute Miri, and the
runtime's platform-sensitive floating-point and Windows publication seams are
not compatible with a substitute nightly Miri invocation. This is a documented
exclusion, not a reported pass. Linux ASan/UBSan validates the shipped emitted
C ownership, formatting, argument, and cleanup runtime; all Rust code still
runs under debug overflow checks and the complete debug/Release test matrices.
Linux `/dev/full` journeys verify exact runner and generated-code output failure
records; help and REPL use their deliberately generic stdout error. Windows PE
metadata/long-path tests,
Linux ELF compatibility inspection, and macOS arm64 execution run only on
their target hosts and must never be reported as passing elsewhere.

## 2026-07-26 CI repair validation

The Unix permission-literal and Windows CRLF provenance repairs use these
focused commands:

```sh
cargo test --test cli_contracts
python tests/release_provenance_test.py python tools/release/provenance.py . target/release/faraweave.exe windows-x64
```

The review and full tiers use every canonical command listed above. Final
Windows `tier.qa` uses a clean local clone of `HEAD` with the working diff
applied, every canonical command listed above, and:

```sh
python tools/validation/contracts.py package windows-x64
```

The Windows host cannot execute the `#[cfg(unix)]` unreadable-file journey,
Linux `/dev/full` and sanitizer journeys, Linux ELF inspection, or macOS arm64
execution. The exact Unix clippy diagnostic is covered structurally by the
octal permission literal and must be re-executed by Linux and macOS CI.
