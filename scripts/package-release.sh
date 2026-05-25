#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

metadata="$(cargo metadata --no-deps --format-version 1)"
package_version() {
  local package="$1"
  printf '%s\n' "$metadata" | tr '{' '\n' | sed -n "s/.*\"name\":\"$package\",\"version\":\"\([^\"]*\)\".*/\1/p" | head -n 1
}

version="$(package_version imx-preview)"
if [[ -z "$version" ]]; then
  echo "error: failed to read imx-preview package version" >&2
  exit 1
fi
for package in imx-cli imx-core imx-codec-farbfeld imx-codec-pnm imx-codec-qoi; do
  package_version_value="$(package_version "$package")"
  if [[ "$package_version_value" != "$version" ]]; then
    echo "error: package $package version $package_version_value does not match imx-preview $version" >&2
    exit 1
  fi
done
if ! grep -q "version=\"\${IMX_VERSION:-v$version}\"" scripts/install.sh; then
  echo "error: scripts/install.sh default version does not match v$version" >&2
  exit 1
fi
host_target="$(rustc -vV | sed -n 's/^host: //p')"
target="${IMX_PACKAGE_TARGET:-$host_target}"
if [[ -n "${IMX_EXPECTED_TARGET:-}" && "$target" != "$IMX_EXPECTED_TARGET" ]]; then
  echo "error: expected release target $IMX_EXPECTED_TARGET, got $target" >&2
  exit 1
fi
if [[ "$target" != "$host_target" ]]; then
  if [[ -z "${IMX_PACKAGE_RUNNER:-}" ]]; then
    echo "error: IMX_PACKAGE_RUNNER is required to smoke cross-target release archive $target on host $host_target" >&2
    exit 2
  fi
  if [[ -z "${IMX_LINKAGE_COMMAND:-}" ]]; then
    echo "error: IMX_LINKAGE_COMMAND is required to inspect cross-target release archive $target on host $host_target" >&2
    exit 2
  fi
fi
artifact_dir="${IMX_ARTIFACT_DIR:-$root/target/release-artifacts}"
if [[ "$artifact_dir" != /* ]]; then
  artifact_dir="$root/$artifact_dir"
fi
staging="$root/target/imx-preview-$version-$target"
archive_name="imx-preview-$version-$target.tar.gz"
archive_path="$artifact_dir/$archive_name"
rm -rf "$artifact_dir" "$staging"
mkdir -p "$artifact_dir" "$staging"

build_args=(--release --locked -p imx-cli --bin imx)
binary_dir="$root/target/release"
if [[ "$target" != "$host_target" ]]; then
  build_args+=(--target "$target")
  binary_dir="$root/target/$target/release"
fi
cargo build "${build_args[@]}"

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required for deterministic release packaging" >&2
  exit 1
fi

cp "$binary_dir/imx" "$staging/"
cp "$root/README.md" "$staging/"
cp "$root/COMPATIBILITY.md" "$staging/"
cp "$root/RELEASE_NOTES.md" "$staging/"
cp "$root/PRODUCTION_READINESS.md" "$staging/"
mkdir -p "$staging/scripts"
cp "$root/scripts/install.sh" "$staging/scripts/"
for doc in LICENSE NOTICE; do
  if [[ -f "$root/$doc" ]]; then
    cp "$root/$doc" "$staging/"
  elif [[ -f "$root/../$doc" ]]; then
    cp "$root/../$doc" "$staging/"
  fi
done

export IMX_RELEASE_STAGING="$staging"
export IMX_RELEASE_ARCHIVE="$archive_path"
python3 <<'PY'
import gzip
import os
import stat
import tarfile
from pathlib import Path

staging = Path(os.environ["IMX_RELEASE_STAGING"])
archive_path = Path(os.environ["IMX_RELEASE_ARCHIVE"])
root = staging.parent

paths = [staging]
paths.extend(sorted(staging.rglob("*"), key=lambda path: str(path.relative_to(root))))

with archive_path.open("wb") as raw:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for path in paths:
                archive_name = str(path.relative_to(root))
                info = archive.gettarinfo(str(path), archive_name)
                info.uid = 0
                info.gid = 0
                info.uname = ""
                info.gname = ""
                info.mtime = 0
                if path.is_dir():
                    info.mode = 0o755
                    archive.addfile(info)
                    continue
                if path.stat().st_mode & stat.S_IXUSR:
                    info.mode = 0o755
                else:
                    info.mode = 0o644
                with path.open("rb") as file:
                    archive.addfile(info, file)
PY

verify_dir="$(mktemp -d "$root/target/package-smoke.XXXXXX")"
trap 'rm -rf "$verify_dir"' EXIT
tar -xzf "$archive_path" -C "$verify_dir"
packaged_binary="$verify_dir/imx-preview-$version-$target/imx"
runner=()
if [[ -n "${IMX_PACKAGE_RUNNER:-}" ]]; then
  read -r -a runner <<<"$IMX_PACKAGE_RUNNER"
fi
run_packaged_binary() {
  if ((${#runner[@]})); then
    "${runner[@]}" "$packaged_binary" "$@"
  else
    "$packaged_binary" "$@"
  fi
}

packaged_version="$(run_packaged_binary --version)"
if [[ "$packaged_version" != "imx $version" ]]; then
  echo "error: packaged binary version mismatch: expected imx $version, got $packaged_version" >&2
  exit 1
fi
file "$packaged_binary" >"$artifact_dir/file-$target.txt"
case "$target" in
  aarch64-unknown-linux-gnu)
    grep -E 'ARM aarch64|AArch64|ARM64' "$artifact_dir/file-$target.txt" >/dev/null
    ;;
esac
if [[ -n "${IMX_LINKAGE_COMMAND:-}" ]]; then
  read -r -a linkage_command <<<"$IMX_LINKAGE_COMMAND"
  "${linkage_command[@]}" "$packaged_binary" >"$artifact_dir/linkage-$target.txt"
elif [[ "$(uname -s)" == "Darwin" ]]; then
  otool -L "$packaged_binary" >"$artifact_dir/linkage-$target.txt"
else
  ldd "$packaged_binary" >"$artifact_dir/linkage-$target.txt"
fi
! grep -E 'Magick(Core|Wand)|ImageMagick' "$artifact_dir/linkage-$target.txt"
printf 'P3\n2 1\n255\n255 0 0 0 0 255\n' >"$verify_dir/input.ppm"
printf 'P2\n2 1\n255\n0 255\n' >"$verify_dir/input.pgm"
printf 'P1\n2 1\n0 1\n' >"$verify_dir/input.pbm"
run_packaged_binary identify "$verify_dir/input.ppm" >/dev/null
run_packaged_binary identify "$verify_dir/input.pgm" >/dev/null
run_packaged_binary identify "$verify_dir/input.pbm" >/dev/null
run_packaged_binary identify "PPM:$verify_dir/input.ppm" >/dev/null
run_packaged_binary identify "PGM:$verify_dir/input.pgm" >/dev/null
run_packaged_binary identify "PBM:$verify_dir/input.pbm" >/dev/null
run_packaged_binary "$verify_dir/input.ppm" "$verify_dir/output.ff"
run_packaged_binary identify "$verify_dir/output.ff" >/dev/null
run_packaged_binary identify "FARBFELD:$verify_dir/output.ff" >/dev/null
run_packaged_binary "$verify_dir/output.ff" "$verify_dir/output.qoi"
run_packaged_binary identify "$verify_dir/output.qoi" >/dev/null
run_packaged_binary identify "QOI:$verify_dir/output.qoi" >/dev/null
run_packaged_binary "PPM:$verify_dir/input.ppm" "FARBFELD:$verify_dir/prefix-output.ff"
run_packaged_binary "FARBFELD:$verify_dir/prefix-output.ff" "QOI:$verify_dir/prefix-output.qoi"
run_packaged_binary "FARBFELD:$verify_dir/prefix-output.ff" "PBM:$verify_dir/prefix-output.pbm"
run_packaged_binary "FARBFELD:$verify_dir/prefix-output.ff" "PGM:$verify_dir/prefix-output.pgm"
run_packaged_binary "FARBFELD:$verify_dir/prefix-output.ff" "PPM:$verify_dir/prefix-output.ppm"
run_packaged_binary "$verify_dir/output.ff" "$verify_dir/rewrite.ff"
run_packaged_binary "$verify_dir/output.qoi" "$verify_dir/rewrite.qoi"
run_packaged_binary "$verify_dir/input.pbm" "$verify_dir/rewrite.pbm"
run_packaged_binary "$verify_dir/input.pgm" "$verify_dir/rewrite.pgm"
run_packaged_binary "$verify_dir/input.ppm" "$verify_dir/rewrite.ppm"
run_packaged_binary identify "$verify_dir/rewrite.ff" >/dev/null
run_packaged_binary identify "$verify_dir/rewrite.qoi" >/dev/null
run_packaged_binary identify "$verify_dir/rewrite.pbm" >/dev/null
run_packaged_binary identify "$verify_dir/rewrite.pgm" >/dev/null
run_packaged_binary identify "$verify_dir/rewrite.ppm" >/dev/null
run_packaged_binary identify "QOI:$verify_dir/prefix-output.qoi" >/dev/null
run_packaged_binary identify "PBM:$verify_dir/prefix-output.pbm" >/dev/null
run_packaged_binary identify "PGM:$verify_dir/prefix-output.pgm" >/dev/null
run_packaged_binary identify "PPM:$verify_dir/prefix-output.ppm" >/dev/null

if command -v shasum >/dev/null 2>&1; then
  (cd "$artifact_dir" && shasum -a 256 "$archive_name" >SHA256SUMS)
else
  (cd "$artifact_dir" && sha256sum "$archive_name" >SHA256SUMS)
fi

echo "$artifact_dir"
