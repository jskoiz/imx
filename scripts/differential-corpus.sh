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
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.pgm"
"$imx" "$fixture_dir/pbm-threshold-4x1.ff" "$fixture_dir/pbm-threshold-4x1.ppm"

results="$out_dir/results.jsonl"
summary="$out_dir/summary.json"
: >"$results"

formats=(farbfeld qoi pbm pgm ppm)

format_label() {
  case "$1" in
    farbfeld) echo "FARBFELD" ;;
    qoi) echo "QOI" ;;
    pbm) echo "PBM" ;;
    pgm) echo "PGM" ;;
    ppm) echo "PPM" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

format_ext() {
  case "$1" in
    farbfeld) echo "ff" ;;
    qoi) echo "qoi" ;;
    pbm) echo "pbm" ;;
    pgm) echo "pgm" ;;
    ppm) echo "ppm" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

fixture_path() {
  case "$1" in
    farbfeld) echo "$fixture_dir/gradient-64.ff" ;;
    qoi) echo "$fixture_dir/gradient-64.qoi" ;;
    pbm) echo "$fixture_dir/gradient-64.pbm" ;;
    pgm) echo "$fixture_dir/gradient-64.pgm" ;;
    ppm) echo "$fixture_dir/gradient-64.ppm" ;;
    *) echo "error: unknown format $1" >&2; exit 2 ;;
  esac
}

fixture_path_for_case() {
  local src="$1"
  local dst="$2"
  if [[ "$dst" == "pbm" && "$src" != "pbm" ]]; then
    case "$src" in
      farbfeld) echo "$fixture_dir/pbm-threshold-4x1.ff" ;;
      qoi) echo "$fixture_dir/pbm-threshold-4x1.qoi" ;;
      pgm) echo "$fixture_dir/pbm-threshold-4x1.pgm" ;;
      ppm) echo "$fixture_dir/pbm-threshold-4x1.ppm" ;;
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

failures=0
passes=0

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

run_transcode_case() {
  local src="$1"
  local dst="$2"
  local src_label dst_label input imx_output oracle_output imx_raw oracle_raw case_id
  src_label="$(format_label "$src")"
  dst_label="$(format_label "$dst")"
  input="$(fixture_path_for_case "$src" "$dst")"
  case_id="transcode.$src.$dst"
  imx_output="$out_dir/$case_id.imx.$(format_ext "$dst")"
  oracle_output="$out_dir/$case_id.oracle.$(format_ext "$dst")"
  imx_raw="$out_dir/$case_id.imx.rgba"
  oracle_raw="$out_dir/$case_id.oracle.rgba"

  if ! "$imx" "$input" "$imx_output" >"$out_dir/$case_id.imx.stdout" 2>"$out_dir/$case_id.imx.stderr"; then
    record "$case_id" failed "IMX transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$src_label:$input" "$dst_label:$oracle_output" >"$out_dir/$case_id.oracle.stdout" 2>"$out_dir/$case_id.oracle.stderr"; then
    record "$case_id" failed "ImageMagick oracle transcode failed"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$imx_output" -depth 8 "RGBA:$imx_raw" >"$out_dir/$case_id.imx-decode.stdout" 2>"$out_dir/$case_id.imx-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode IMX output"
    failures=$((failures + 1))
    return
  fi

  if ! "$oracle" "$dst_label:$oracle_output" -depth 8 "RGBA:$oracle_raw" >"$out_dir/$case_id.oracle-decode.stdout" 2>"$out_dir/$case_id.oracle-decode.stderr"; then
    record "$case_id" failed "ImageMagick could not decode oracle output"
    failures=$((failures + 1))
    return
  fi

  if cmp -s "$imx_raw" "$oracle_raw"; then
    record "$case_id" passed "$src_label to $dst_label decoded pixels match oracle output"
    passes=$((passes + 1))
  else
    record "$case_id" failed "$src_label to $dst_label decoded pixels differ from oracle output"
    failures=$((failures + 1))
  fi
}

for fmt in "${formats[@]}"; do
  run_identify_case "$fmt"
done

for src in "${formats[@]}"; do
  for dst in "${formats[@]}"; do
    run_transcode_case "$src" "$dst"
  done
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
  "identify_cases": 5,
  "transcode_cases": 25,
  "passes": $passes,
  "failures": $failures
}
EOF

if [[ "$status" != "passed" ]]; then
  echo "error: differential corpus failed; see $out_dir" >&2
  exit 1
fi

echo "$out_dir"
