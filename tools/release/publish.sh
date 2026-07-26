#!/usr/bin/env bash
set -euo pipefail
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
tag="v${version}"
test "${SOURCE_COMMIT}" = "$(git rev-parse HEAD)"
test "$(git cat-file -t "refs/tags/${tag}")" = tag
test "$(git rev-parse "refs/tags/${tag}^{commit}")" = "${SOURCE_COMMIT}"
if gh release view "${tag}" >/dev/null 2>&1; then
  echo "release already exists: ${tag}" >&2; exit 1
fi
gh release create "${tag}" artifacts/* --draft --verify-tag --title "Faraweave ${version}"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
gh release download "${tag}" --dir "$tmp"
for local in artifacts/*; do cmp "$local" "$tmp/$(basename "$local")"; done
# Publication is deliberately the final mutation.
gh release edit "${tag}" --draft=false

