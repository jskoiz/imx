#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

oracle="${IMAGEMAGICK_MAGICK:-magick}"
if ! "$oracle" -version >/dev/null 2>&1; then
  echo "error: ImageMagick oracle is not runnable: $oracle" >&2
  exit 2
fi

stamp="$(date +%Y%m%d-%H%M%S)"
out_dir="${IMX_BENCH_OUT:-$root/target/release-bench-$stamp}"
fixture_dir="$out_dir/fixtures"
mkdir -p "$out_dir"

cargo build --release -p imx-cli --bin imx
cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir"

{
  echo "date=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "uname=$(uname -a)"
  echo "standalone=$root/target/release/imx"
  "$root/target/release/imx" --version | sed 's/^/standalone_version: /'
  echo "oracle=$oracle"
  "$oracle" -version | sed 's/^/oracle_version: /'
  cat "$fixture_dir/manifest.txt"
} >"$out_dir/metadata.txt"

if [[ "$(uname -s)" == "Darwin" ]]; then
  time_cmd=(/usr/bin/time -l)
else
  time_cmd=(/usr/bin/time -v)
fi

run_timed() {
  local label="$1"
  shift
  "${time_cmd[@]}" "$@" >"$out_dir/$label.stdout" 2>"$out_dir/$label.time"
}

iterations="${IMX_BENCH_ITERATIONS:-10}"
IMX_BENCH_ITERATIONS="$iterations" cargo bench --bench throughput >"$out_dir/standalone-library-bench.stdout" 2>"$out_dir/standalone-library-bench.stderr"

run_timed standalone-ff-to-qoi "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.qoi"
run_timed standalone-ff-to-bmp "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.bmp"
run_timed standalone-ff-to-pbm "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.pbm"
run_timed standalone-ff-to-ppm "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.ppm"
run_timed standalone-ff-to-pgm "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.pgm"
run_timed standalone-ff-to-png "$root/target/release/imx" "$fixture_dir/gradient-64.ff" "$out_dir/standalone-gradient.png"
run_timed standalone-ppm-to-jpeg "$root/target/release/imx" "$fixture_dir/gradient-64.ppm" "$out_dir/standalone-gradient.jpg"
run_timed standalone-jpeg-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.jpg" "$out_dir/standalone-jpeg.ff"
run_timed standalone-qoi-to-ff "$root/target/release/imx" "$fixture_dir/qoi-rgba-2x2.qoi" "$out_dir/standalone-qoi-rgba.ff"
run_timed standalone-bmp-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.bmp" "$out_dir/standalone-bmp.ff"
run_timed standalone-pbm-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.pbm" "$out_dir/standalone-pbm.ff"
run_timed standalone-ppm-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.ppm" "$out_dir/standalone-ppm.ff"
run_timed standalone-ppm16-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64-ppm16.ppm" "$out_dir/standalone-ppm16.ff"
run_timed standalone-pgm-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.pgm" "$out_dir/standalone-pgm.ff"
run_timed standalone-png-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64.png" "$out_dir/standalone-png.ff"
run_timed standalone-png16-to-ff "$root/target/release/imx" "$fixture_dir/gradient-64-png16.png" "$out_dir/standalone-png16.ff"
run_timed standalone-ppm-resize "$root/target/release/imx" resize 17x11 "PPM:$fixture_dir/gradient-64.ppm" "PPM:$out_dir/standalone-resized.ppm"
run_timed standalone-bmp-resize "$root/target/release/imx" resize 17x11 "BMP:$fixture_dir/gradient-64.bmp" "BMP:$out_dir/standalone-resized.bmp"
run_timed standalone-ppm-resize-fit "$root/target/release/imx" resize-fit 17x11 "PPM:$fixture_dir/intake-comments-2x1.ppm" "PPM:$out_dir/standalone-resized-fit.ppm"
run_timed standalone-bmp-resize-fit "$root/target/release/imx" resize-fit 17x11 "BMP:$fixture_dir/gradient-64.bmp" "BMP:$out_dir/standalone-resized-fit.bmp"
mkdir -p "$out_dir/standalone-batch" "$out_dir/oracle-batch"
cp "$fixture_dir/gradient-64.ppm" "$out_dir/batch-ppm.ppm"
cp "$fixture_dir/gradient-64.pgm" "$out_dir/batch-pgm.pgm"
cp "$fixture_dir/gradient-64.png" "$out_dir/batch-png.png"
cp "$fixture_dir/gradient-64.bmp" "$out_dir/batch-bmp.bmp"
run_timed standalone-batch-convert-mixed-to-ppm "$root/target/release/imx" batch-convert --to PPM --output-dir "$out_dir/standalone-batch" --resize-fit 17x11 "PPM:$out_dir/batch-ppm.ppm" "PGM:$out_dir/batch-pgm.pgm" "PNG:$out_dir/batch-png.png" "BMP:$out_dir/batch-bmp.bmp"

run_timed oracle-farbfeld-decode "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" NULL:
run_timed oracle-bmp-decode "$oracle" "BMP:$fixture_dir/gradient-64.bmp" NULL:
run_timed oracle-qoi-decode "$oracle" "QOI:$fixture_dir/gradient-64.qoi" NULL:
run_timed oracle-pbm-decode "$oracle" "PBM:$fixture_dir/gradient-64.pbm" NULL:
run_timed oracle-ppm-decode "$oracle" "PPM:$fixture_dir/gradient-64.ppm" NULL:
run_timed oracle-ppm16-decode "$oracle" "PPM:$fixture_dir/gradient-64-ppm16.ppm" NULL:
run_timed oracle-pgm-decode "$oracle" "PGM:$fixture_dir/gradient-64.pgm" NULL:
run_timed oracle-png-decode "$oracle" "PNG:$fixture_dir/gradient-64.png" NULL:
run_timed oracle-png16-decode "$oracle" "PNG:$fixture_dir/gradient-64-png16.png" NULL:
run_timed oracle-jpeg-decode "$oracle" "JPEG:$fixture_dir/gradient-64.jpg" NULL:
run_timed oracle-farbfeld-encode "$oracle" -size 64x64 -depth 16 -endian MSB "RGBA:$fixture_dir/gradient-64.rgba16be" "FARBFELD:$out_dir/oracle-gradient.ff"
run_timed oracle-bmp-encode "$oracle" -size 64x64 -depth 8 "RGB:$fixture_dir/gradient-64.rgb" "BMP:$out_dir/oracle-gradient.bmp"
run_timed oracle-qoi-encode "$oracle" -size 64x64 -depth 8 "RGBA:$fixture_dir/gradient-64.rgba" "QOI:$out_dir/oracle-gradient.qoi"
run_timed oracle-pbm-encode "$oracle" -size 64x64 -depth 8 "GRAY:$fixture_dir/gradient-64.gray" "PBM:$out_dir/oracle-gradient.pbm"
run_timed oracle-ppm-encode "$oracle" -size 64x64 -depth 8 "RGB:$fixture_dir/gradient-64.rgb" "PPM:$out_dir/oracle-gradient.ppm"
run_timed oracle-ppm16-encode "$oracle" -size 64x64 -depth 16 -endian MSB "RGB:$fixture_dir/gradient-64.rgb16be" "PPM:$out_dir/oracle-gradient-ppm16.ppm"
run_timed oracle-pgm-encode "$oracle" -size 64x64 -depth 8 "GRAY:$fixture_dir/gradient-64.gray" "PGM:$out_dir/oracle-gradient.pgm"
run_timed oracle-png-encode "$oracle" -size 64x64 -depth 8 "RGBA:$fixture_dir/gradient-64.rgba" "PNG:$out_dir/oracle-gradient.png"
run_timed oracle-png16-encode "$oracle" -size 64x64 -depth 16 -endian MSB "RGBA:$fixture_dir/gradient-64.rgba16be" "PNG:$out_dir/oracle-gradient-png16.png"
run_timed oracle-jpeg-encode "$oracle" -size 64x64 -depth 8 "RGB:$fixture_dir/gradient-64.rgb" -quality 90 -sampling-factor 4:4:4 -interlace none -strip "JPEG:$out_dir/oracle-gradient.jpg"
run_timed oracle-ff-to-qoi "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "QOI:$out_dir/oracle-gradient-transcode.qoi"
run_timed oracle-ff-to-bmp "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "BMP:$out_dir/oracle-gradient-transcode.bmp"
run_timed oracle-ff-to-pbm "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "PBM:$out_dir/oracle-gradient-transcode.pbm"
run_timed oracle-ff-to-ppm "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "PPM:$out_dir/oracle-gradient-transcode.ppm"
run_timed oracle-ff-to-pgm "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "PGM:$out_dir/oracle-gradient-transcode.pgm"
run_timed oracle-ff-to-png "$oracle" "FARBFELD:$fixture_dir/gradient-64.ff" "PNG:$out_dir/oracle-gradient-transcode.png"
run_timed oracle-ppm-to-jpeg "$oracle" "PPM:$fixture_dir/gradient-64.ppm" -quality 90 -sampling-factor 4:4:4 -interlace none -strip "JPEG:$out_dir/oracle-gradient-transcode.jpg"
run_timed oracle-jpeg-to-ff "$oracle" "JPEG:$fixture_dir/gradient-64.jpg" "FARBFELD:$out_dir/oracle-jpeg.ff"
run_timed oracle-qoi-to-ff "$oracle" "QOI:$fixture_dir/qoi-rgba-2x2.qoi" "FARBFELD:$out_dir/oracle-qoi-rgba.ff"
run_timed oracle-bmp-to-ff "$oracle" "BMP:$fixture_dir/gradient-64.bmp" "FARBFELD:$out_dir/oracle-bmp.ff"
run_timed oracle-pbm-to-ff "$oracle" "PBM:$fixture_dir/gradient-64.pbm" "FARBFELD:$out_dir/oracle-pbm.ff"
run_timed oracle-ppm-to-ff "$oracle" "PPM:$fixture_dir/gradient-64.ppm" "FARBFELD:$out_dir/oracle-ppm.ff"
run_timed oracle-ppm16-to-ff "$oracle" "PPM:$fixture_dir/gradient-64-ppm16.ppm" "FARBFELD:$out_dir/oracle-ppm16.ff"
run_timed oracle-pgm-to-ff "$oracle" "PGM:$fixture_dir/gradient-64.pgm" "FARBFELD:$out_dir/oracle-pgm.ff"
run_timed oracle-png-to-ff "$oracle" "PNG:$fixture_dir/gradient-64.png" "FARBFELD:$out_dir/oracle-png.ff"
run_timed oracle-png16-to-ff "$oracle" "PNG:$fixture_dir/gradient-64-png16.png" "FARBFELD:$out_dir/oracle-png16.ff"
run_timed oracle-ppm-resize "$oracle" "PPM:$fixture_dir/gradient-64.ppm" -filter Point -resize 17x11! "PPM:$out_dir/oracle-resized.ppm"
run_timed oracle-bmp-resize "$oracle" "BMP:$fixture_dir/gradient-64.bmp" -filter Point -resize 17x11! "BMP:$out_dir/oracle-resized.bmp"
run_timed oracle-ppm-resize-fit "$oracle" "PPM:$fixture_dir/intake-comments-2x1.ppm" -filter Point -resize 17x11 "PPM:$out_dir/oracle-resized-fit.ppm"
run_timed oracle-bmp-resize-fit "$oracle" "BMP:$fixture_dir/gradient-64.bmp" -filter Point -resize 17x11 "BMP:$out_dir/oracle-resized-fit.bmp"
run_timed oracle-batch-convert-mixed-to-ppm bash -c '"$0" "PPM:$1" -filter Point -resize 17x11 "PPM:$5/batch-ppm.ppm" && "$0" "PGM:$2" -filter Point -resize 17x11 "PPM:$5/batch-pgm.ppm" && "$0" "PNG:$3" -filter Point -resize 17x11 "PPM:$5/batch-png.ppm" && "$0" "BMP:$4" -filter Point -resize 17x11 "PPM:$5/batch-bmp.ppm"' "$oracle" "$out_dir/batch-ppm.ppm" "$out_dir/batch-pgm.pgm" "$out_dir/batch-png.png" "$out_dir/batch-bmp.bmp" "$out_dir/oracle-batch"

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$out_dir"/*.{ff,jpg,qoi,bmp,pbm,ppm,pgm,png} >"$out_dir/output-sha256.txt" 2>/dev/null || true
else
  sha256sum "$out_dir"/*.{ff,jpg,qoi,bmp,pbm,ppm,pgm,png} >"$out_dir/output-sha256.txt" 2>/dev/null || true
fi

cargo run -p imx-cli --bin imx-summarize-bench -- "$out_dir"
bash scripts/check-bench-thresholds.sh "$out_dir"

echo "$out_dir"
