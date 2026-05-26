#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

oracle="${IMAGEMAGICK_MAGICK:-magick}"
if ! "$oracle" -version >/dev/null 2>&1; then
  echo "error: ImageMagick oracle is not runnable: $oracle" >&2
  exit 2
fi

imx="${IMX_STANDALONE_BIN:-$root/target/debug/imx}"
if [[ ! -x "$imx" ]]; then
  cargo build -p imx-cli --bin imx >/dev/null
fi

stamp="$(date +%Y%m%d-%H%M%S)"
out_dir="${IMX_DIFFERENTIAL_CORPUS_OUT:-$root/target/differential-corpus-$stamp}"
fixture_dir="$out_dir/fixtures"
mkdir -p "$fixture_dir"

cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir" >/dev/null
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.qoi"
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.jpg"
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.pgm"
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.png"
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.ppm"
"$imx" "$fixture_dir/gradient-64.ppm" "$fixture_dir/jpeg-source-64.ff"
"$imx" "$fixture_dir/gradient-64.ppm" "$fixture_dir/jpeg-source-64.qoi"
"$imx" "$fixture_dir/gradient-64.ppm" "$fixture_dir/jpeg-source-64.png"

results="$out_dir/results.jsonl"
jpeg_metrics="$out_dir/jpeg-metrics.jsonl"
summary="$out_dir/summary.json"
: >"$results"
: >"$jpeg_metrics"

formats=(farbfeld jpeg qoi pbm pgm png ppm)

format_label() {
  case "$1" in
    farbfeld) echo "FARBFELD" ;;
    jpeg) echo "JPEG" ;;
    qoi) echo "QOI" ;;
    pbm) echo "PBM" ;;
    pgm) echo "PGM" ;;
    png) echo "PNG" ;;
    ppm) echo "PPM" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

format_ext() {
  case "$1" in
    farbfeld) echo "ff" ;;
    jpeg) echo "jpg" ;;
    qoi) echo "qoi" ;;
    pbm) echo "pbm" ;;
    pgm) echo "pgm" ;;
    png) echo "png" ;;
    ppm) echo "ppm" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

fixture_path() {
  case "$1" in
    farbfeld) echo "$fixture_dir/gradient-64.ff" ;;
    jpeg) echo "$fixture_dir/gradient-64.jpg" ;;
    qoi) echo "$fixture_dir/gradient-64.qoi" ;;
    pbm) echo "$fixture_dir/gradient-64.pbm" ;;
    pgm) echo "$fixture_dir/gradient-64.pgm" ;;
    png) echo "$fixture_dir/gradient-64.png" ;;
    ppm) echo "$fixture_dir/gradient-64.ppm" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

ppm16_fixture_path() {
  echo "$fixture_dir/gradient-64-ppm16.ppm"
}

png16_fixture_path() {
  echo "$fixture_dir/gradient-64-png16.png"
}

fixture_path_for_case() {
  local src="$1"
  local dst="$2"
  if [[ "$dst" == "pbm" && "$src" != "pbm" ]]; then
    case "$src" in
      farbfeld) echo "$fixture_dir/pbm-threshold-4x1.ff" ;;
      jpeg) echo "$fixture_dir/pbm-threshold-4x1.jpg" ;;
      qoi) echo "$fixture_dir/pbm-threshold-4x1.qoi" ;;
      pgm) echo "$fixture_dir/pbm-threshold-4x1.pgm" ;;
      png) echo "$fixture_dir/pbm-threshold-4x1.png" ;;
      ppm) echo "$fixture_dir/pbm-threshold-4x1.ppm" ;;
      *) fixture_path "$src" ;;
    esac
    return
  fi
  if [[ "$dst" == "jpeg" ]]; then
    case "$src" in
      farbfeld) echo "$fixture_dir/jpeg-source-64.ff" ;;
      qoi) echo "$fixture_dir/jpeg-source-64.qoi" ;;
      png) echo "$fixture_dir/jpeg-source-64.png" ;;
      *) fixture_path "$src" ;;
    esac
    return
  fi
  fixture_path "$src"
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

record() {
  local case_id="$1"
  local status="$2"
  local detail="$3"
  printf '{"schema_version":1,"case_id":"%s","status":"%s","detail":"%s"}\n' \
    "$(json_escape "$case_id")" \
    "$(json_escape "$status")" \
    "$(json_escape "$detail")" >>"$results"
}

record_jpeg_metrics() {
  local case_id="$1"
  local lhs="$2"
  local rhs="$3"
  python3 - "$case_id" "$lhs" "$rhs" "$jpeg_metrics" <<'PY'
import json
import math
import sys

case_id, lhs_path, rhs_path, metrics_path = sys.argv[1:5]
lhs = open(lhs_path, "rb").read()
rhs = open(rhs_path, "rb").read()
if len(lhs) != len(rhs):
    row = {
        "schema_version": 1,
        "case_id": case_id,
        "status": "failed",
        "detail": f"raw length mismatch {len(lhs)} != {len(rhs)}",
    }
else:
    diffs = [abs(a - b) for a, b in zip(lhs, rhs)]
    count = len(diffs)
    max_abs_diff = max(diffs) if diffs else 0
    mae = sum(diffs) / count if count else 0.0
    mse = sum(d * d for d in diffs) / count if count else 0.0
    rmse = math.sqrt(mse)
    psnr = 99.0 if mse == 0 else 20.0 * math.log10(255.0 / rmse)
    sorted_diffs = sorted(diffs)
    p99 = sorted_diffs[min(count - 1, int(math.ceil(count * 0.99)) - 1)] if count else 0
    over_8 = sum(1 for d in diffs if d > 8)
    over_16 = sum(1 for d in diffs if d > 16)
    passed = (
        max_abs_diff <= 128
        and mae <= 12.0
        and rmse <= 20.0
        and p99 <= 80
        and psnr >= 22.0
    )
    row = {
        "schema_version": 1,
        "case_id": case_id,
        "status": "passed" if passed else "failed",
        "raw_bytes": count,
        "max_abs_diff": max_abs_diff,
        "mae": round(mae, 6),
        "rmse": round(rmse, 6),
        "psnr_db": round(psnr, 6),
        "p99_abs_diff": p99,
        "channels_over_8": over_8,
        "channels_over_16": over_16,
    }
with open(metrics_path, "a", encoding="utf-8") as handle:
    handle.write(json.dumps(row, separators=(",", ":")) + "\n")
if row["status"] != "passed":
    print(json.dumps(row, indent=2), file=sys.stderr)
    sys.exit(1)
PY
}

failures=0
passes=0
jpeg_metric_cases=0
jpeg_orientation_cases=0

run_identify_case() {
  local fmt="$1"
  local label input imx_out oracle_out
  label="$(format_label "$fmt")"
  input="$(fixture_path "$fmt")"
  imx_out="$out_dir/identify-$fmt.imx.txt"
  oracle_out="$out_dir/identify-$fmt.oracle.txt"

  if "$imx" identify "$input" >"$imx_out" 2>"$out_dir/identify-$fmt.imx.stderr" &&
    "$oracle" identify -format '%m %w %h %[colorspace] %[depth]' "$label:$input" >"$oracle_out" 2>"$out_dir/identify-$fmt.oracle.stderr"; then
    record "identify.$fmt" passed "$label identify accepted by IMX and ImageMagick"
    passes=$((passes + 1))
  else
    record "identify.$fmt" failed "$label identify failed in IMX or ImageMagick"
    failures=$((failures + 1))
  fi
}

run_prefixed_identify_case() {
  local fmt="$1"
  local label input imx_out oracle_out
  label="$(format_label "$fmt")"
  input="$(fixture_path "$fmt")"
  imx_out="$out_dir/identify-prefixed-$fmt.imx.txt"
  oracle_out="$out_dir/identify-prefixed-$fmt.oracle.txt"

  if "$imx" identify "$label:$input" >"$imx_out" 2>"$out_dir/identify-prefixed-$fmt.imx.stderr" &&
    "$oracle" identify -format '%m %w %h %[colorspace] %[depth]' "$label:$input" >"$oracle_out" 2>"$out_dir/identify-prefixed-$fmt.oracle.stderr"; then
    record "identify-prefixed.$fmt" passed "$label-prefixed identify accepted by IMX and ImageMagick"
    passes=$((passes + 1))
  else
    record "identify-prefixed.$fmt" failed "$label-prefixed identify failed in IMX or ImageMagick"
    failures=$((failures + 1))
  fi
}

run_ppm16_identify_case() {
  local mode="$1"
  local input imx_input case_id imx_out oracle_out
  input="$(ppm16_fixture_path)"
  case_id="identify-ppm16"
  imx_input="$input"
  if [[ "$mode" == "prefixed" ]]; then
    case_id="identify-prefixed-ppm16"
    imx_input="PPM:$input"
  fi
  imx_out="$out_dir/$case_id.imx.txt"
  oracle_out="$out_dir/$case_id.oracle.txt"

  if "$imx" identify "$imx_input" >"$imx_out" 2>"$out_dir/$case_id.imx.stderr" &&
    "$oracle" identify -format '%m %w %h %[colorspace] %[depth]' "PPM:$input" >"$oracle_out" 2>"$out_dir/$case_id.oracle.stderr"; then
    if grep -q 'depth=16' "$imx_out"; then
      record "$case_id" passed "high-depth PPM identify accepted by IMX and ImageMagick"
      passes=$((passes + 1))
    else
      record "$case_id" failed "high-depth PPM identify did not report depth=16"
      failures=$((failures + 1))
    fi
  else
    record "$case_id" failed "high-depth PPM identify failed in IMX or ImageMagick"
    failures=$((failures + 1))
  fi
}

run_png16_identify_case() {
  local mode="$1"
  local input imx_input case_id imx_out oracle_out
  input="$(png16_fixture_path)"
  case_id="identify-png16"
  imx_input="$input"
  if [[ "$mode" == "prefixed" ]]; then
    case_id="identify-prefixed-png16"
    imx_input="PNG:$input"
  fi
  imx_out="$out_dir/$case_id.imx.txt"
  oracle_out="$out_dir/$case_id.oracle.txt"

  if "$imx" identify "$imx_input" >"$imx_out" 2>"$out_dir/$case_id.imx.stderr" &&
    "$oracle" identify -format '%m %w %h %[colorspace] %[depth]' "PNG:$input" >"$oracle_out" 2>"$out_dir/$case_id.oracle.stderr"; then
    if grep -q 'depth=16' "$imx_out"; then
      record "$case_id" passed "high-depth PNG identify accepted by IMX and ImageMagick"
      passes=$((passes + 1))
    else
      record "$case_id" failed "high-depth PNG identify did not report depth=16"
      failures=$((failures + 1))
    fi
  else
    record "$case_id" failed "high-depth PNG identify failed in IMX or ImageMagick"
    failures=$((failures + 1))
  fi
}

run_jpeg_orientation_case() {
  local orientation="$1"
  local input case_id imx_identify imx_output oracle_output imx_raw oracle_raw expected_dimensions
  input="$fixture_dir/photo-orientation-o$orientation.jpg"
  case_id="jpeg-orientation.o$orientation"
  imx_identify="$out_dir/$case_id.identify.imx.txt"
  imx_output="$out_dir/$case_id.imx.ppm"
  oracle_output="$out_dir/$case_id.oracle.ppm"
  imx_raw="$out_dir/$case_id.imx.rgb"
  oracle_raw="$out_dir/$case_id.oracle.rgb"

  if ((orientation >= 5)); then
    expected_dimensions="width=2 height=3"
  else
    expected_dimensions="width=3 height=2"
  fi

  if "$imx" identify "JPEG:$input" >"$imx_identify" 2>"$out_dir/$case_id.identify.imx.stderr" &&
    grep -q "format=JPEG $expected_dimensions channels=RGB depth=8" "$imx_identify"; then
    record "$case_id.identify" passed "IMX reported EXIF-oriented JPEG dimensions"
    passes=$((passes + 1))
  else
    record "$case_id.identify" failed "IMX did not report EXIF-oriented JPEG dimensions"
    failures=$((failures + 1))
    return
  fi

  if ! "$imx" "JPEG:$input" "PPM:$imx_output" >"$out_dir/$case_id.imx.stdout" 2>"$out_dir/$case_id.imx.stderr"; then
    record "$case_id.transcode" failed "IMX JPEG orientation transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "JPEG:$input" -auto-orient "PPM:$oracle_output" >"$out_dir/$case_id.oracle.stdout" 2>"$out_dir/$case_id.oracle.stderr"; then
    record "$case_id.transcode" failed "ImageMagick JPEG -auto-orient transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "PPM:$imx_output" -depth 8 "RGB:$imx_raw" >"$out_dir/$case_id.imx-decode.stdout" 2>"$out_dir/$case_id.imx-decode.stderr"; then
    record "$case_id.transcode" failed "ImageMagick could not decode IMX oriented output"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "PPM:$oracle_output" -depth 8 "RGB:$oracle_raw" >"$out_dir/$case_id.oracle-decode.stdout" 2>"$out_dir/$case_id.oracle-decode.stderr"; then
    record "$case_id.transcode" failed "ImageMagick could not decode oracle oriented output"
    failures=$((failures + 1))
    return
  fi

  if record_jpeg_metrics "$case_id.transcode" "$imx_raw" "$oracle_raw" >"$out_dir/$case_id.metrics.stdout" 2>"$out_dir/$case_id.metrics.stderr"; then
    record "$case_id.transcode" passed "IMX JPEG orientation output is within ImageMagick -auto-orient tolerance"
    passes=$((passes + 1))
    jpeg_metric_cases=$((jpeg_metric_cases + 1))
    jpeg_orientation_cases=$((jpeg_orientation_cases + 1))
  else
    record "$case_id.transcode" failed "IMX JPEG orientation output exceeds ImageMagick -auto-orient tolerance"
    failures=$((failures + 1))
  fi
}

run_transcode_case() {
  local src="$1"
  local dst="$2"
  local mode="${3:-plain}"
  local src_label dst_label input imx_input imx_output imx_output_arg oracle_output imx_raw oracle_raw case_id raw_format
  local -a oracle_args
  src_label="$(format_label "$src")"
  dst_label="$(format_label "$dst")"
  input="$(fixture_path_for_case "$src" "$dst")"
  case_id="transcode.$src.$dst"
  imx_input="$input"
  if [[ "$mode" == "prefixed" ]]; then
    case_id="transcode-prefixed.$src.$dst"
    imx_input="$src_label:$input"
  fi
  imx_output="$out_dir/$case_id.imx.$(format_ext "$dst")"
  imx_output_arg="$imx_output"
  if [[ "$mode" == "prefixed" ]]; then
    imx_output_arg="$dst_label:$imx_output"
  fi
  oracle_output="$out_dir/$case_id.oracle.$(format_ext "$dst")"
  imx_raw="$out_dir/$case_id.imx.rgba"
  oracle_raw="$out_dir/$case_id.oracle.rgba"
  raw_format="RGBA"
  if [[ "$src" == "jpeg" || "$dst" == "jpeg" ]]; then
    imx_raw="$out_dir/$case_id.imx.rgb"
    oracle_raw="$out_dir/$case_id.oracle.rgb"
    raw_format="RGB"
  fi

  if ! "$imx" "$imx_input" "$imx_output_arg" >"$out_dir/$case_id.imx.stdout" 2>"$out_dir/$case_id.imx.stderr"; then
    record "$case_id" failed "IMX transcode failed"
    failures=$((failures + 1))
    return
  fi

  oracle_args=("$src_label:$input")
  if [[ "$dst" == "jpeg" ]]; then
    oracle_args+=("-quality" "90" "-sampling-factor" "4:4:4" "-interlace" "none" "-strip")
  fi
  oracle_args+=("$dst_label:$oracle_output")
  if ! "$oracle" "${oracle_args[@]}" >"$out_dir/$case_id.oracle.stdout" 2>"$out_dir/$case_id.oracle.stderr"; then
    record "$case_id" failed "ImageMagick oracle transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$imx_output" -depth 8 "$raw_format:$imx_raw" >"$out_dir/$case_id.imx-decode.stdout" 2>"$out_dir/$case_id.imx-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode IMX output"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$oracle_output" -depth 8 "$raw_format:$oracle_raw" >"$out_dir/$case_id.oracle-decode.stdout" 2>"$out_dir/$case_id.oracle-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode oracle output"
    failures=$((failures + 1))
    return
  fi

  if [[ "$src" == "jpeg" || "$dst" == "jpeg" ]]; then
    if record_jpeg_metrics "$case_id" "$imx_raw" "$oracle_raw" >"$out_dir/$case_id.metrics.stdout" 2>"$out_dir/$case_id.metrics.stderr"; then
      record "$case_id" passed "$src_label to $dst_label decoded RGB pixels are within JPEG tolerance"
      passes=$((passes + 1))
      jpeg_metric_cases=$((jpeg_metric_cases + 1))
    else
      record "$case_id" failed "$src_label to $dst_label decoded RGB pixels exceed JPEG tolerance"
      failures=$((failures + 1))
    fi
  elif cmp -s "$imx_raw" "$oracle_raw"; then
    record "$case_id" passed "$src_label to $dst_label decoded pixels match oracle output"
    passes=$((passes + 1))
  else
    record "$case_id" failed "$src_label to $dst_label decoded pixels differ from oracle output"
    failures=$((failures + 1))
  fi
}

run_ppm16_transcode_case() {
  local dst="$1"
  local dst_label input imx_output oracle_output imx_raw oracle_raw case_id raw_format
  dst_label="$(format_label "$dst")"
  input="$(ppm16_fixture_path)"
  case_id="transcode.ppm16.$dst"
  imx_output="$out_dir/$case_id.imx.$(format_ext "$dst")"
  oracle_output="$out_dir/$case_id.oracle.$(format_ext "$dst")"
  imx_raw="$out_dir/$case_id.imx.raw"
  oracle_raw="$out_dir/$case_id.oracle.raw"
  raw_format="RGB"
  if [[ "$dst" == "farbfeld" ]]; then
    raw_format="RGB"
  fi

  if ! "$imx" "PPM:$input" "$dst_label:$imx_output" >"$out_dir/$case_id.imx.stdout" 2>"$out_dir/$case_id.imx.stderr"; then
    record "$case_id" failed "IMX high-depth PPM transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "PPM:$input" "$dst_label:$oracle_output" >"$out_dir/$case_id.oracle.stdout" 2>"$out_dir/$case_id.oracle.stderr"; then
    record "$case_id" failed "ImageMagick high-depth PPM transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$imx_output" -depth 16 -endian MSB "$raw_format:$imx_raw" >"$out_dir/$case_id.imx-decode.stdout" 2>"$out_dir/$case_id.imx-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode high-depth IMX output"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$oracle_output" -depth 16 -endian MSB "$raw_format:$oracle_raw" >"$out_dir/$case_id.oracle-decode.stdout" 2>"$out_dir/$case_id.oracle-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode high-depth oracle output"
    failures=$((failures + 1))
    return
  fi

  if cmp -s "$imx_raw" "$oracle_raw"; then
    record "$case_id" passed "high-depth PPM to $dst_label decoded 16-bit pixels match oracle output"
    passes=$((passes + 1))
  else
    record "$case_id" failed "high-depth PPM to $dst_label decoded 16-bit pixels differ from oracle output"
    failures=$((failures + 1))
  fi
}

run_png16_transcode_case() {
  local dst="$1"
  local dst_label input imx_output oracle_output imx_raw oracle_raw case_id raw_format
  dst_label="$(format_label "$dst")"
  input="$(png16_fixture_path)"
  case_id="transcode.png16.$dst"
  imx_output="$out_dir/$case_id.imx.$(format_ext "$dst")"
  oracle_output="$out_dir/$case_id.oracle.$(format_ext "$dst")"
  imx_raw="$out_dir/$case_id.imx.raw"
  oracle_raw="$out_dir/$case_id.oracle.raw"
  raw_format="RGB"
  if [[ "$dst" == "farbfeld" ]]; then
    raw_format="RGBA"
  fi

  if ! "$imx" "PNG:$input" "$dst_label:$imx_output" >"$out_dir/$case_id.imx.stdout" 2>"$out_dir/$case_id.imx.stderr"; then
    record "$case_id" failed "IMX high-depth PNG transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "PNG:$input" "$dst_label:$oracle_output" >"$out_dir/$case_id.oracle.stdout" 2>"$out_dir/$case_id.oracle.stderr"; then
    record "$case_id" failed "ImageMagick high-depth PNG transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$imx_output" -depth 16 -endian MSB "$raw_format:$imx_raw" >"$out_dir/$case_id.imx-decode.stdout" 2>"$out_dir/$case_id.imx-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode high-depth PNG IMX output"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$oracle_output" -depth 16 -endian MSB "$raw_format:$oracle_raw" >"$out_dir/$case_id.oracle-decode.stdout" 2>"$out_dir/$case_id.oracle-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode high-depth PNG oracle output"
    failures=$((failures + 1))
    return
  fi

  if cmp -s "$imx_raw" "$oracle_raw"; then
    record "$case_id" passed "high-depth PNG to $dst_label decoded 16-bit pixels match oracle output"
    passes=$((passes + 1))
  else
    record "$case_id" failed "high-depth PNG to $dst_label decoded 16-bit pixels differ from oracle output"
    failures=$((failures + 1))
  fi
}

for fmt in "${formats[@]}"; do
  run_identify_case "$fmt"
  run_prefixed_identify_case "$fmt"
done
run_ppm16_identify_case plain
run_ppm16_identify_case prefixed
run_png16_identify_case plain
run_png16_identify_case prefixed

for orientation in 1 2 3 4 5 6 7 8; do
  run_jpeg_orientation_case "$orientation"
done

for src in "${formats[@]}"; do
  for dst in "${formats[@]}"; do
    run_transcode_case "$src" "$dst"
  done
done
run_ppm16_transcode_case farbfeld
run_ppm16_transcode_case ppm
run_png16_transcode_case farbfeld
run_png16_transcode_case ppm

for prefixed_pair in farbfeld:jpeg jpeg:qoi qoi:png png:ppm ppm:pgm pgm:pbm pbm:farbfeld; do
  run_transcode_case "${prefixed_pair%%:*}" "${prefixed_pair##*:}" prefixed
done

status="passed"
if [[ "$failures" != "0" ]]; then
  status="failed"
fi

cat >"$summary" <<EOF
{
  "schema_version": 1,
  "status": "$status",
  "imx": "$imx",
  "oracle": "$oracle",
  "fixture_manifest": "fixtures/manifest.json",
  "results": "results.jsonl",
  "jpeg_metrics": "jpeg-metrics.jsonl",
  "identify_cases": 18,
  "transcode_cases": 60,
  "jpeg_metric_cases": $jpeg_metric_cases,
  "jpeg_orientation_cases": $jpeg_orientation_cases,
  "passes": $passes,
  "failures": $failures
}
EOF

if [[ "$status" != "passed" ]]; then
  echo "error: differential corpus failed; see $out_dir" >&2
  exit 1
fi

echo "$out_dir"
