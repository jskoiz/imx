#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

mkdir -p "$root/target"
work_dir="$(mktemp -d "$root/target/homebrew-formula-generator.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT

write_checksums() {
  local output="$1"
  shift
  : >"$output"
  local index=1
  local archive
  for archive in "$@"; do
    printf '%064x  %s\n' "$index" "$archive" >>"$output"
    index=$((index + 1))
  done
}

assert_contains() {
  local file="$1"
  local pattern="$2"
  if ! grep -Fq "$pattern" "$file"; then
    echo "error: expected $file to contain: $pattern" >&2
    exit 1
  fi
}

assert_not_contains() {
  local file="$1"
  local pattern="$2"
  if grep -Fq "$pattern" "$file"; then
    echo "error: expected $file not to contain: $pattern" >&2
    exit 1
  fi
}

assert_formula_syntax() {
  local formula="$1"
  ruby -c "$formula" >/dev/null
}

v040="$work_dir/v0.4.0.SHA256SUMS"
write_checksums "$v040" \
  imx-preview-0.4.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.4.0-aarch64-apple-darwin.tar.gz \
  imx-preview-0.4.0-x86_64-apple-darwin.tar.gz
v040_formula="$work_dir/v0.4.0.rb"
bash scripts/generate-homebrew-formula.sh v0.4.0 "$v040" "$v040_formula"
assert_formula_syntax "$v040_formula"
assert_contains "$v040_formula" "on_macos do"
assert_contains "$v040_formula" "aarch64-apple-darwin"
assert_contains "$v040_formula" "x86_64-apple-darwin"
assert_contains "$v040_formula" "x86_64-unknown-linux-gnu"
assert_not_contains "$v040_formula" "aarch64-unknown-linux-gnu"
assert_not_contains "$v040_formula" "PPM:input.ppm"

linux_only="$work_dir/linux-only.SHA256SUMS"
write_checksums "$linux_only" \
  imx-preview-0.9.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.9.0-aarch64-unknown-linux-gnu.tar.gz
linux_only_formula="$work_dir/linux-only.rb"
bash scripts/generate-homebrew-formula.sh 0.9.0 "$linux_only" "$linux_only_formula"
assert_formula_syntax "$linux_only_formula"
assert_not_contains "$linux_only_formula" "on_macos do"
assert_contains "$linux_only_formula" "on_linux do"
assert_contains "$linux_only_formula" "x86_64-unknown-linux-gnu"
assert_contains "$linux_only_formula" "aarch64-unknown-linux-gnu"
assert_contains "$linux_only_formula" "PPM:input.ppm"
assert_contains "$linux_only_formula" "FARBFELD:prefix-output.ff"
assert_contains "$linux_only_formula" "PNG:output.png"
assert_contains "$linux_only_formula" "FARBFELD:png-output.ff"
assert_contains "$linux_only_formula" "JPEG:output.jpg"
assert_contains "$linux_only_formula" "FARBFELD:jpeg-output.ff"

all_targets="$work_dir/all-targets.SHA256SUMS"
write_checksums "$all_targets" \
  imx-preview-0.6.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.6.0-aarch64-unknown-linux-gnu.tar.gz \
  imx-preview-0.6.0-aarch64-apple-darwin.tar.gz \
  imx-preview-0.6.0-x86_64-apple-darwin.tar.gz
all_targets_formula="$work_dir/all-targets.rb"
bash scripts/generate-homebrew-formula.sh v0.6.0 "$all_targets" "$all_targets_formula"
assert_formula_syntax "$all_targets_formula"
assert_contains "$all_targets_formula" "on_macos do"
assert_contains "$all_targets_formula" "on_linux do"
assert_contains "$all_targets_formula" "aarch64-unknown-linux-gnu"
assert_contains "$all_targets_formula" "x86_64-unknown-linux-gnu"
assert_contains "$all_targets_formula" "aarch64-apple-darwin"
assert_contains "$all_targets_formula" "x86_64-apple-darwin"

empty="$work_dir/empty.SHA256SUMS"
: >"$empty"
if bash scripts/generate-homebrew-formula.sh v0.8.0 "$empty" "$work_dir/empty.rb" 2>"$work_dir/empty.err"; then
  echo "error: empty SHA256SUMS unexpectedly generated a formula" >&2
  exit 1
fi
assert_contains "$work_dir/empty.err" "does not contain any supported IMX release archives"

echo "Homebrew formula generator smoke passed."
