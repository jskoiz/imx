#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

corpus_root="${IMX_FUZZ_CORPUS:-$root/fuzz/corpus}"
generated_dir="$root/target/fuzz-seed-fixtures"
rm -rf "$generated_dir"
mkdir -p "$corpus_root/farbfeld_decode" "$corpus_root/qoi_decode" "$corpus_root/ppm_decode"

cargo run -p imx-cli --bin imx-generate-fixtures -- "$generated_dir" >/dev/null

cp "$generated_dir/gradient-64.ff" "$corpus_root/farbfeld_decode/gradient-64.ff"
cp "$generated_dir/quantization-2x2.ff" "$corpus_root/farbfeld_decode/quantization-2x2.ff"
cp "$generated_dir/gradient-64.qoi" "$corpus_root/qoi_decode/gradient-64.qoi"
cp "$generated_dir/qoi-rgba-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgba-2x2.qoi"
cp "$generated_dir/qoi-rgb-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgb-2x2.qoi"
cp "$generated_dir/gradient-64.ppm" "$corpus_root/ppm_decode/gradient-64.ppm"

printf 'farbfeld' >"$corpus_root/farbfeld_decode/header-only.ff"
printf 'qoif\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00' >"$corpus_root/qoi_decode/header-only.qoi"
printf 'P3\n# comment\n2 1\n31\n0 15 31 31 0 15\n' >"$corpus_root/ppm_decode/ascii-max31.ppm"
printf 'P6\n2 1\n255\n\xff\x00\x00\x00\x80\xff' >"$corpus_root/ppm_decode/binary-2x1.ppm"

echo "$corpus_root"
