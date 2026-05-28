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
  imx-preview-0.12.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.12.0-aarch64-unknown-linux-gnu.tar.gz
linux_only_formula="$work_dir/linux-only.rb"
bash scripts/generate-homebrew-formula.sh 0.12.0 "$linux_only" "$linux_only_formula"
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
assert_contains "$linux_only_formula" "JPEG:oriented-o6.jpg"
assert_contains "$linux_only_formula" "JPEG:progressive-rgb.jpg"
assert_contains "$linux_only_formula" "JPEG:progressive-o6.jpg"
assert_contains "$linux_only_formula" "FARBFELD:jpeg-output.ff"
assert_contains "$linux_only_formula" "PPM:intake-comments.ppm"
assert_contains "$linux_only_formula" "PGM:intake-pgm16.pgm"
assert_contains "$linux_only_formula" "FARBFELD:intake-pgm16.ff"
assert_not_contains "$linux_only_formula" "PPM:resized.ppm"

resize_release="$work_dir/resize-release.SHA256SUMS"
write_checksums "$resize_release" \
  imx-preview-0.13.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.13.0-aarch64-unknown-linux-gnu.tar.gz
resize_formula="$work_dir/resize-release.rb"
bash scripts/generate-homebrew-formula.sh v0.13.0 "$resize_release" "$resize_formula"
assert_formula_syntax "$resize_formula"
assert_contains "$resize_formula" '"resize", "1x1", "PPM:input.ppm", "PPM:resized.ppm"'
assert_contains "$resize_formula" "format=PPM width=1 height=1 channels=RGB depth=8"
assert_not_contains "$resize_formula" '"resize-fit", "5x5", "PPM:input.ppm", "PPM:fit.ppm"'

resize_fit_release="$work_dir/resize-fit-release.SHA256SUMS"
write_checksums "$resize_fit_release" \
  imx-preview-0.14.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.14.0-aarch64-unknown-linux-gnu.tar.gz
resize_fit_formula="$work_dir/resize-fit-release.rb"
bash scripts/generate-homebrew-formula.sh v0.14.0 "$resize_fit_release" "$resize_fit_formula"
assert_formula_syntax "$resize_fit_formula"
assert_contains "$resize_fit_formula" '"resize", "1x1", "PPM:input.ppm", "PPM:resized.ppm"'
assert_contains "$resize_fit_formula" "format=PPM width=1 height=1 channels=RGB depth=8"
assert_contains "$resize_fit_formula" '"resize-fit", "5x5", "PPM:input.ppm", "PPM:fit.ppm"'
assert_contains "$resize_fit_formula" "format=PPM width=5 height=3 channels=RGB depth=8"
assert_not_contains "$resize_fit_formula" '"batch-convert", "--to", "PPM"'

batch_release="$work_dir/batch-release.SHA256SUMS"
write_checksums "$batch_release" \
  imx-preview-0.15.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.15.0-aarch64-unknown-linux-gnu.tar.gz
batch_formula="$work_dir/batch-release.rb"
bash scripts/generate-homebrew-formula.sh v0.15.0 "$batch_release" "$batch_formula"
assert_formula_syntax "$batch_formula"
assert_contains "$batch_formula" '"resize", "1x1", "PPM:input.ppm", "PPM:resized.ppm"'
assert_contains "$batch_formula" '"resize-fit", "5x5", "PPM:input.ppm", "PPM:fit.ppm"'
assert_contains "$batch_formula" '"batch-convert", "--to", "PPM", "--output-dir", "batch", "--resize-fit", "5x5", "PPM:batch-ppm.ppm", "PGM:batch-pgm.pgm"'
assert_contains "$batch_formula" "format=PPM width=5 height=3 channels=RGB depth=8"
assert_not_contains "$batch_formula" "BMP:output.bmp"

bmp_release="$work_dir/bmp-release.SHA256SUMS"
write_checksums "$bmp_release" \
  imx-preview-0.16.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.16.0-aarch64-unknown-linux-gnu.tar.gz
bmp_formula="$work_dir/bmp-release.rb"
bash scripts/generate-homebrew-formula.sh v0.16.0 "$bmp_release" "$bmp_formula"
assert_formula_syntax "$bmp_formula"
assert_contains "$bmp_formula" '"batch-convert", "--to", "PPM", "--output-dir", "batch", "--resize-fit", "5x5", "PPM:batch-ppm.ppm", "PGM:batch-pgm.pgm"'
assert_contains "$bmp_formula" "BMP:output.bmp"
assert_contains "$bmp_formula" '"resize", "1x1", "BMP:output.bmp", "BMP:resized.bmp"'
assert_contains "$bmp_formula" '"resize-fit", "5x5", "BMP:output.bmp", "BMP:fit.bmp"'
assert_contains "$bmp_formula" '"batch-convert", "--to", "BMP", "--output-dir", "batch-bmp", "--resize-fit", "5x5", "PPM:input.ppm"'
assert_contains "$bmp_formula" "format=BMP width=5 height=3 channels=RGB depth=8"
assert_not_contains "$bmp_formula" '"self-test"'

self_test_release="$work_dir/self-test-release.SHA256SUMS"
write_checksums "$self_test_release" \
  imx-preview-0.17.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.17.0-aarch64-unknown-linux-gnu.tar.gz
self_test_formula="$work_dir/self-test-release.rb"
bash scripts/generate-homebrew-formula.sh v0.17.0 "$self_test_release" "$self_test_formula"
assert_formula_syntax "$self_test_formula"
assert_contains "$self_test_formula" 'system bin/"imx", "self-test"'
assert_contains "$self_test_formula" "BMP:output.bmp"
assert_contains "$self_test_formula" '"batch-convert", "--to", "BMP", "--output-dir", "batch-bmp", "--resize-fit", "5x5", "PPM:input.ppm"'
assert_not_contains "$self_test_formula" 'identify --json PPM:input.ppm'

json_report_release="$work_dir/json-report-release.SHA256SUMS"
write_checksums "$json_report_release" \
  imx-preview-0.18.0-x86_64-unknown-linux-gnu.tar.gz \
  imx-preview-0.18.0-aarch64-unknown-linux-gnu.tar.gz
json_report_formula="$work_dir/json-report-release.rb"
bash scripts/generate-homebrew-formula.sh v0.18.0 "$json_report_release" "$json_report_formula"
assert_formula_syntax "$json_report_formula"
assert_contains "$json_report_formula" 'system bin/"imx", "self-test"'
assert_contains "$json_report_formula" 'identify --json PPM:input.ppm'
assert_contains "$json_report_formula" 'report --json PPM:input.ppm'
assert_contains "$json_report_formula" 'assert_equal "supported", report_json.fetch("status")'

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
