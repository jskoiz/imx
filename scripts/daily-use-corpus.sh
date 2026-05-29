#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

out_dir="${IMX_DAILY_USE_OUT:-$root/target/daily-use-corpus}"
fixture_dir="${IMX_DAILY_USE_FIXTURE_DIR:-$out_dir/generated-fixtures}"
imx="${IMX_DAILY_USE_BIN:-$root/target/debug/imx}"
runner=()
if [[ -n "${IMX_DAILY_USE_RUNNER:-}" ]]; then
  read -r -a runner <<<"$IMX_DAILY_USE_RUNNER"
fi

rm -rf "$out_dir"
mkdir -p "$out_dir" "$fixture_dir" "$out_dir/outputs" "$out_dir/malformed"

if [[ -n "${IMX_DAILY_USE_BIN:-}" ]]; then
  if [[ ! -f "$imx" ]]; then
    echo "error: IMX_DAILY_USE_BIN does not exist: $imx" >&2
    exit 2
  fi
elif [[ ! -x "$imx" ]]; then
  cargo build -p imx-cli --bin imx >/dev/null
fi

if [[ -z "${IMX_DAILY_USE_FIXTURE_DIR:-}" ]]; then
  cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir" >/dev/null
elif [[ ! -f "$fixture_dir/manifest.json" ]]; then
  echo "error: fixture manifest is missing: $fixture_dir/manifest.json" >&2
  exit 2
fi

results="$out_dir/results.jsonl"
manifest="$out_dir/manifest.json"
summary="$out_dir/summary.json"
: >"$results"

json_escape() {
  python3 - "$1" <<'PY'
import json
import sys
print(json.dumps(sys.argv[1])[1:-1])
PY
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

run_imx() {
  if ((${#runner[@]})); then
    "${runner[@]}" "$imx" "$@"
  else
    "$imx" "$@"
  fi
}

expected_json() {
  local format="$1"
  local width="$2"
  local height="$3"
  local channels="$4"
  local depth="$5"
  printf '{"schema_version":1,"format":"%s","width":%s,"height":%s,"channels":"%s","depth":%s}' \
    "$format" "$width" "$height" "$channels" "$depth"
}

expected_report_json() {
  local format="$1"
  local width="$2"
  local height="$3"
  local channels="$4"
  local depth="$5"
  local frames="${6:-1}"
  printf '{"schema_version":2,"status":"supported","diagnostic_code":null,"format":"%s","width":%s,"height":%s,"channels":"%s","depth":%s,"frames":%s}' \
    "$format" "$width" "$height" "$channels" "$depth" "$frames"
}

assert_file_exact() {
  local path="$1"
  local expected="$2"
  local actual
  actual="$(cat "$path")"
  if [[ "$actual" != "$expected" ]]; then
    echo "error: $path mismatch" >&2
    echo "expected: $expected" >&2
    echo "actual:   $actual" >&2
    return 1
  fi
}

assert_diagnostic_json() {
  local path="$1"
  local expected_status="$2"
  local expected_code="$3"
  local expected_schema="${4:-1}"
  python3 - "$path" "$expected_status" "$expected_code" "$expected_schema" <<'PY'
import json
import sys

path, expected_status, expected_code, expected_schema = sys.argv[1:5]
payload = json.loads(open(path, encoding="utf-8").read())
if payload.get("schema_version") != int(expected_schema):
    raise SystemExit(f"{path}: schema_version mismatch: {payload!r}")
if payload.get("status") != expected_status:
    raise SystemExit(f"{path}: status mismatch: {payload!r}")
if payload.get("diagnostic_code") != expected_code:
    raise SystemExit(f"{path}: diagnostic_code mismatch: {payload!r}")
if expected_status == "unsupported" and not payload.get("message"):
    raise SystemExit(f"{path}: unsupported diagnostic did not include message: {payload!r}")
PY
}

passes=0
failures=0
supported_cases=0
transcode_cases=0
diagnostic_cases=0
identify_error_cases=0

run_supported_case() {
  local case_id="$1"
  local prefix="$2"
  local file_name="$3"
  local format="$4"
  local width="$5"
  local height="$6"
  local channels="$7"
  local depth="$8"
  local frames="${9:-1}"
  local input="$fixture_dir/$file_name"
  local arg="$prefix:$input"
  local identify_json="$out_dir/$case_id.identify.json"
  local report_json="$out_dir/$case_id.report.json"
  local expected_identify expected_report
  supported_cases=$((supported_cases + 1))
  expected_identify="$(expected_json "$format" "$width" "$height" "$channels" "$depth")"
  expected_report="$(expected_report_json "$format" "$width" "$height" "$channels" "$depth" "$frames")"
  if run_imx identify --json "$arg" >"$identify_json" 2>"$out_dir/$case_id.identify.stderr" &&
    assert_file_exact "$identify_json" "$expected_identify" &&
    run_imx report --json "$arg" >"$report_json" 2>"$out_dir/$case_id.report.stderr" &&
    assert_file_exact "$report_json" "$expected_report"; then
    record "supported.$case_id" passed "$prefix identify/report JSON matched expected metadata"
    passes=$((passes + 1))
  else
    record "supported.$case_id" failed "$prefix identify/report JSON did not match expected metadata"
    failures=$((failures + 1))
  fi
}

run_transcode_case() {
  local case_id="$1"
  local input_prefix="$2"
  local input_file="$3"
  local output_prefix="$4"
  local output_file="$5"
  local format="$6"
  local width="$7"
  local height="$8"
  local channels="$9"
  local depth="${10}"
  local input="$fixture_dir/$input_file"
  local output="$out_dir/outputs/$output_file"
  local report_json="$out_dir/$case_id.output.report.json"
  local expected_report
  transcode_cases=$((transcode_cases + 1))
  expected_report="$(expected_report_json "$format" "$width" "$height" "$channels" "$depth")"
  if run_imx "$input_prefix:$input" "$output_prefix:$output" >"$out_dir/$case_id.transcode.stdout" 2>"$out_dir/$case_id.transcode.stderr" &&
    run_imx report --json "$output_prefix:$output" >"$report_json" 2>"$out_dir/$case_id.report.stderr" &&
    assert_file_exact "$report_json" "$expected_report"; then
    record "transcode.$case_id" passed "$input_prefix to $output_prefix output reported expected metadata"
    passes=$((passes + 1))
  else
    record "transcode.$case_id" failed "$input_prefix to $output_prefix output did not report expected metadata"
    failures=$((failures + 1))
  fi
}

run_report_diagnostic_case() {
  local case_id="$1"
  local arg="$2"
  local expected_code="$3"
  local report_json="$out_dir/$case_id.report.json"
  diagnostic_cases=$((diagnostic_cases + 1))
  if run_imx report --json "$arg" >"$report_json" 2>"$out_dir/$case_id.report.stderr" &&
    assert_diagnostic_json "$report_json" unsupported "$expected_code" 2; then
    record "diagnostic.$case_id" passed "report --json returned $expected_code"
    passes=$((passes + 1))
  else
    record "diagnostic.$case_id" failed "report --json did not return $expected_code"
    failures=$((failures + 1))
  fi
}

run_identify_error_case() {
  local case_id="$1"
  local arg="$2"
  local expected_code="$3"
  local stdout="$out_dir/$case_id.identify.stdout"
  local stderr="$out_dir/$case_id.identify.stderr"
  identify_error_cases=$((identify_error_cases + 1))
  if run_imx identify --json "$arg" >"$stdout" 2>"$stderr"; then
    record "identify-error.$case_id" failed "identify --json unexpectedly succeeded"
    failures=$((failures + 1))
  elif [[ -s "$stdout" ]]; then
    record "identify-error.$case_id" failed "identify --json wrote stdout on failure"
    failures=$((failures + 1))
  elif assert_diagnostic_json "$stderr" unsupported "$expected_code"; then
    record "identify-error.$case_id" passed "identify --json stderr returned $expected_code"
    passes=$((passes + 1))
  else
    record "identify-error.$case_id" failed "identify --json stderr did not return $expected_code"
    failures=$((failures + 1))
  fi
}

geometry_cases=0

run_geometry_case() {
  local case_id="$1"
  local input_prefix="$2"
  local input_file="$3"
  shift 3
  local op_args=()
  while [[ "$1" != "--" ]]; do
    op_args+=("$1")
    shift
  done
  shift
  local width="$1"
  local height="$2"
  local channels="$3"
  local depth="$4"
  local input="$fixture_dir/$input_file"
  local output="$out_dir/outputs/$case_id.ppm"
  local report_json="$out_dir/$case_id.geometry.report.json"
  local expected_report
  geometry_cases=$((geometry_cases + 1))
  expected_report="$(expected_report_json PPM "$width" "$height" "$channels" "$depth")"
  if run_imx "${op_args[@]}" "$input_prefix:$input" "PPM:$output" >"$out_dir/$case_id.geometry.stdout" 2>"$out_dir/$case_id.geometry.stderr" &&
    run_imx report --json "PPM:$output" >"$report_json" 2>"$out_dir/$case_id.geometry.report.stderr" &&
    assert_file_exact "$report_json" "$expected_report"; then
    record "geometry.$case_id" passed "${op_args[*]} output reported expected metadata"
    passes=$((passes + 1))
  else
    record "geometry.$case_id" failed "${op_args[*]} output did not report expected metadata"
    failures=$((failures + 1))
  fi
}

run_supported_case gradient-bmp BMP gradient-64.bmp BMP 64 64 RGB 8
run_supported_case gradient-farbfeld FARBFELD gradient-64.ff FARBFELD 64 64 RGBA 16
run_supported_case gradient-jpeg JPEG gradient-64.jpg JPEG 64 64 RGB 8
run_supported_case gradient-qoi QOI gradient-64.qoi QOI 64 64 RGBA 8
run_supported_case gradient-pbm PBM gradient-64.pbm PBM 64 64 GRAY 1
run_supported_case gradient-pgm PGM gradient-64.pgm PGM 64 64 GRAY 16
run_supported_case gradient-png PNG gradient-64.png PNG 64 64 RGBA 8
run_supported_case gradient-ppm PPM gradient-64.ppm PPM 64 64 RGB 8
run_supported_case intake-bmp-top-down-rgba32 BMP intake-top-down-rgba32-2x2.bmp BMP 2 2 RGBA 8
run_supported_case intake-jpeg-progressive-camera JPEG progressive-camera-exif-le-o6.jpg JPEG 3 4 RGB 8
run_supported_case intake-qoi-rgb-linear QOI intake-qoi-rgb-linear-2x2.qoi QOI 2 2 RGB 8
run_supported_case intake-pgm-binary-comments PGM intake-pgm-binary-comments-crlf-3x1.pgm PGM 3 1 GRAY 8
run_supported_case intake-png-rgb16 PNG intake-rgb16-2x1.png PNG 2 1 RGB 16
run_supported_case intake-ppm-binary-comments PPM intake-ppm-binary-comments-crlf-2x1.ppm PPM 2 1 RGB 8
run_supported_case intake-webp-rgb WEBP intake-webp-rgb-2x1.webp WEBP 2 1 RGB 8
run_supported_case intake-webp-rgba WEBP intake-webp-rgba-2x1.webp WEBP 2 1 RGBA 8
run_supported_case intake-gif-rgba GIF intake-gif-rgba-2x1.gif GIF 2 1 RGBA 8
run_supported_case intake-gif-animated GIF intake-gif-animated-2x2-3frames.gif GIF 2 2 RGBA 8 3

run_transcode_case bmp-to-ppm BMP gradient-64.bmp PPM bmp-to-ppm.ppm PPM 64 64 RGB 8
run_transcode_case farbfeld-to-qoi FARBFELD gradient-64.ff QOI farbfeld-to-qoi.qoi QOI 64 64 RGBA 8
run_transcode_case jpeg-to-farbfeld JPEG gradient-64.jpg FARBFELD jpeg-to-farbfeld.ff FARBFELD 64 64 RGBA 16
run_transcode_case qoi-to-png QOI gradient-64.qoi PNG qoi-to-png.png PNG 64 64 RGBA 8
run_transcode_case pbm-to-pgm PBM gradient-64.pbm PGM pbm-to-pgm.pgm PGM 64 64 GRAY 8
run_transcode_case pgm-to-ppm PGM gradient-64.pgm PPM pgm-to-ppm.ppm PPM 64 64 RGB 16
run_transcode_case png-to-farbfeld PNG gradient-64.png FARBFELD png-to-farbfeld.ff FARBFELD 64 64 RGBA 16
run_transcode_case ppm-to-bmp PPM gradient-64.ppm BMP ppm-to-bmp.bmp BMP 64 64 RGB 8
run_transcode_case webp-to-png WEBP intake-webp-rgba-2x1.webp PNG webp-to-png.png PNG 2 1 RGBA 8
run_transcode_case gif-to-png GIF intake-gif-rgba-2x1.gif PNG gif-to-png.png PNG 2 1 RGBA 8

run_geometry_case crop-bmp BMP gradient-64.bmp crop 4x3+2+1 -- 4 3 RGB 8
run_geometry_case rotate90-bmp BMP gradient-64.bmp rotate 90 -- 64 64 RGB 8
run_geometry_case flip-bmp BMP gradient-64.bmp flip -- 64 64 RGB 8
run_geometry_case flop-bmp BMP gradient-64.bmp flop -- 64 64 RGB 8

python3 - "$fixture_dir/gradient-64.bmp" "$out_dir/malformed" <<'PY'
import sys
from pathlib import Path

source = Path(sys.argv[1])
out_dir = Path(sys.argv[2])
out_dir.mkdir(parents=True, exist_ok=True)

(out_dir / "unknown.dat").write_bytes(b"not an image\n")
(out_dir / "bad-max.ppm").write_bytes(b"P3\n1 1\n65536\n0 0 0\n")

bad_qoi = bytearray(b"qoif")
bad_qoi.extend((1).to_bytes(4, "big"))
bad_qoi.extend((1).to_bytes(4, "big"))
bad_qoi.extend(bytes([2, 0]))
(out_dir / "bad-channels.qoi").write_bytes(bad_qoi)

bad_bmp = bytearray(source.read_bytes())
bad_bmp[30:34] = (1).to_bytes(4, "little")
(out_dir / "bad-compression.bmp").write_bytes(bad_bmp)
PY

run_report_diagnostic_case unsupported-prefix "TGA:$fixture_dir/gradient-64.ppm" input.unsupported_format_prefix
run_report_diagnostic_case missing-prefix-path "PNG:" input.missing_prefix_path
run_report_diagnostic_case missing-input "PPM:$out_dir/malformed/missing.ppm" input.missing
run_report_diagnostic_case prefix-mismatch "QOI:$fixture_dir/gradient-64.ppm" input.format_prefix_mismatch
run_report_diagnostic_case unsupported-format "$out_dir/malformed/unknown.dat" input.unsupported_format
run_report_diagnostic_case malformed-qoi "QOI:$out_dir/malformed/bad-channels.qoi" qoi.invalid_channels
run_report_diagnostic_case malformed-pnm "PPM:$out_dir/malformed/bad-max.ppm" pnm.invalid_max_value
run_report_diagnostic_case unsupported-bmp "BMP:$out_dir/malformed/bad-compression.bmp" bmp.unsupported_feature
run_identify_error_case identify-json-malformed-qoi "QOI:$out_dir/malformed/bad-channels.qoi" qoi.invalid_channels
run_identify_error_case identify-json-prefix-mismatch "QOI:$fixture_dir/gradient-64.ppm" input.format_prefix_mismatch

git_rev="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
status="passed"
if [[ "$failures" != "0" ]]; then
  status="failed"
fi

cat >"$manifest" <<EOF
{
  "schema_version": 1,
  "git_rev": "$git_rev",
  "generated_at": "$generated_at",
  "binary": "$imx",
  "runner": "${IMX_DAILY_USE_RUNNER:-}",
  "fixture_manifest": "generated-fixtures/manifest.json",
  "supported_surface": [
    "identify --json for generated BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM/WEBP/GIF fixtures",
    "report --json for generated BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM/WEBP/GIF fixtures",
    "representative prefixed transcodes across all supported format families, including WEBP/GIF decode-only inputs",
    "geometry operations (crop/rotate/flip/flop) reported via report --json",
    "report --json diagnostics for unsupported prefixes, missing paths, missing inputs, prefix mismatches, unsupported formats, malformed QOI/PNM, and unsupported BMP compression",
    "identify --json failure JSON on stderr for malformed and mismatched inputs"
  ]
}
EOF

cat >"$summary" <<EOF
{
  "schema_version": 1,
  "status": "$status",
  "manifest": "manifest.json",
  "generated_fixtures": "generated-fixtures/manifest.json",
  "results": "results.jsonl",
  "supported_cases": $supported_cases,
  "transcode_cases": $transcode_cases,
  "geometry_cases": $geometry_cases,
  "diagnostic_cases": $diagnostic_cases,
  "identify_error_cases": $identify_error_cases,
  "passes": $passes,
  "failures": $failures
}
EOF

if [[ "$status" != "passed" ]]; then
  echo "error: daily-use corpus failed; see $out_dir" >&2
  exit 1
fi

echo "$out_dir"
