# Porting manifest

The supported product hosts are Ubuntu 24.04 x64, Windows 2022 x64, and macOS
15 arm64. All hosts run the same Rust debug/Release suites, canonical FWIR
corpus, strict C11/native journey, release provenance checks, and host package
contract.

Windows additionally verifies PE product identity, `longPathAware`, and
long-path publication; Linux runs ASan/UBSan generated-C journeys and
`/dev/full` output failures; macOS executes its arm64 native artifact and
archive. These are platform adaptations of one semantic contract, not separate
backends; exact commands and exclusions are in
[the validation ladder](validation-ladder.md).
