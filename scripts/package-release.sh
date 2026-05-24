#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"imx-preview","version":"\([^"]*\)".*/\1/p')"
if [[ -z "$version" ]]; then
  version="0.1.0"
fi
target="$(rustc -vV | sed -n 's/^host: //p')"
artifact_dir="$root/target/release-artifacts"
staging="$root/target/imx-preview-$version-$target"
rm -rf "$artifact_dir" "$staging"
mkdir -p "$artifact_dir" "$staging"

cargo build --release -p imx-cli --bin imx

cp "$root/target/release/imx" "$staging/"
cp "$root/README.md" "$staging/"
cp "$root/COMPATIBILITY.md" "$staging/"
cp "$root/RELEASE_NOTES.md" "$staging/"
cp "$root/PRODUCTION_READINESS.md" "$staging/"
for doc in LICENSE NOTICE; do
  if [[ -f "$root/$doc" ]]; then
    cp "$root/$doc" "$staging/"
  elif [[ -f "$root/../$doc" ]]; then
    cp "$root/../$doc" "$staging/"
  fi
done

(
  cd "$root/target"
  tar -czf "$artifact_dir/imx-preview-$version-$target.tar.gz" "$(basename "$staging")"
)

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$artifact_dir"/* >"$artifact_dir/SHA256SUMS"
else
  sha256sum "$artifact_dir"/* >"$artifact_dir/SHA256SUMS"
fi

echo "$artifact_dir"
