#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"
test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
cargo build --release
cargo test --workspace --all-targets --all-features --release
python3 tools/validation/contracts.py package linux-x64

