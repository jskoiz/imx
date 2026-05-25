#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: check-bench-thresholds.sh <bench-dir>" >&2
  exit 2
fi

bench_dir="$1"
summary="$bench_dir/summary.json"
output="$bench_dir/threshold-summary.json"

if [[ ! -f "$summary" ]]; then
  echo "error: missing benchmark summary: $summary" >&2
  exit 1
fi

max_library_rss="${IMX_BENCH_MAX_LIBRARY_RSS_BYTES:-536870912}"
max_process_rss="${IMX_BENCH_MAX_PROCESS_RSS_BYTES:-1073741824}"
min_mib_s="${IMX_BENCH_MIN_THROUGHPUT_MIB_S:-0.01}"

library_rss="$(sed -n 's/.*"max_rss_bytes": \([0-9][0-9]*\).*/\1/p' "$summary" | head -n 1)"
library_rss="${library_rss:-0}"

process_max_rss="$(sed -n 's/.*"max_rss_bytes": \([0-9][0-9]*\).*/\1/p' "$summary" | sort -n | tail -n 1)"
process_max_rss="${process_max_rss:-0}"

min_observed_throughput="$(
  sed -n 's/.*"[^"]*_mib_s": \([0-9][0-9.]*\).*/\1/p' "$summary" |
    awk 'NR == 1 || $1 < min { min = $1 } END { if (NR == 0) print "0"; else print min }'
)"

status="passed"
reason=""
if (( library_rss > max_library_rss )); then
  status="failed"
  reason="library RSS $library_rss exceeds $max_library_rss"
elif (( process_max_rss > max_process_rss )); then
  status="failed"
  reason="process RSS $process_max_rss exceeds $max_process_rss"
elif ! awk -v actual="$min_observed_throughput" -v minimum="$min_mib_s" 'BEGIN { exit !(actual >= minimum) }'; then
  status="failed"
  reason="minimum throughput $min_observed_throughput MiB/s is below $min_mib_s MiB/s"
fi

cat >"$output" <<EOF
{
  "schema_version": 1,
  "status": "$status",
  "max_library_rss_bytes": $max_library_rss,
  "observed_library_rss_bytes": $library_rss,
  "max_process_rss_bytes": $max_process_rss,
  "observed_max_process_rss_bytes": $process_max_rss,
  "min_throughput_mib_s": $min_mib_s,
  "observed_min_throughput_mib_s": $min_observed_throughput,
  "reason": "$reason"
}
EOF

if [[ "$status" != "passed" ]]; then
  echo "error: benchmark threshold check failed: $reason" >&2
  exit 1
fi

echo "$output"
