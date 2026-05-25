#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if ! command -v cargo-fuzz >/dev/null 2>&1 && ! cargo fuzz --version >/dev/null 2>&1; then
  echo "error: cargo-fuzz is required; install with: cargo install cargo-fuzz --locked" >&2
  exit 2
fi

if ! rustup toolchain list | grep -q '^nightly'; then
  echo "error: Rust nightly toolchain is required for cargo-fuzz" >&2
  exit 2
fi

corpus_root="$(bash scripts/seed-fuzz-corpus.sh)"
out_dir="${IMX_FUZZ_OUT:-$root/target/fuzz-runs/$(date +%Y%m%d-%H%M%S)}"
max_total_time="${IMX_FUZZ_MAX_TOTAL_TIME:-5}"
rss_limit_mb="${IMX_FUZZ_RSS_LIMIT_MB:-1024}"
nightly_cargo="$(rustup which --toolchain nightly cargo)"
nightly_rustc="$(rustup which --toolchain nightly rustc)"
mkdir -p "$out_dir"

{
  echo "{"
  echo "  \"schema_version\": 1,"
  echo "  \"started_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  echo "  \"max_total_time_seconds\": $max_total_time,"
  echo "  \"rss_limit_mb\": $rss_limit_mb,"
  echo "  \"corpus_root\": \"${corpus_root}\","
  echo "  \"targets\": ["
} >"$out_dir/summary.json"

first=1
for target in farbfeld_decode qoi_decode pnm_decode; do
  log="$out_dir/$target.log"
  if CARGO="$nightly_cargo" RUSTC="$nightly_rustc" cargo fuzz run "$target" "$corpus_root/$target" \
    -- -max_total_time="$max_total_time" -rss_limit_mb="$rss_limit_mb" >"$log" 2>&1; then
    status="passed"
  else
    status="failed"
  fi

  if [[ "$first" == "0" ]]; then
    echo "    ," >>"$out_dir/summary.json"
  fi
  first=0
  corpus_count="$(find "$corpus_root/$target" -type f | wc -l | tr -d ' ')"
  cat >>"$out_dir/summary.json" <<EOF
    {
      "target": "$target",
      "status": "$status",
      "seed_count": $corpus_count,
      "log": "$target.log"
    }
EOF
  if [[ "$status" != "passed" ]]; then
    cat "$log" >&2
    echo "  ]" >>"$out_dir/summary.json"
    echo "}" >>"$out_dir/summary.json"
    exit 1
  fi
done

{
  echo "  ],"
  echo "  \"completed_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\""
  echo "}"
} >>"$out_dir/summary.json"

echo "$out_dir"
