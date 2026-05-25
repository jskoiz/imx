#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

out_dir="${IMX_SAME_FORMAT_DETERMINISM_OUT:-$root/target/same-format-determinism}"
standalone="${IMX_STANDALONE_BIN:-$root/target/debug/imx}"

cargo build -p imx-cli --bin imx >/dev/null
rm -rf "$out_dir"
mkdir -p "$out_dir"

fixture_dir="$out_dir/fixtures"
cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir" >/dev/null

for ext in ff qoi pbm pgm ppm; do
  "$standalone" "$fixture_dir/gradient-64.$ext" "$out_dir/a.$ext"
  "$standalone" "$fixture_dir/gradient-64.$ext" "$out_dir/b.$ext"
  cmp "$out_dir/a.$ext" "$out_dir/b.$ext"
done

cat >"$out_dir/summary.json" <<EOF
{
  "schema_version": 1,
  "formats": ["ff", "qoi", "pbm", "pgm", "ppm"],
  "status": "passed"
}
EOF

echo "$out_dir"
