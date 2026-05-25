#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

corpus_root="${IMX_FUZZ_CORPUS:-$root/fuzz/corpus}"
generated_dir="$root/target/fuzz-seed-fixtures"
rm -rf "$generated_dir"
mkdir -p "$corpus_root/farbfeld_decode" "$corpus_root/qoi_decode" "$corpus_root/pnm_decode"

cargo run -p imx-cli --bin imx-generate-fixtures -- "$generated_dir" >/dev/null

cp "$generated_dir/gradient-64.ff" "$corpus_root/farbfeld_decode/gradient-64.ff"
cp "$generated_dir/quantization-2x2.ff" "$corpus_root/farbfeld_decode/quantization-2x2.ff"
cp "$generated_dir/gradient-64.qoi" "$corpus_root/qoi_decode/gradient-64.qoi"
cp "$generated_dir/qoi-rgba-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgba-2x2.qoi"
cp "$generated_dir/qoi-rgb-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgb-2x2.qoi"
cp "$generated_dir/gradient-64.ppm" "$corpus_root/pnm_decode/gradient-64.ppm"
cp "$generated_dir/gradient-64.pgm" "$corpus_root/pnm_decode/gradient-64.pgm"

printf 'farbfeld' >"$corpus_root/farbfeld_decode/header-only.ff"
printf 'qoif\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00' >"$corpus_root/qoi_decode/header-only.qoi"
printf 'P3\n# comment\n2 1\n31\n0 15 31 31 0 15\n' >"$corpus_root/pnm_decode/ascii-ppm-max31.ppm"
printf 'P6\n2 1\n255\n\xff\x00\x00\x00\x80\xff' >"$corpus_root/pnm_decode/binary-ppm-2x1.ppm"
printf 'P2\n# gray ramp\n2 2\n15\n0 7 15 3\n' >"$corpus_root/pnm_decode/pgm-p2-2x2-max15.pgm"
printf 'P5\n2 2\n255\n\x00\x7f\x80\xff' >"$corpus_root/pnm_decode/pgm-p5-2x2-max255.pgm"
printf 'P2\n1 3\n65535\n0 32768 65535\n' >"$corpus_root/pnm_decode/pgm-p2-1x3-max65535.pgm"
printf 'P5\n2 2\n65535\n\x00\x00\x7f\xff\x80\x00\xff\xff' >"$corpus_root/pnm_decode/pgm-p5-2x2-max65535.pgm"
printf 'P5\n1 1\n255\n' >"$corpus_root/pnm_decode/pgm-p5-header-only.pgm"
printf 'P5\n1 1\n65535\n\x12' >"$corpus_root/pnm_decode/pgm-p5-16bit-half-sample.pgm"
printf 'P2\n1 1\n10\n11\n' >"$corpus_root/pnm_decode/pgm-p2-sample-over-max.pgm"
printf 'P2\n1 1\n0\n0\n' >"$corpus_root/pnm_decode/pgm-p2-maxval-zero.pgm"
printf 'P2\n1 1\n65536\n0\n' >"$corpus_root/pnm_decode/pgm-p2-maxval-65536.pgm"
printf 'P5\n1 1\n255X' >"$corpus_root/pnm_decode/pgm-p5-missing-raster-separator.pgm"
printf 'P2\n# unterminated comment' >"$corpus_root/pnm_decode/pgm-p2-comment-eof.pgm"
printf 'P5\n100000 100000\n255\n' >"$corpus_root/pnm_decode/pgm-p5-huge-dims.pgm"

echo "$corpus_root"
