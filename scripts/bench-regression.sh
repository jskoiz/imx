#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

base_ref="${IMX_BENCH_BASE_REF:-v0.5.0}"
stamp="$(date +%Y%m%d-%H%M%S)"
out_dir="${IMX_BENCH_REGRESSION_OUT:-$root/target/bench-regression-$stamp}"
baseline_checkout="$out_dir/baseline-checkout"
baseline_out="$out_dir/baseline"
current_out="$out_dir/current"
report="$out_dir/regression-report.json"
failures="$out_dir/failures.txt"
warnings="$out_dir/warnings.txt"

throughput_fail_ratio="${IMX_BENCH_THROUGHPUT_FAIL_RATIO:-0}"
throughput_warn_ratio="${IMX_BENCH_THROUGHPUT_WARN_RATIO:-0.90}"
rss_fail_ratio="${IMX_BENCH_RSS_FAIL_RATIO:-1.25}"
rss_warn_ratio="${IMX_BENCH_RSS_WARN_RATIO:-1.10}"
rss_fail_slack="${IMX_BENCH_RSS_FAIL_SLACK_BYTES:-16777216}"
rss_warn_slack="${IMX_BENCH_RSS_WARN_SLACK_BYTES:-8388608}"

rm -rf "$out_dir"
mkdir -p "$out_dir"
: >"$failures"
: >"$warnings"

cleanup() {
  git worktree remove --force "$baseline_checkout" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git worktree add --detach "$baseline_checkout" "$base_ref" >/dev/null

(
  cd "$baseline_checkout"
  IMX_BENCH_OUT="$baseline_out" bash scripts/bench-release.sh >/dev/null
)

IMX_BENCH_OUT="$current_out" bash scripts/bench-release.sh >/dev/null

extract_library_metric() {
  local summary="$1"
  local metric="$2"
  sed -n "s/.*\"$metric\": \([0-9][0-9.]*\).*/\1/p" "$summary" | head -n 1
}

extract_process_rss() {
  local summary="$1"
  sed -n 's/.*"case_id": "\([^"]*\)".*"max_rss_bytes": \([0-9][0-9]*\).*/\1 \2/p' "$summary"
}

current_summary="$current_out/summary.json"
baseline_summary="$baseline_out/summary.json"

sed -n 's/.*"\([^"]*_mib_s\)": \([0-9][0-9.]*\).*/\1 \2/p' "$current_summary" |
while read -r metric current_value; do
  baseline_value="$(extract_library_metric "$baseline_summary" "$metric")"
  if [[ -z "$baseline_value" ]]; then
    echo "new throughput metric $metric has no baseline in $base_ref" >>"$warnings"
    continue
  fi
  awk -v metric="$metric" -v base="$baseline_value" -v current="$current_value" \
    -v fail_ratio="$throughput_fail_ratio" -v warn_ratio="$throughput_warn_ratio" \
    -v failures="$failures" -v warnings="$warnings" '
      BEGIN {
        if (base <= 0) {
          printf "%s baseline throughput %.6f is not usable\n", metric, base >> failures
        } else if (fail_ratio > 0 && current < base * fail_ratio) {
          printf "%s throughput %.6f below %.2fx baseline %.6f\n", metric, current, fail_ratio, base >> failures
        } else if (current < base * warn_ratio) {
          printf "%s throughput %.6f below %.2fx baseline %.6f\n", metric, current, warn_ratio, base >> warnings
        }
      }'
done

baseline_library_rss="$(extract_library_metric "$baseline_summary" max_rss_bytes)"
current_library_rss="$(extract_library_metric "$current_summary" max_rss_bytes)"
if [[ -n "$baseline_library_rss" && -n "$current_library_rss" ]]; then
  awk -v label="library max_rss_bytes" -v base="$baseline_library_rss" -v current="$current_library_rss" \
    -v fail_ratio="$rss_fail_ratio" -v warn_ratio="$rss_warn_ratio" \
    -v fail_slack="$rss_fail_slack" -v warn_slack="$rss_warn_slack" \
    -v failures="$failures" -v warnings="$warnings" '
      function max(a, b) { return a > b ? a : b }
      BEGIN {
        fail_limit = max(base * fail_ratio, base + fail_slack)
        warn_limit = max(base * warn_ratio, base + warn_slack)
        if (current > fail_limit) {
          printf "%s RSS %.0f exceeds fail limit %.0f from baseline %.0f\n", label, current, fail_limit, base >> failures
        } else if (current > warn_limit) {
          printf "%s RSS %.0f exceeds warn limit %.0f from baseline %.0f\n", label, current, warn_limit, base >> warnings
        }
      }'
else
  echo "missing library max_rss_bytes metric" >>"$failures"
fi

extract_process_rss "$current_summary" |
while read -r case_id current_rss; do
  case "$case_id" in
    standalone-*) ;;
    *) continue ;;
  esac
  baseline_rss="$(extract_process_rss "$baseline_summary" | awk -v case_id="$case_id" '$1 == case_id { print $2 }')"
  if [[ -z "$baseline_rss" ]]; then
    echo "new process RSS case $case_id has no baseline in $base_ref" >>"$warnings"
    continue
  fi
  awk -v label="$case_id" -v base="$baseline_rss" -v current="$current_rss" \
    -v fail_ratio="$rss_fail_ratio" -v warn_ratio="$rss_warn_ratio" \
    -v fail_slack="$rss_fail_slack" -v warn_slack="$rss_warn_slack" \
    -v failures="$failures" -v warnings="$warnings" '
      function max(a, b) { return a > b ? a : b }
      BEGIN {
        fail_limit = max(base * fail_ratio, base + fail_slack)
        warn_limit = max(base * warn_ratio, base + warn_slack)
        if (current > fail_limit) {
          printf "%s RSS %.0f exceeds fail limit %.0f from baseline %.0f\n", label, current, fail_limit, base >> failures
        } else if (current > warn_limit) {
          printf "%s RSS %.0f exceeds warn limit %.0f from baseline %.0f\n", label, current, warn_limit, base >> warnings
        }
      }'
done

failure_count="$(wc -l <"$failures" | tr -d ' ')"
warning_count="$(wc -l <"$warnings" | tr -d ' ')"
status="passed"
if [[ "$failure_count" != "0" ]]; then
  status="failed"
fi

cat >"$report" <<EOF
{
  "schema_version": 1,
  "status": "$status",
  "baseline_ref": "$base_ref",
  "baseline_summary": "baseline/summary.json",
  "current_summary": "current/summary.json",
  "throughput_fail_ratio": $throughput_fail_ratio,
  "throughput_warn_ratio": $throughput_warn_ratio,
  "rss_fail_ratio": $rss_fail_ratio,
  "rss_warn_ratio": $rss_warn_ratio,
  "rss_fail_slack_bytes": $rss_fail_slack,
  "rss_warn_slack_bytes": $rss_warn_slack,
  "failure_count": $failure_count,
  "warning_count": $warning_count,
  "failures": "failures.txt",
  "warnings": "warnings.txt"
}
EOF

if [[ "$status" != "passed" ]]; then
  cat "$failures" >&2
  exit 1
fi

echo "$out_dir"
