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

out_dir="${IMX_FUZZ_OUT:-$root/target/fuzz-runs/$(date +%Y%m%d-%H%M%S)}"
max_total_time="${IMX_FUZZ_MAX_TOTAL_TIME:-5}"
rss_limit_mb="${IMX_FUZZ_RSS_LIMIT_MB:-1024}"
nightly_cargo="$(rustup which --toolchain nightly cargo)"
nightly_rustc="$(rustup which --toolchain nightly rustc)"
mkdir -p "$out_dir"

if [[ -z "${IMX_FUZZ_CORPUS:-}" ]]; then
  export IMX_FUZZ_CORPUS="$out_dir/corpus"
fi
corpus_root="$(bash scripts/seed-fuzz-corpus.sh)"

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

copy_artifacts() {
  if [[ -d "$root/fuzz/artifacts" ]]; then
    mkdir -p "$out_dir/artifacts"
    cp -R "$root/fuzz/artifacts/." "$out_dir/artifacts/"
  fi
}

{
  echo "{"
  echo "  \"schema_version\": 1,"
  echo "  \"started_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  echo "  \"git_rev\": \"$(json_escape "$(git rev-parse HEAD 2>/dev/null || echo unknown)")\","
  echo "  \"runner_os\": \"$(json_escape "$(uname -s)")\","
  echo "  \"runner_arch\": \"$(json_escape "$(uname -m)")\","
  echo "  \"rustc\": \"$(json_escape "$("$nightly_rustc" --version)")\","
  echo "  \"cargo_fuzz\": \"$(json_escape "$(cargo fuzz --version 2>/dev/null || echo cargo-fuzz)")\","
  echo "  \"max_total_time_seconds\": $max_total_time,"
  echo "  \"rss_limit_mb\": $rss_limit_mb,"
  echo "  \"corpus_root\": \"${corpus_root}\","
  echo "  \"artifact_root\": \"artifacts\","
  echo "  \"targets\": ["
} >"$out_dir/summary.json"

first=1
for target in farbfeld_decode qoi_decode pnm_decode; do
  log="$out_dir/$target.log"
  artifact_dir="$out_dir/artifacts/$target"
  mkdir -p "$artifact_dir"
  initial_corpus_count="$(find "$corpus_root/$target" -type f | wc -l | tr -d ' ')"
  command="cargo fuzz run $target $corpus_root/$target -- -max_total_time=$max_total_time -rss_limit_mb=$rss_limit_mb -artifact_prefix=$artifact_dir/"
  if CARGO="$nightly_cargo" RUSTC="$nightly_rustc" cargo fuzz run "$target" "$corpus_root/$target" \
    -- -max_total_time="$max_total_time" -rss_limit_mb="$rss_limit_mb" -artifact_prefix="$artifact_dir/" >"$log" 2>&1; then
    status="passed"
    exit_code=0
  else
    status="failed"
    exit_code=$?
  fi

  if [[ "$first" == "0" ]]; then
    echo "    ," >>"$out_dir/summary.json"
  fi
  first=0
  corpus_count="$(find "$corpus_root/$target" -type f | wc -l | tr -d ' ')"
  artifact_count="$(find "$artifact_dir" -type f | wc -l | tr -d ' ')"
  cat >>"$out_dir/summary.json" <<EOF
    {
      "target": "$target",
      "status": "$status",
      "exit_code": $exit_code,
      "initial_seed_count": $initial_corpus_count,
      "final_corpus_count": $corpus_count,
      "artifact_count": $artifact_count,
      "command": "$(json_escape "$command")",
      "log": "$target.log",
      "artifact_dir": "artifacts/$target"
    }
EOF
  if [[ "$status" != "passed" ]]; then
    copy_artifacts
    cat "$log" >&2
    echo "  ]" >>"$out_dir/summary.json"
    echo "  ," >>"$out_dir/summary.json"
    echo "  \"completed_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"" >>"$out_dir/summary.json"
    echo "}" >>"$out_dir/summary.json"
    exit 1
  fi
done

copy_artifacts

{
  echo "  ],"
  echo "  \"completed_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\""
  echo "}"
} >>"$out_dir/summary.json"

echo "$out_dir"
