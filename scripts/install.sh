#!/usr/bin/env sh
set -eu

repo="${IMX_REPO:-jskoiz/imx}"
version="${IMX_VERSION:-v0.16.0}"
install_dir="${IMX_INSTALL_DIR:-$HOME/.local/bin}"
run_smoke="${IMX_INSTALL_SMOKE:-1}"
min_glibc="2.34"

os="$(uname -s)"
arch="$(uname -m)"

detect_glibc_version() {
  if command -v getconf >/dev/null 2>&1; then
    getconf GNU_LIBC_VERSION 2>/dev/null | awk '/glibc/ { print $2; exit }'
    return
  fi
  if command -v ldd >/dev/null 2>&1; then
    ldd --version 2>&1 | sed -n '1s/.* \([0-9][0-9.]*\).*/\1/p'
  fi
}

require_glibc_floor() {
  glibc_version="$(detect_glibc_version || true)"
  if [ -z "$glibc_version" ]; then
    echo "error: this release archive requires glibc $min_glibc or newer; install from source on non-glibc Linux" >&2
    exit 2
  fi
  glibc_major="${glibc_version%%.*}"
  glibc_minor="${glibc_version#*.}"
  glibc_minor="${glibc_minor%%.*}"
  case "$glibc_major:$glibc_minor" in
    *[!0-9:]*|:*)
      echo "error: could not parse glibc version '$glibc_version'; this release archive requires glibc $min_glibc or newer" >&2
      exit 2
      ;;
  esac
  if [ "$glibc_major" -lt 2 ] || { [ "$glibc_major" -eq 2 ] && [ "$glibc_minor" -lt 34 ]; }; then
    echo "error: this release archive requires glibc $min_glibc or newer; detected glibc $glibc_version" >&2
    exit 2
  fi
}

case "$os:$arch" in
  Linux:x86_64)
    target="x86_64-unknown-linux-gnu"
    require_glibc_floor
    ;;
  Linux:aarch64|Linux:arm64)
    target="aarch64-unknown-linux-gnu"
    require_glibc_floor
    ;;
  Darwin:*)
    echo "error: this release installer supports Linux glibc archives only; install from source on macOS" >&2
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
  printf 'P6\n2 1\n65535\n\x12\x34\x56\x78\x9a\xbc\x00\x00\x80\x00\xff\xff' >"$smoke_dir/input16.ppm"
  printf 'P2\n2 2\n255\n0 85 170 255\n' >"$smoke_dir/input.pgm"
  printf 'P1\n2 2\n0 1\n1 0\n' >"$smoke_dir/input.pbm"
  printf 'P3\n2 1\n255\n255 0 0 0 0 255\n' >"$smoke_dir/fit-input.ppm"
  printf 'P2\n2 1\n255\n0 255\n' >"$smoke_dir/fit-input.pgm"
  printf 'P1\n2 1\n0 1\n' >"$smoke_dir/fit-input.pbm"
  "$install_dir/imx" identify "$smoke_dir/input.ppm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/input16.ppm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/input.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/input.pbm" >/dev/null
  "$install_dir/imx" identify "PPM:$smoke_dir/input.ppm" >/dev/null
  "$install_dir/imx" identify "PPM:$smoke_dir/input16.ppm" >/dev/null
  "$install_dir/imx" identify "PGM:$smoke_dir/input.pgm" >/dev/null
  "$install_dir/imx" identify "PBM:$smoke_dir/input.pbm" >/dev/null
  "$install_dir/imx" "$smoke_dir/input.ppm" "$smoke_dir/output.qoi"
  "$install_dir/imx" "$smoke_dir/input16.ppm" "$smoke_dir/output16.ff"
  "$install_dir/imx" "$smoke_dir/input.ppm" "$smoke_dir/output.bmp"
  "$install_dir/imx" identify "$smoke_dir/output.qoi" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output16.ff" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.bmp" >/dev/null
  "$install_dir/imx" identify "QOI:$smoke_dir/output.qoi" >/dev/null
  "$install_dir/imx" identify "FARBFELD:$smoke_dir/output16.ff" >/dev/null
  "$install_dir/imx" identify "BMP:$smoke_dir/output.bmp" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.qoi" "$smoke_dir/output.ff"
  "$install_dir/imx" identify "$smoke_dir/output.ff" >/dev/null
  "$install_dir/imx" identify "FARBFELD:$smoke_dir/output.ff" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.pbm"
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.pgm"
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.png"
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/output.ppm"
  "$install_dir/imx" "PPM:$smoke_dir/input.ppm" "QOI:$smoke_dir/prefix-output.qoi"
  "$install_dir/imx" "PPM:$smoke_dir/input16.ppm" "PPM:$smoke_dir/prefix-output16.ppm"
  "$install_dir/imx" "QOI:$smoke_dir/prefix-output.qoi" "FARBFELD:$smoke_dir/prefix-output.ff"
  "$install_dir/imx" "FARBFELD:$smoke_dir/prefix-output.ff" "PBM:$smoke_dir/prefix-output.pbm"
  "$install_dir/imx" "FARBFELD:$smoke_dir/prefix-output.ff" "PGM:$smoke_dir/prefix-output.pgm"
  "$install_dir/imx" "FARBFELD:$smoke_dir/prefix-output.ff" "PNG:$smoke_dir/prefix-output.png"
  "$install_dir/imx" "FARBFELD:$smoke_dir/prefix-output.ff" "PPM:$smoke_dir/prefix-output.ppm"
  "$install_dir/imx" "FARBFELD:$smoke_dir/prefix-output.ff" "BMP:$smoke_dir/prefix-output.bmp"
  "$install_dir/imx" resize 1x1 "PPM:$smoke_dir/input.ppm" "PPM:$smoke_dir/resized.ppm"
  "$install_dir/imx" identify "PPM:$smoke_dir/resized.ppm" | grep -F "format=PPM width=1 height=1" >/dev/null
  "$install_dir/imx" resize 1x1 "BMP:$smoke_dir/output.bmp" "BMP:$smoke_dir/resized.bmp"
  "$install_dir/imx" identify "BMP:$smoke_dir/resized.bmp" | grep -F "format=BMP width=1 height=1" >/dev/null
  "$install_dir/imx" "PPM:$smoke_dir/fit-input.ppm" "FARBFELD:$smoke_dir/fit-source.ff"
  "$install_dir/imx" "PPM:$smoke_dir/fit-input.ppm" "BMP:$smoke_dir/fit-source.bmp"
  "$install_dir/imx" "PPM:$smoke_dir/fit-input.ppm" "JPEG:$smoke_dir/fit-source.jpg"
  "$install_dir/imx" "PPM:$smoke_dir/fit-input.ppm" "QOI:$smoke_dir/fit-source.qoi"
  "$install_dir/imx" "PPM:$smoke_dir/fit-input.ppm" "PNG:$smoke_dir/fit-source.png"
  "$install_dir/imx" resize-fit 5x5 "FARBFELD:$smoke_dir/fit-source.ff" "FARBFELD:$smoke_dir/fit.ff"
  "$install_dir/imx" resize-fit 5x5 "BMP:$smoke_dir/fit-source.bmp" "BMP:$smoke_dir/fit.bmp"
  "$install_dir/imx" resize-fit 5x5 "JPEG:$smoke_dir/fit-source.jpg" "JPEG:$smoke_dir/fit.jpg"
  "$install_dir/imx" resize-fit 5x5 "QOI:$smoke_dir/fit-source.qoi" "QOI:$smoke_dir/fit.qoi"
  "$install_dir/imx" resize-fit 5x5 "PBM:$smoke_dir/fit-input.pbm" "PBM:$smoke_dir/fit.pbm"
  "$install_dir/imx" resize-fit 5x5 "PGM:$smoke_dir/fit-input.pgm" "PGM:$smoke_dir/fit.pgm"
  "$install_dir/imx" resize-fit 5x5 "PNG:$smoke_dir/fit-source.png" "PNG:$smoke_dir/fit.png"
  "$install_dir/imx" resize-fit 5x5 "PPM:$smoke_dir/fit-input.ppm" "PPM:$smoke_dir/fit.ppm"
  "$install_dir/imx" identify "FARBFELD:$smoke_dir/fit.ff" | grep -F "format=FARBFELD width=5 height=3" >/dev/null
  "$install_dir/imx" identify "BMP:$smoke_dir/fit.bmp" | grep -F "format=BMP width=5 height=3" >/dev/null
  "$install_dir/imx" identify "JPEG:$smoke_dir/fit.jpg" | grep -F "format=JPEG width=5 height=3" >/dev/null
  "$install_dir/imx" identify "QOI:$smoke_dir/fit.qoi" | grep -F "format=QOI width=5 height=3" >/dev/null
  "$install_dir/imx" identify "PBM:$smoke_dir/fit.pbm" | grep -F "format=PBM width=5 height=3" >/dev/null
  "$install_dir/imx" identify "PGM:$smoke_dir/fit.pgm" | grep -F "format=PGM width=5 height=3" >/dev/null
  "$install_dir/imx" identify "PNG:$smoke_dir/fit.png" | grep -F "format=PNG width=5 height=3" >/dev/null
  "$install_dir/imx" identify "PPM:$smoke_dir/fit.ppm" | grep -F "format=PPM width=5 height=3" >/dev/null
  cp "$smoke_dir/fit-input.ppm" "$smoke_dir/batch-ppm.ppm"
  cp "$smoke_dir/fit-input.pgm" "$smoke_dir/batch-pgm.pgm"
  mkdir -p "$smoke_dir/batch"
  "$install_dir/imx" batch-convert --to PPM --output-dir "$smoke_dir/batch" --resize-fit 5x5 "PPM:$smoke_dir/batch-ppm.ppm" "PGM:$smoke_dir/batch-pgm.pgm"
  "$install_dir/imx" identify "PPM:$smoke_dir/batch/batch-ppm.ppm" | grep -F "format=PPM width=5 height=3" >/dev/null
  "$install_dir/imx" identify "PPM:$smoke_dir/batch/batch-pgm.ppm" | grep -F "format=PPM width=5 height=3" >/dev/null
  mkdir -p "$smoke_dir/batch-bmp"
  "$install_dir/imx" batch-convert --to BMP --output-dir "$smoke_dir/batch-bmp" --resize-fit 5x5 "PPM:$smoke_dir/batch-ppm.ppm" "PGM:$smoke_dir/batch-pgm.pgm"
  "$install_dir/imx" identify "BMP:$smoke_dir/batch-bmp/batch-ppm.bmp" | grep -F "format=BMP width=5 height=3" >/dev/null
  "$install_dir/imx" identify "BMP:$smoke_dir/batch-bmp/batch-pgm.bmp" | grep -F "format=BMP width=5 height=3" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.pbm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.png" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/output.ppm" >/dev/null
  "$install_dir/imx" identify "PBM:$smoke_dir/prefix-output.pbm" >/dev/null
  "$install_dir/imx" identify "PGM:$smoke_dir/prefix-output.pgm" >/dev/null
  "$install_dir/imx" identify "PNG:$smoke_dir/prefix-output.png" >/dev/null
  "$install_dir/imx" identify "PPM:$smoke_dir/prefix-output.ppm" >/dev/null
  "$install_dir/imx" identify "BMP:$smoke_dir/prefix-output.bmp" >/dev/null
  "$install_dir/imx" "$smoke_dir/output.ff" "$smoke_dir/rewrite.ff"
  "$install_dir/imx" "$smoke_dir/output.bmp" "$smoke_dir/rewrite.bmp"
  "$install_dir/imx" "$smoke_dir/output.qoi" "$smoke_dir/rewrite.qoi"
  "$install_dir/imx" "$smoke_dir/input.pbm" "$smoke_dir/rewrite.pbm"
  "$install_dir/imx" "$smoke_dir/input.pgm" "$smoke_dir/rewrite.pgm"
  "$install_dir/imx" "$smoke_dir/output.png" "$smoke_dir/rewrite.png"
  "$install_dir/imx" "$smoke_dir/input.ppm" "$smoke_dir/rewrite.ppm"
  "$install_dir/imx" identify "$smoke_dir/rewrite.ff" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.bmp" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.qoi" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.pbm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.pgm" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.png" >/dev/null
  "$install_dir/imx" identify "$smoke_dir/rewrite.ppm" >/dev/null
  echo "smoke passed"
fi

echo "installed $install_dir/imx"
