#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_url="${IMX_INSTALL_REPO_URL:-$root}"
revision="${IMX_INSTALL_REVISION:-HEAD}"
work_dir="${IMX_INSTALL_WORK_DIR:-$root/target/install-verify}"
install_root="$work_dir/install-root"
checkout="$work_dir/checkout"

rm -rf "$work_dir"
mkdir -p "$work_dir"

git clone "$repo_url" "$checkout" >/dev/null
(
  cd "$checkout"
  git checkout "$revision" >/dev/null
  cargo install --path crates/cli --bin imx --locked --root "$install_root" >/dev/null
  "$install_root/bin/imx" --version

  fixture_dir="$work_dir/fixtures"
  cargo run -p imx-cli --bin imx-generate-fixtures -- "$fixture_dir" >/dev/null
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.ff"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.qoi"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.pbm"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.ppm"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.pgm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.qoi"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.pbm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.pgm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pbm" "$work_dir/pbm-gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ppm" "$work_dir/gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pgm" "$work_dir/pgm-gradient.ff"
)

cat >"$work_dir/install-summary.json" <<EOF
{
  "schema_version": 1,
  "repo_url": "$repo_url",
  "revision": "$revision",
  "installed_binary": "$install_root/bin/imx",
  "status": "passed"
}
EOF

echo "$work_dir"
