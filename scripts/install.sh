#!/usr/bin/env sh
set -eu

repo="${IMX_REPO:-jskoiz/imx}"
version="${IMX_VERSION:-v0.5.0}"
install_dir="${IMX_INSTALL_DIR:-$HOME/.local/bin}"
run_smoke="${IMX_INSTALL_SMOKE:-1}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Linux:x86_64)
    target="x86_64-unknown-linux-gnu"
    ;;
  Linux:aarch64|Linux:arm64)
    target="aarch64-unknown-linux-gnu"
    ;;
  Darwin:*)
    echo "error: this release installer supports Linux archives only; use the v0.4.0 installer for published macOS archives or install from source" >&2
    exit 2
    ;;
  *)
    echo "error: unsupported platform: $os $arch" >&2
    exit 2
    ;;
esac

archive="imx-preview-${version#v}-$target.tar.gz"
base_url="https://github.com/$repo/releases/download/$version"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/imx-install.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT INT TERM

download() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$output" "$url"
  else
    echo "error: curl or wget is required" >&2
    exit 2
  fi
}

download "$base_url/SHA256SUMS" "$work_dir/SHA256SUMS"

(
  cd "$work_dir"
  if ! grep " $archive\$" SHA256SUMS > SHA256SUMS.selected; then
    echo "error: SHA256SUMS does not contain selected archive: $archive" >&2
    exit 1
  fi
  download "$base_url/$archive" "$archive"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c SHA256SUMS.selected
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c SHA256SUMS.selected
  else
    echo "error: shasum or sha256sum is required" >&2
    exit 2
  fi
  tar -xzf "$archive"
)

mkdir -p "$install_dir"
binary="$work_dir/imx-preview-${version#v}-$target/imx"
if command -v install >/dev/null 2>&1; then
  install -m 755 "$binary" "$install_dir/imx"
else
  cp "$binary" "$install_dir/imx"
  chmod 755 "$install_dir/imx"
fi

installed_version="$("$install_dir/imx" --version)"
if [ "$installed_version" != "imx ${version#v}" ]; then
  echo "error: installed binary version mismatch: expected imx ${version#v}, got $installed_version" >&2
  exit 1
fi
echo "$installed_version"

if [ "$run_smoke" != "0" ] && [ "$run_smoke" != "false" ]; then
  smoke_dir="$work_dir/smoke"
  mkdir -p "$smoke_dir"
  printf 'P3\n2 2\n255\n255 0 0 0 255 0 0 0 255 255 255 255\n' >"$smoke_dir/input.ppm"
  printf 'P2\n2 2\n255\n0 85 170 255\n' >"$smoke_dir/input.pgm"
  printf 'P1\n2 2\n0 1\n1 0\n' >"$smoke_dir/input.pbm"
  "$install_dir/imx" identify "$smoke_dir/input.ppm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/input.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/input.pbm" >/dev/null
  "$install_dir/imx" "$smoke_dir/input.ppm" "$smoke_dir/output.qoi"
  "$install_dir/imx" identify "$smoke_dir/output.qoi" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.qoi" "$smoke_dir/output.ff"
  "$install_dir/imx" identify "$smoke_dir/output.ff" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.pbm"
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.pgm"
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.ppm"
  "$install_dir/imx" identify "$smoke_dir/output.pbm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.ppm" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/rewrite.ff"
  "$install_dir/imx" "$smoke_dir/output.qoi" "$smoke_dir/rewrite.qoi"
  "$install_dir/imx" "$smoke_dir/input.pbm" "$smoke_dir/rewrite.pbm"
  "$install_dir/imx" "$smoke_dir/input.pgm" "$smoke_dir/rewrite.pgm"
  "$install_dir/imx" "$smoke_dir/input.ppm" "$smoke_dir/rewrite.ppm"
  "$install_dir/imx" identify "$smoke_dir/rewrite.ff" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.qoi" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.pbm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.ppm" >/dev/null
  echo "smoke passed"
fi

echo "installed $install_dir/imx"
