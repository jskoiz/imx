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
case "$target" in
  *-unknown-linux-gnu)
    bash scripts/check-glibc-symbols.sh "$binary" >"$work_dir/glibc-symbols.txt"
    ;;
esac

archive_version="$(run_archive_binary --version)"
if [[ "$archive_version" != "imx ${version#v}" ]]; then
  echo "error: archive binary version mismatch: expected imx ${version#v}, got $archive_version" >&2
  exit 1
fi
run_archive_binary self-test >/dev/null

smoke_dir="$work_dir/smoke"
mkdir -p "$smoke_dir"
printf 'P3\n2 2\n255\n255 0 0 0 255 0 0 0 255 255 255 255\n' >"$smoke_dir/input.ppm"
printf 'P6\n2 1\n65535\n\x12\x34\x56\x78\x9a\xbc\x00\x00\x80\x00\xff\xff' >"$smoke_dir/input16.ppm"
printf 'P2\n2 2\n255\n0 85 170 255\n' >"$smoke_dir/input.pgm"
printf 'P1\n2 2\n0 1\n1 0\n' >"$smoke_dir/input.pbm"
printf 'P3\n2 1\n255\n255 0 0 0 0 255\n' >"$smoke_dir/fit-input.ppm"
printf 'P2\n2 1\n255\n0 255\n' >"$smoke_dir/fit-input.pgm"
printf 'P1\n2 1\n0 1\n' >"$smoke_dir/fit-input.pbm"
printf 'P3\n# v0.12 intake fixture\n2 1\n1023\n0 512 1023\n1023 256 128\n' >"$smoke_dir/intake-comments.ppm"
printf 'P5\n2 1\n65535\n\x12\x34\xff\xff' >"$smoke_dir/intake-pgm16.pgm"

run_archive_binary identify "$smoke_dir/input.ppm" >"$smoke_dir/identify-ppm.txt"
run_archive_binary identify "$smoke_dir/input16.ppm" >"$smoke_dir/identify-ppm16.txt"
run_archive_binary identify "$smoke_dir/input.pgm" >"$smoke_dir/identify-pgm.txt"
run_archive_binary identify "$smoke_dir/input.pbm" >"$smoke_dir/identify-pbm.txt"
run_archive_binary identify "PPM:$smoke_dir/intake-comments.ppm" >"$smoke_dir/identify-intake-comments-ppm.txt"
grep -Fx 'format=PPM width=2 height=1 channels=RGB depth=16' "$smoke_dir/identify-intake-comments-ppm.txt"
run_archive_binary identify "PGM:$smoke_dir/intake-pgm16.pgm" >"$smoke_dir/identify-intake-pgm16.txt"
grep -Fx 'format=PGM width=2 height=1 channels=GRAY depth=16' "$smoke_dir/identify-intake-pgm16.txt"
run_archive_binary identify "PPM:$smoke_dir/input.ppm" >"$smoke_dir/identify-prefix-ppm.txt"
run_archive_binary identify "PPM:$smoke_dir/input16.ppm" >"$smoke_dir/identify-prefix-ppm16.txt"
run_archive_binary identify "PGM:$smoke_dir/input.pgm" >"$smoke_dir/identify-prefix-pgm.txt"
run_archive_binary identify "PBM:$smoke_dir/input.pbm" >"$smoke_dir/identify-prefix-pbm.txt"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/output.qoi"
run_archive_binary "$smoke_dir/input16.ppm" "$smoke_dir/output16.ff"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/output.jpg"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/output.bmp"
run_archive_binary "PPM:$smoke_dir/intake-comments.ppm" "PGM:$smoke_dir/intake-comments.pgm"
run_archive_binary "PGM:$smoke_dir/intake-pgm16.pgm" "FARBFELD:$smoke_dir/intake-pgm16.ff"
run_archive_binary identify "$smoke_dir/output.qoi" >"$smoke_dir/identify-qoi.txt"
run_archive_binary identify "$smoke_dir/output.jpg" >"$smoke_dir/identify-jpeg.txt"
run_archive_binary identify "$smoke_dir/output.bmp" >"$smoke_dir/identify-bmp.txt"
run_archive_binary identify "$smoke_dir/output16.ff" >"$smoke_dir/identify-farbfeld16.txt"
run_archive_binary identify "QOI:$smoke_dir/output.qoi" >"$smoke_dir/identify-prefix-qoi.txt"
run_archive_binary identify "JPEG:$smoke_dir/output.jpg" >"$smoke_dir/identify-prefix-jpeg.txt"
run_archive_binary identify "BMP:$smoke_dir/output.bmp" >"$smoke_dir/identify-prefix-bmp.txt"
python3 - "$smoke_dir/progressive-rgb.jpg" <<'PY'
import sys

hex_bytes = (
    "ffd8ffe000104a46494600010100000100010000ffdb004300030202020202030202020303030304060404040404080606050609080a0a090809090a0c0f0c0a0b0e0b09090d110d0e0f101011100a0c12131210130f101010"
    "ffdb00430103030304030408040408100b090b1010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010ffc20011080003000403011100021101031101"
    "ffc40014000100000000000000000000000000000006ffc4001501010100000000000000000000000000000205ffda000c030100021003100000011d347fffc4001510010100000000000000000000000000000503"
    "ffda000801010001050265140ca7ffc4001f1100020005050000000000000000000000010200030531411112131421ffda0008010301013f01a5d427f5f9491ba616763a0f59c96636c936b0c47f"
    "ffc4001f1100020005050000000000000000000000010200041112210305142271ffda0008010201013f01e34b6e88af39a28c56e03a2e05ec683181524fa498ffc4001c1000030002030100000000000000000000010203041100058191"
    "ffda0008010100063f02c75ebb36f8cb680a389d0a82db2bbf8aa3ce7fffc400161001010100000000000000000000000000011100ffda0008010100013f2111a3b847819763ffda000c030100020003000000103f"
    "ffc4001811010100030000000000000000000000000111002131ffda0008010301013f104d4241dd88295313800033ffc4001811010100030000000000000000000000000121001131ffda0008010201013f106ca63d7160487003d0573f"
    "ffc400161001010100000000000000000000000000011121ffda0008010100013f1066307afe986b00a05ad5ffd9"
)
open(sys.argv[1], "wb").write(bytes.fromhex(hex_bytes))
PY
run_archive_binary identify "JPEG:$smoke_dir/progressive-rgb.jpg" >"$smoke_dir/identify-progressive-jpeg.txt"
grep -Fx 'format=JPEG width=4 height=3 channels=RGB depth=8' "$smoke_dir/identify-progressive-jpeg.txt"
run_archive_binary "JPEG:$smoke_dir/progressive-rgb.jpg" "PPM:$smoke_dir/progressive-rgb.ppm"
run_archive_binary identify "PPM:$smoke_dir/progressive-rgb.ppm" >"$smoke_dir/identify-progressive-ppm.txt"
grep -Fx 'format=PPM width=4 height=3 channels=RGB depth=8' "$smoke_dir/identify-progressive-ppm.txt"
python3 - "$smoke_dir/progressive-rgb.jpg" "$smoke_dir/progressive-o6.jpg" <<'PY'
import sys

source, output = sys.argv[1:3]
jpeg = open(source, "rb").read()
app1 = (
    b"Exif\0\0MM\0*\0\0\0\x08"
    + (1).to_bytes(2, "big")
    + (0x0112).to_bytes(2, "big")
    + (3).to_bytes(2, "big")
    + (1).to_bytes(4, "big")
    + (6).to_bytes(2, "big")
    + b"\0\0"
    + (0).to_bytes(4, "big")
)
segment = b"\xff\xe1" + (len(app1) + 2).to_bytes(2, "big") + app1
open(output, "wb").write(jpeg[:2] + segment + jpeg[2:])
PY
run_archive_binary identify "JPEG:$smoke_dir/progressive-o6.jpg" >"$smoke_dir/identify-progressive-orientation-jpeg.txt"
grep -Fx 'format=JPEG width=3 height=4 channels=RGB depth=8' "$smoke_dir/identify-progressive-orientation-jpeg.txt"
run_archive_binary "JPEG:$smoke_dir/progressive-o6.jpg" "PPM:$smoke_dir/progressive-o6.ppm"
run_archive_binary identify "PPM:$smoke_dir/progressive-o6.ppm" >"$smoke_dir/identify-progressive-orientation-ppm.txt"
grep -Fx 'format=PPM width=3 height=4 channels=RGB depth=8' "$smoke_dir/identify-progressive-orientation-ppm.txt"
printf 'P3\n2 1\n255\n255 0 0 0 0 255\n' >"$smoke_dir/orientation-source.ppm"
run_archive_binary "$smoke_dir/orientation-source.ppm" "$smoke_dir/orientation-source.jpg"
python3 - "$smoke_dir/orientation-source.jpg" "$smoke_dir/oriented-o6.jpg" <<'PY'
import sys

source, output = sys.argv[1:3]
jpeg = open(source, "rb").read()
app1 = (
    b"Exif\0\0MM\0*\0\0\0\x08"
    + (1).to_bytes(2, "big")
    + (0x0112).to_bytes(2, "big")
    + (3).to_bytes(2, "big")
    + (1).to_bytes(4, "big")
    + (6).to_bytes(2, "big")
    + b"\0\0"
    + (0).to_bytes(4, "big")
)
segment = b"\xff\xe1" + (len(app1) + 2).to_bytes(2, "big") + app1
open(output, "wb").write(jpeg[:2] + segment + jpeg[2:])
PY
run_archive_binary identify "JPEG:$smoke_dir/oriented-o6.jpg" >"$smoke_dir/identify-orientation-jpeg.txt"
grep -Fx 'format=JPEG width=1 height=2 channels=RGB depth=8' "$smoke_dir/identify-orientation-jpeg.txt"
run_archive_binary "JPEG:$smoke_dir/oriented-o6.jpg" "PPM:$smoke_dir/oriented-o6.ppm"
run_archive_binary identify "PPM:$smoke_dir/oriented-o6.ppm" >"$smoke_dir/identify-orientation-ppm.txt"
grep -Fx 'format=PPM width=1 height=2 channels=RGB depth=8' "$smoke_dir/identify-orientation-ppm.txt"
run_archive_binary identify "FARBFELD:$smoke_dir/output16.ff" >"$smoke_dir/identify-prefix-farbfeld16.txt"
run_archive_binary "$smoke_dir/output.qoi" "$smoke_dir/output.ff"
run_archive_binary "JPEG:$smoke_dir/output.jpg" "FARBFELD:$smoke_dir/jpeg-output.ff"
run_archive_binary identify "$smoke_dir/output.ff" >"$smoke_dir/identify-farbfeld.txt"
run_archive_binary identify "$smoke_dir/jpeg-output.ff" >"$smoke_dir/identify-jpeg-output-farbfeld.txt"
run_archive_binary identify "FARBFELD:$smoke_dir/output.ff" >"$smoke_dir/identify-prefix-farbfeld.txt"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.pbm"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.pgm"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.png"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/output.ppm"
run_archive_binary "PPM:$smoke_dir/input.ppm" "QOI:$smoke_dir/prefix-output.qoi"
run_archive_binary "PPM:$smoke_dir/input.ppm" "JPEG:$smoke_dir/prefix-output.jpg"
run_archive_binary "JPEG:$smoke_dir/prefix-output.jpg" "FARBFELD:$smoke_dir/prefix-jpeg-output.ff"
run_archive_binary "PPM:$smoke_dir/input16.ppm" "PPM:$smoke_dir/prefix-output16.ppm"
run_archive_binary "QOI:$smoke_dir/prefix-output.qoi" "FARBFELD:$smoke_dir/prefix-output.ff"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PBM:$smoke_dir/prefix-output.pbm"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PGM:$smoke_dir/prefix-output.pgm"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PNG:$smoke_dir/prefix-output.png"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "PPM:$smoke_dir/prefix-output.ppm"
run_archive_binary "FARBFELD:$smoke_dir/prefix-output.ff" "BMP:$smoke_dir/prefix-output.bmp"
run_archive_binary resize 17x11 "FARBFELD:$smoke_dir/output.ff" "FARBFELD:$smoke_dir/resized.ff"
run_archive_binary resize 17x11 "BMP:$smoke_dir/output.bmp" "BMP:$smoke_dir/resized.bmp"
run_archive_binary resize 17x11 "JPEG:$smoke_dir/output.jpg" "JPEG:$smoke_dir/resized.jpg"
run_archive_binary resize 17x11 "QOI:$smoke_dir/output.qoi" "QOI:$smoke_dir/resized.qoi"
run_archive_binary resize 17x11 "PBM:$smoke_dir/input.pbm" "PBM:$smoke_dir/resized.pbm"
run_archive_binary resize 17x11 "PGM:$smoke_dir/input.pgm" "PGM:$smoke_dir/resized.pgm"
run_archive_binary resize 17x11 "PNG:$smoke_dir/output.png" "PNG:$smoke_dir/resized.png"
run_archive_binary resize 17x11 "PPM:$smoke_dir/input.ppm" "PPM:$smoke_dir/resized.ppm"
run_archive_binary identify "FARBFELD:$smoke_dir/resized.ff" >"$smoke_dir/identify-resized-farbfeld.txt"
run_archive_binary identify "BMP:$smoke_dir/resized.bmp" >"$smoke_dir/identify-resized-bmp.txt"
run_archive_binary identify "JPEG:$smoke_dir/resized.jpg" >"$smoke_dir/identify-resized-jpeg.txt"
run_archive_binary identify "QOI:$smoke_dir/resized.qoi" >"$smoke_dir/identify-resized-qoi.txt"
run_archive_binary identify "PBM:$smoke_dir/resized.pbm" >"$smoke_dir/identify-resized-pbm.txt"
run_archive_binary identify "PGM:$smoke_dir/resized.pgm" >"$smoke_dir/identify-resized-pgm.txt"
run_archive_binary identify "PNG:$smoke_dir/resized.png" >"$smoke_dir/identify-resized-png.txt"
run_archive_binary identify "PPM:$smoke_dir/resized.ppm" >"$smoke_dir/identify-resized-ppm.txt"
grep -F 'format=FARBFELD width=17 height=11' "$smoke_dir/identify-resized-farbfeld.txt"
grep -F 'format=BMP width=17 height=11' "$smoke_dir/identify-resized-bmp.txt"
grep -F 'format=JPEG width=17 height=11' "$smoke_dir/identify-resized-jpeg.txt"
grep -F 'format=QOI width=17 height=11' "$smoke_dir/identify-resized-qoi.txt"
grep -F 'format=PBM width=17 height=11' "$smoke_dir/identify-resized-pbm.txt"
grep -F 'format=PGM width=17 height=11' "$smoke_dir/identify-resized-pgm.txt"
grep -F 'format=PNG width=17 height=11' "$smoke_dir/identify-resized-png.txt"
grep -F 'format=PPM width=17 height=11' "$smoke_dir/identify-resized-ppm.txt"
run_archive_binary "PPM:$smoke_dir/fit-input.ppm" "FARBFELD:$smoke_dir/fit-source.ff"
run_archive_binary "PPM:$smoke_dir/fit-input.ppm" "BMP:$smoke_dir/fit-source.bmp"
run_archive_binary "PPM:$smoke_dir/fit-input.ppm" "JPEG:$smoke_dir/fit-source.jpg"
run_archive_binary "PPM:$smoke_dir/fit-input.ppm" "QOI:$smoke_dir/fit-source.qoi"
run_archive_binary "PPM:$smoke_dir/fit-input.ppm" "PNG:$smoke_dir/fit-source.png"
run_archive_binary resize-fit 5x5 "FARBFELD:$smoke_dir/fit-source.ff" "FARBFELD:$smoke_dir/fit.ff"
run_archive_binary resize-fit 5x5 "BMP:$smoke_dir/fit-source.bmp" "BMP:$smoke_dir/fit.bmp"
run_archive_binary resize-fit 5x5 "JPEG:$smoke_dir/fit-source.jpg" "JPEG:$smoke_dir/fit.jpg"
run_archive_binary resize-fit 5x5 "QOI:$smoke_dir/fit-source.qoi" "QOI:$smoke_dir/fit.qoi"
run_archive_binary resize-fit 5x5 "PBM:$smoke_dir/fit-input.pbm" "PBM:$smoke_dir/fit.pbm"
run_archive_binary resize-fit 5x5 "PGM:$smoke_dir/fit-input.pgm" "PGM:$smoke_dir/fit.pgm"
run_archive_binary resize-fit 5x5 "PNG:$smoke_dir/fit-source.png" "PNG:$smoke_dir/fit.png"
run_archive_binary resize-fit 5x5 "PPM:$smoke_dir/fit-input.ppm" "PPM:$smoke_dir/fit.ppm"
run_archive_binary identify "FARBFELD:$smoke_dir/fit.ff" >"$smoke_dir/identify-fit-farbfeld.txt"
run_archive_binary identify "BMP:$smoke_dir/fit.bmp" >"$smoke_dir/identify-fit-bmp.txt"
run_archive_binary identify "JPEG:$smoke_dir/fit.jpg" >"$smoke_dir/identify-fit-jpeg.txt"
run_archive_binary identify "QOI:$smoke_dir/fit.qoi" >"$smoke_dir/identify-fit-qoi.txt"
run_archive_binary identify "PBM:$smoke_dir/fit.pbm" >"$smoke_dir/identify-fit-pbm.txt"
run_archive_binary identify "PGM:$smoke_dir/fit.pgm" >"$smoke_dir/identify-fit-pgm.txt"
run_archive_binary identify "PNG:$smoke_dir/fit.png" >"$smoke_dir/identify-fit-png.txt"
run_archive_binary identify "PPM:$smoke_dir/fit.ppm" >"$smoke_dir/identify-fit-ppm.txt"
grep -F 'format=FARBFELD width=5 height=3' "$smoke_dir/identify-fit-farbfeld.txt"
grep -F 'format=BMP width=5 height=3' "$smoke_dir/identify-fit-bmp.txt"
grep -F 'format=JPEG width=5 height=3' "$smoke_dir/identify-fit-jpeg.txt"
grep -F 'format=QOI width=5 height=3' "$smoke_dir/identify-fit-qoi.txt"
grep -F 'format=PBM width=5 height=3' "$smoke_dir/identify-fit-pbm.txt"
grep -F 'format=PGM width=5 height=3' "$smoke_dir/identify-fit-pgm.txt"
grep -F 'format=PNG width=5 height=3' "$smoke_dir/identify-fit-png.txt"
grep -F 'format=PPM width=5 height=3' "$smoke_dir/identify-fit-ppm.txt"
cp "$smoke_dir/fit-input.ppm" "$smoke_dir/batch-ppm.ppm"
cp "$smoke_dir/fit-input.pgm" "$smoke_dir/batch-pgm.pgm"
mkdir -p "$smoke_dir/batch"
run_archive_binary batch-convert --to PPM --output-dir "$smoke_dir/batch" --resize-fit 5x5 "PPM:$smoke_dir/batch-ppm.ppm" "PGM:$smoke_dir/batch-pgm.pgm"
run_archive_binary identify "PPM:$smoke_dir/batch/batch-ppm.ppm" >"$smoke_dir/identify-batch-ppm.txt"
run_archive_binary identify "PPM:$smoke_dir/batch/batch-pgm.ppm" >"$smoke_dir/identify-batch-pgm.txt"
grep -F 'format=PPM width=5 height=3' "$smoke_dir/identify-batch-ppm.txt"
grep -F 'format=PPM width=5 height=3' "$smoke_dir/identify-batch-pgm.txt"
mkdir -p "$smoke_dir/batch-bmp"
run_archive_binary batch-convert --to BMP --output-dir "$smoke_dir/batch-bmp" --resize-fit 5x5 "PPM:$smoke_dir/batch-ppm.ppm" "PGM:$smoke_dir/batch-pgm.pgm"
run_archive_binary identify "BMP:$smoke_dir/batch-bmp/batch-ppm.bmp" >"$smoke_dir/identify-batch-bmp-ppm.txt"
run_archive_binary identify "BMP:$smoke_dir/batch-bmp/batch-pgm.bmp" >"$smoke_dir/identify-batch-bmp-pgm.txt"
grep -F 'format=BMP width=5 height=3' "$smoke_dir/identify-batch-bmp-ppm.txt"
grep -F 'format=BMP width=5 height=3' "$smoke_dir/identify-batch-bmp-pgm.txt"
run_archive_binary "$smoke_dir/output.ff" "$smoke_dir/rewrite.ff"
run_archive_binary "$smoke_dir/output.bmp" "$smoke_dir/rewrite.bmp"
run_archive_binary "$smoke_dir/output.jpg" "$smoke_dir/rewrite.jpg"
run_archive_binary "$smoke_dir/output.qoi" "$smoke_dir/rewrite.qoi"
run_archive_binary "$smoke_dir/input.pbm" "$smoke_dir/rewrite.pbm"
run_archive_binary "$smoke_dir/input.pgm" "$smoke_dir/rewrite.pgm"
run_archive_binary "$smoke_dir/output.png" "$smoke_dir/rewrite.png"
run_archive_binary "$smoke_dir/input.ppm" "$smoke_dir/rewrite.ppm"
run_archive_binary identify "$smoke_dir/output.pbm" >"$smoke_dir/identify-output-pbm.txt"
run_archive_binary identify "$smoke_dir/output.pgm" >"$smoke_dir/identify-output-pgm.txt"
run_archive_binary identify "$smoke_dir/output.png" >"$smoke_dir/identify-output-png.txt"
run_archive_binary identify "$smoke_dir/output.ppm" >"$smoke_dir/identify-output-ppm.txt"
run_archive_binary identify "$smoke_dir/rewrite.ff" >"$smoke_dir/identify-rewrite-farbfeld.txt"
run_archive_binary identify "$smoke_dir/rewrite.bmp" >"$smoke_dir/identify-rewrite-bmp.txt"
run_archive_binary identify "$smoke_dir/rewrite.jpg" >"$smoke_dir/identify-rewrite-jpeg.txt"
run_archive_binary identify "$smoke_dir/rewrite.qoi" >"$smoke_dir/identify-rewrite-qoi.txt"
run_archive_binary identify "$smoke_dir/rewrite.pbm" >"$smoke_dir/identify-rewrite-pbm.txt"
run_archive_binary identify "$smoke_dir/rewrite.pgm" >"$smoke_dir/identify-rewrite-pgm.txt"
run_archive_binary identify "$smoke_dir/rewrite.png" >"$smoke_dir/identify-rewrite-png.txt"
run_archive_binary identify "$smoke_dir/rewrite.ppm" >"$smoke_dir/identify-rewrite-ppm.txt"
run_archive_binary identify "QOI:$smoke_dir/prefix-output.qoi" >"$smoke_dir/identify-prefix-output-qoi.txt"
run_archive_binary identify "JPEG:$smoke_dir/prefix-output.jpg" >"$smoke_dir/identify-prefix-output-jpeg.txt"
run_archive_binary identify "PBM:$smoke_dir/prefix-output.pbm" >"$smoke_dir/identify-prefix-output-pbm.txt"
run_archive_binary identify "PGM:$smoke_dir/prefix-output.pgm" >"$smoke_dir/identify-prefix-output-pgm.txt"
run_archive_binary identify "PNG:$smoke_dir/prefix-output.png" >"$smoke_dir/identify-prefix-output-png.txt"
run_archive_binary identify "PPM:$smoke_dir/prefix-output.ppm" >"$smoke_dir/identify-prefix-output-ppm.txt"
run_archive_binary identify "BMP:$smoke_dir/prefix-output.bmp" >"$smoke_dir/identify-prefix-output-bmp.txt"

cat >"$summary" <<EOF
{
  "schema_version": 1,
  "repo": "$repo",
  "version": "$version",
  "target": "$target",
  "archive": "$archive",
  "checksum_file": "downloads/SHA256SUMS",
  "linkage": "linkage.txt",
  "glibc_symbols": "glibc-symbols.txt",
  "smoke_dir": "smoke",
  "status": "passed"
}
EOF

echo "$work_dir"
