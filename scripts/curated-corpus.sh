#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

out_dir="${IMX_CURATED_CORPUS_OUT:-$root/target/curated-corpus}"
fixture_dir="$out_dir/generated-fixtures"
rm -rf "$out_dir"
mkdir -p "$fixture_dir"

cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir" >/dev/null
cargo test --test curated_corpus -- --nocapture

git_rev="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

cat >"$out_dir/summary.json" <<EOF
{
  "schema_version": 1,
  "git_rev": "$git_rev",
  "generated_at": "$generated_at",
  "fixtures": "generated-fixtures/manifest.json",
  "coverage": [
    "FARBFELD RGBA16",
    "JPEG progressive grayscale",
    "QOI RGB linear",
    "PBM ASCII comments",
    "PGM ASCII scaled and binary 16-bit",
    "PNG grayscale-alpha and RGBA16",
    "PPM ASCII high maxval",
    "adversarial malformed diagnostics",
    "resource boundary checks"
  ],
  "status": "passed"
}
EOF

echo "$out_dir"
