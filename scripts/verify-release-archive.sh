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

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os:$arch" in
    Linux:x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux:aarch64|Linux:arm64) echo "aarch64-unknown-linux-gnu" ;;
    Darwin:arm64|Darwin:aarch64) echo "aarch64-apple-darwin" ;;
    Darwin:x86_64) echo "x86_64-apple-darwin" ;;
    *)
      echo "error: unsupported platform: $os $arch" >&2
      exit 2
      ;;
  esac
}

host_target="$(detect_target)"
target="${IMX_RELEASE_TARGET:-$host_target}"
if [[ "$target" != "$host_target" ]]; then
  if [[ -z "${IMX_RELEASE_RUNNER:-}" ]]; then
    echo "error: IMX_RELEASE_RUNNER is required to smoke release archive $target on host $host_target" >&2
    exit 2
  fi
  if [[ -z "${IMX_LINKAGE_COMMAND:-}" ]]; then
    echo "error: IMX_LINKAGE_COMMAND is required to inspect release archive $target on host $host_target" >&2
    exit 2
  fi
fi
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
  cp "$IMX_RELEASE_DIR/$archive" "$download_dir/"
else
  download "$base_url/SHA256SUMS" "$download_dir/SHA256SUMS"
  download "$base_url/$archive" "$download_dir/$archive"
fi

(
  cd "$download_dir"
  expected_sha="$(
    awk -v archive="$archive" '$2 == archive { print $1 }' SHA256SUMS
  )"
  if [[ -z "$expected_sha" ]]; then
    echo "error: SHA256SUMS does not contain selected archive: $archive" >&2
    exit 1
  fi
  if command -v shasum >/dev/null 2>&1; then
    printf '%s  %s\n' "$expected_sha" "$archive" | shasum -a 256 -c -
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s  %s\n' "$expected_sha" "$archive" | sha256sum -c -
  else
    echo "error: shasum or sha256sum is required" >&2
    exit 2
  fi
  tar -xzf "$archive" -C "$extract_dir"
)

binary="$extract_dir/imx-preview-${version#v}-$target/imx"
runner=()
if [[ -n "${IMX_RELEASE_RUNNER:-}" ]]; then
  read -r -a runner <<<"$IMX_RELEASE_RUNNER"
fi
run_archive_binary() {
  if ((${#runner[@]})); then
    "${runner[@]}" "$binary" "$@"
  else
    "$binary" "$@"
  fi
}

archive_version="$(run_archive_binary --version)"
if [[ "$archive_version" != "imx ${version#v}" ]]; then
  echo "error: archive binary version mismatch: expected imx ${version#v}, got $archive_version" >&2
  exit 1
fi
file "$binary" >"$work_dir/file.txt"
case "$target" in
  aarch64-unknown-linux-gnu)
    grep -E 'ARM aarch64|AArch64|ARM64' "$work_dir/file.txt" >/dev/null
    ;;
esac

if [[ -n "${IMX_LINKAGE_COMMAND:-}" ]]; then
  read -r -a linkage_command <<<"$IMX_LINKAGE_COMMAND"
  "${linkage_command[@]}" "$binary" >"$work_dir/linkage.txt"
elif [[ "$(uname -s)" == "Darwin" ]]; then
  otool -L "$binary" >"$work_dir/linkage.txt"
else
  ldd "$binary" >"$work_dir/linkage.txt"
fi
! grep -E 'Magick(Core|Wand)|ImageMagick' "$work_dir/linkage.txt"

smoke_dir="$work_dir/smoke"
mkdir -p "$smoke_dir"
printf 'P3\n2 2\n255\n255 0 0 0 255 0 0 0 255 255 255 255\n' >"$smoke_dir/input.ppm"
printf 'P2\n2 2\n255\n0 85 170 255\n' >"$smoke_dir/input.pgm"
printf 'P1\n2 2\n0 1\n1 0\n' >"$smoke_dir/input.pbm"

run_archive_binary identify "$smoke_dir/input.ppm" >"$smoke_dir/identify-ppm.txt"
run_archive_binary identify "$smoke_dir/input.pgm" >"$smoke_dir/identify-pgm.txt"
run_archive_binary identify "$smoke_dir/input.pbm" >"$smoke_dir/identify-pbm.txt"
run_archive_binary identify "PPM:$smoke_dir/input.ppm" >"$smoke_dir/identify-prefix-ppm.txt"
run_archive_binary identify "PGM:$smoke_dir/input.pgm" >"$smoke_dir/identify-prefix-pgm.txt"
run_archive_binary identify "PBM:$smoke_dir/input.pbm" >"$smoke_dir/identify-prefix-pbm.txt"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/output.qoi"
run_archive_binary identify "$smoke_dir/output.qoi" >"$smoke_dir/identify-qoi.txt"
run_archive_binary identify "QOI:$smoke_dir/output.qoi" >"$smoke_dir/identify-prefix-qoi.txt"
run_archive_binary "$smoke_dir/output.qoi" "$smoke_dir/output.ff"
run_archive_binary identify "$smoke_dir/output.ff" >"$smoke_dir/identify-farbfeld.txt"
run_archive_binary identify "FARBFELD:$smoke_dir/output.ff" >"$smoke_dir/identify-prefix-farbfeld.txt"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.pbm"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.pgm"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.ppm"
run_archive_binary "PPM:$smoke_dir/input.ppm" "QOI:$smoke_dir/prefix-output.qoi"
run_archive_binary "QOI:$smoke_dir/prefix-output.qoi" "FARBFELD:$smoke_dir/prefix-output.ff"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PBM:$smoke_dir/prefix-output.pbm"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PGM:$smoke_dir/prefix-output.pgm"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PPM:$smoke_dir/prefix-output.ppm"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/rewrite.ff"
run_archive_binary "$smoke_dir/output.qoi" "$smoke_dir/rewrite.qoi"
run_archive_binary "$smoke_dir/input.pbm" "$smoke_dir/rewrite.pbm"
run_archive_binary "$smoke_dir/input.pgm" "$smoke_dir/rewrite.pgm"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/rewrite.ppm"
run_archive_binary identify "$smoke_dir/output.pbm" >"$smoke_dir/identify-output-pbm.txt"
run_archive_binary identify "$smoke_dir/output.pgm" >"$smoke_dir/identify-output-pgm.txt"
run_archive_binary identify "$smoke_dir/output.ppm" >"$smoke_dir/identify-output-ppm.txt"
run_archive_binary identify "$smoke_dir/rewrite.ff" >"$smoke_dir/identify-rewrite-farbfeld.txt"
run_archive_binary identify "$smoke_dir/rewrite.qoi" >"$smoke_dir/identify-rewrite-qoi.txt"
run_archive_binary identify "$smoke_dir/rewrite.pbm" >"$smoke_dir/identify-rewrite-pbm.txt"
run_archive_binary identify "$smoke_dir/rewrite.pgm" >"$smoke_dir/identify-rewrite-pgm.txt"
run_archive_binary identify "$smoke_dir/rewrite.ppm" >"$smoke_dir/identify-rewrite-ppm.txt"
run_archive_binary identify "QOI:$smoke_dir/prefix-output.qoi" >"$smoke_dir/identify-prefix-output-qoi.txt"
run_archive_binary identify "PBM:$smoke_dir/prefix-output.pbm" >"$smoke_dir/identify-prefix-output-pbm.txt"
run_archive_binary identify "PGM:$smoke_dir/prefix-output.pgm" >"$smoke_dir/identify-prefix-output-pgm.txt"
run_archive_binary identify "PPM:$smoke_dir/prefix-output.ppm" >"$smoke_dir/identify-prefix-output-ppm.txt"

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
