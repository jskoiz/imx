#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

repo="${IMX_REPO:-jskoiz/imx}"
version="${IMX_VERSION:-}"
if [[ -z "$version" ]]; then
  version="v$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"imx-preview","version":"\([^"]*\)".*/\1/p')"
fi
if [[ "$version" != v* ]]; then
  version="v$version"
fi

targets=(
  "x86_64-unknown-linux-gnu"
  "aarch64-apple-darwin"
  "x86_64-apple-darwin"
)

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Darwin:arm64|Darwin:aarch64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    *)
      echo "error: unsupported platform: $os $arch" >&2
      exit 2
      ;;
  esac
}

target="${IMX_RELEASE_TARGET:-$(detect_target)}"
archive="imx-preview-${version#v}-$target.tar.gz"
work_dir="${IMX_RELEASE_SMOKE_DIR:-$root/target/release-archive-smoke/$target}"
download_dir="$work_dir/downloads"
extract_dir="$work_dir/extract"
summary="$work_dir/summary.json"
base_url="https://github.com/$repo/releases/download/$version"

rm -rf "$work_dir"
mkdir -p "$download_dir" "$extract_dir"

download() {
  local url="$1"
  local output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl --retry 8 --retry-delay 3 --retry-all-errors -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    local attempt
    for attempt in 1 2 3 4 5 6 7 8; do
      if wget -qO "$output" "$url"; then
        return 0
      fi
      sleep 3
    done
    return 1
  else
    echo "error: curl or wget is required" >&2
    exit 2
  fi
}

if [[ -n "${IMX_RELEASE_DIR:-}" ]]; then
  cp "$IMX_RELEASE_DIR/SHA256SUMS" "$download_dir/SHA256SUMS"
  if [[ ! -f "$IMX_RELEASE_DIR/$archive" ]]; then
    echo "error: selected archive is missing from IMX_RELEASE_DIR: $archive" >&2
    exit 1
  fi
  cp "$IMX_RELEASE_DIR"/*.tar.gz "$download_dir/"
else
  download "$base_url/SHA256SUMS" "$download_dir/SHA256SUMS"
  for release_target in "${targets[@]}"; do
    download \
      "$base_url/imx-preview-${version#v}-$release_target.tar.gz" \
      "$download_dir/imx-preview-${version#v}-$release_target.tar.gz"
  done
fi

(
  cd "$download_dir"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c SHA256SUMS
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c SHA256SUMS
  else
    echo "error: shasum or sha256sum is required" >&2
    exit 2
  fi
  tar -xzf "$archive" -C "$extract_dir"
)

binary="$extract_dir/imx-preview-${version#v}-$target/imx"
"$binary" --version

if [[ "$(uname -s)" == "Darwin" ]]; then
  otool -L "$binary" >"$work_dir/linkage.txt"
  ! grep -E 'Magick(Core|Wand)|ImageMagick' "$work_dir/linkage.txt"
else
  ldd "$binary" >"$work_dir/linkage.txt"
  ! grep -E 'Magick(Core|Wand)|ImageMagick' "$work_dir/linkage.txt"
fi

smoke_dir="$work_dir/smoke"
mkdir -p "$smoke_dir"
printf 'P3\n2 2\n255\n255 0 0 0 255 0 0 0 255 255 255 255\n' >"$smoke_dir/input.ppm"
printf 'P2\n2 2\n255\n0 85 170 255\n' >"$smoke_dir/input.pgm"
printf 'P1\n2 2\n0 1\n1 0\n' >"$smoke_dir/input.pbm"

"$binary" identify "$smoke_dir/input.ppm" >"$smoke_dir/identify-ppm.txt"
"$binary" identify "$smoke_dir/input.pgm" >"$smoke_dir/identify-pgm.txt"
"$binary" identify "$smoke_dir/input.pbm" >"$smoke_dir/identify-pbm.txt"
"$binary" "$smoke_dir/input.ppm" "$smoke_dir/output.qoi"
"$binary" identify "$smoke_dir/output.qoi" >"$smoke_dir/identify-qoi.txt"
"$binary" "$smoke_dir/output.qoi" "$smoke_dir/output.ff"
"$binary" identify "$smoke_dir/output.ff" >"$smoke_dir/identify-farbfeld.txt"
"$binary" "$smoke_dir/output.ff" "$smoke_dir/output.pbm"
"$binary" "$smoke_dir/output.ff" "$smoke_dir/output.pgm"
"$binary" "$smoke_dir/output.ff" "$smoke_dir/output.ppm"
"$binary" identify "$smoke_dir/output.pbm" >"$smoke_dir/identify-output-pbm.txt"
"$binary" identify "$smoke_dir/output.pgm" >"$smoke_dir/identify-output-pgm.txt"
"$binary" identify "$smoke_dir/output.ppm" >"$smoke_dir/identify-output-ppm.txt"

cat >"$summary" <<EOF
{
  "schema_version": 1,
  "repo": "$repo",
  "version": "$version",
  "target": "$target",
  "archive": "$archive",
  "checksum_file": "downloads/SHA256SUMS",
  "linkage": "linkage.txt",
  "smoke_dir": "smoke",
  "status": "passed"
}
EOF

echo "$work_dir"
