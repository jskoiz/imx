#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo tree --workspace --edges normal
cargo test --workspace

cargo build -p imx-cli --bin imx
fixture_dir="${IMX_FIXTURE_DIR:-$root/target/generated-fixtures}"
rm -rf "$fixture_dir"
cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir"

cargo test --test fuzz_smoke -- --nocapture
IMX_BENCH_ITERATIONS="${IMX_BENCH_ITERATIONS:-5}" cargo bench --bench throughput

if [[ "${IMX_REQUIRE_ORACLE:-0}" == "1" ]]; then
  : "${IMAGEMAGICK_MAGICK:?set IMAGEMAGICK_MAGICK to the ImageMagick oracle binary}"
  IMX_REQUIRE_ORACLE=1 \
    IMX_STANDALONE_BIN="$root/target/debug/imx" \
    cargo test --test differential -- --nocapture
fi
