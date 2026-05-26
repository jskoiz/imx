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
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.jpg"
  "$install_root/bin/imx" identify "$fixture_dir/gray-4x1.jpg"
  "$install_root/bin/imx" identify "$fixture_dir/progressive-rgb-4x3.jpg" | grep -Fx 'format=JPEG width=4 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" identify "$fixture_dir/progressive-gray-4x2.jpg" | grep -Fx 'format=JPEG width=4 height=2 channels=GRAY depth=8'
  "$install_root/bin/imx" identify "$fixture_dir/photo-orientation-o6.jpg" | grep -Fx 'format=JPEG width=2 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" identify "$fixture_dir/progressive-orientation-o6.jpg" | grep -Fx 'format=JPEG width=3 height=4 channels=RGB depth=8'
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.qoi"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.pbm"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.png"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.ppm"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64-png16.png"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64-ppm16.ppm"
  "$install_root/bin/imx" identify "$fixture_dir/gradient-64.pgm"
  "$install_root/bin/imx" identify "$fixture_dir/intake-farbfeld-rgba16-2x2.ff" | grep -Fx 'format=FARBFELD width=2 height=2 channels=RGBA depth=16'
  "$install_root/bin/imx" identify "$fixture_dir/intake-qoi-rgb-linear-2x2.qoi" | grep -Fx 'format=QOI width=2 height=2 channels=RGB depth=8'
  "$install_root/bin/imx" identify "$fixture_dir/intake-comments-2x1.ppm" | grep -Fx 'format=PPM width=2 height=1 channels=RGB depth=16'
  "$install_root/bin/imx" identify "$fixture_dir/intake-pgm16-2x1.pgm" | grep -Fx 'format=PGM width=2 height=1 channels=GRAY depth=16'
  "$install_root/bin/imx" identify "$fixture_dir/intake-rgba16-1x1.png" | grep -Fx 'format=PNG width=1 height=1 channels=RGBA depth=16'
  "$install_root/bin/imx" identify "FARBFELD:$fixture_dir/gradient-64.ff"
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/gradient-64.jpg"
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/gray-4x1.jpg"
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/progressive-rgb-4x3.jpg" | grep -Fx 'format=JPEG width=4 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/progressive-gray-4x2.jpg" | grep -Fx 'format=JPEG width=4 height=2 channels=GRAY depth=8'
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/photo-orientation-o6.jpg" | grep -Fx 'format=JPEG width=2 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" identify "JPEG:$fixture_dir/progressive-orientation-o6.jpg" | grep -Fx 'format=JPEG width=3 height=4 channels=RGB depth=8'
  "$install_root/bin/imx" identify "QOI:$fixture_dir/gradient-64.qoi"
  "$install_root/bin/imx" identify "PBM:$fixture_dir/gradient-64.pbm"
  "$install_root/bin/imx" identify "PNG:$fixture_dir/gradient-64.png"
  "$install_root/bin/imx" identify "PPM:$fixture_dir/gradient-64.ppm"
  "$install_root/bin/imx" identify "PNG:$fixture_dir/gradient-64-png16.png"
  "$install_root/bin/imx" identify "PPM:$fixture_dir/gradient-64-ppm16.ppm"
  "$install_root/bin/imx" identify "PGM:$fixture_dir/gradient-64.pgm"
  "$install_root/bin/imx" identify "FARBFELD:$fixture_dir/intake-farbfeld-rgba16-2x2.ff" | grep -Fx 'format=FARBFELD width=2 height=2 channels=RGBA depth=16'
  "$install_root/bin/imx" identify "QOI:$fixture_dir/intake-qoi-rgb-linear-2x2.qoi" | grep -Fx 'format=QOI width=2 height=2 channels=RGB depth=8'
  "$install_root/bin/imx" identify "PPM:$fixture_dir/intake-comments-2x1.ppm" | grep -Fx 'format=PPM width=2 height=1 channels=RGB depth=16'
  "$install_root/bin/imx" identify "PGM:$fixture_dir/intake-pgm16-2x1.pgm" | grep -Fx 'format=PGM width=2 height=1 channels=GRAY depth=16'
  "$install_root/bin/imx" identify "PNG:$fixture_dir/intake-rgba16-1x1.png" | grep -Fx 'format=PNG width=1 height=1 channels=RGBA depth=16'
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.qoi"
  "$install_root/bin/imx" "FARBFELD:$fixture_dir/gradient-64.ff" "QOI:$work_dir/prefix-gradient.qoi"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.jpg" "$work_dir/jpeg-gradient.ff"
  "$install_root/bin/imx" "JPEG:$fixture_dir/progressive-rgb-4x3.jpg" "PPM:$work_dir/progressive-rgb.ppm"
  "$install_root/bin/imx" identify "PPM:$work_dir/progressive-rgb.ppm" | grep -Fx 'format=PPM width=4 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" "JPEG:$fixture_dir/photo-orientation-o6.jpg" "PPM:$work_dir/oriented-o6.ppm"
  "$install_root/bin/imx" identify "PPM:$work_dir/oriented-o6.ppm" | grep -Fx 'format=PPM width=2 height=3 channels=RGB depth=8'
  "$install_root/bin/imx" "JPEG:$fixture_dir/progressive-orientation-o6.jpg" "PPM:$work_dir/progressive-oriented-o6.ppm"
  "$install_root/bin/imx" identify "PPM:$work_dir/progressive-oriented-o6.ppm" | grep -Fx 'format=PPM width=3 height=4 channels=RGB depth=8'
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ppm" "$work_dir/gradient.jpg"
  "$install_root/bin/imx" "JPEG:$fixture_dir/gradient-64.jpg" "FARBFELD:$work_dir/prefix-jpeg-gradient.ff"
  "$install_root/bin/imx" "PPM:$fixture_dir/gradient-64.ppm" "JPEG:$work_dir/prefix-gradient.jpg"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.pbm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.pgm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/gradient.png"
  "$install_root/bin/imx" "FARBFELD:$fixture_dir/gradient-64.ff" "PGM:$work_dir/prefix-gradient.pgm"
  "$install_root/bin/imx" "FARBFELD:$fixture_dir/gradient-64.ff" "PNG:$work_dir/prefix-gradient.png"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pbm" "$work_dir/pbm-gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.png" "$work_dir/png-gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ppm" "$work_dir/gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64-png16.png" "$work_dir/png16-gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64-ppm16.ppm" "$work_dir/ppm16-gradient.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pgm" "$work_dir/pgm-gradient.ff"
  "$install_root/bin/imx" "PNG:$fixture_dir/intake-rgba16-1x1.png" "FARBFELD:$work_dir/intake-png16.ff"
  "$install_root/bin/imx" "PPM:$fixture_dir/intake-comments-2x1.ppm" "PGM:$work_dir/intake-ppm.pgm"
  "$install_root/bin/imx" "QOI:$fixture_dir/intake-qoi-rgb-linear-2x2.qoi" "PNG:$work_dir/intake-qoi.png"
  "$install_root/bin/imx" "PPM:$fixture_dir/gradient-64.ppm" "FARBFELD:$work_dir/prefix-gradient.ff"
  "$install_root/bin/imx" "PNG:$fixture_dir/gradient-64.png" "FARBFELD:$work_dir/prefix-png-gradient.ff"
  "$install_root/bin/imx" "PPM:$fixture_dir/gradient-64-ppm16.ppm" "PPM:$work_dir/prefix-ppm16-rewrite.ppm"
  "$install_root/bin/imx" "PNG:$fixture_dir/gradient-64-png16.png" "PNG:$work_dir/prefix-png16-rewrite.png"
  "$install_root/bin/imx" "FARBFELD:$work_dir/prefix-gradient.ff" "PBM:$work_dir/prefix-gradient.pbm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ff" "$work_dir/rewrite.ff"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.jpg" "$work_dir/rewrite.jpg"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.qoi" "$work_dir/rewrite.qoi"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pbm" "$work_dir/rewrite.pbm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.pgm" "$work_dir/rewrite.pgm"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.png" "$work_dir/rewrite.png"
  "$install_root/bin/imx" "$fixture_dir/gradient-64.ppm" "$work_dir/rewrite.ppm"
  "$install_root/bin/imx" "QOI:$fixture_dir/gradient-64.qoi" "QOI:$work_dir/prefix-rewrite.qoi"
  "$install_root/bin/imx" "JPEG:$fixture_dir/gradient-64.jpg" "JPEG:$work_dir/prefix-rewrite.jpg"
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
