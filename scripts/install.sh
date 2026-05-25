#!/usr/bin/env sh
set -eu

repo="${IMX_REPO:-jskoiz/imx}"
version="${IMX_VERSION:-v0.3.0}"
install_dir="${IMX_INSTALL_DIR:-$HOME/.local/bin}"

os="$(uname -s)"
arch="$(uname -m)"

case "$os:$arch" in
  Linux:x86_64)
    target="x86_64-unknown-linux-gnu"
    ;;
  Darwin:arm64|Darwin:aarch64)
    target="aarch64-apple-darwin"
    ;;
  Darwin:x86_64)
    target="x86_64-apple-darwin"
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

download "$base_url/$archive" "$work_dir/$archive"
download "$base_url/SHA256SUMS" "$work_dir/SHA256SUMS"

(
  cd "$work_dir"
  grep " $archive\$" SHA256SUMS > SHA256SUMS.selected
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

"$install_dir/imx" --version
echo "installed $install_dir/imx"
