#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"imx-preview","version":"\([^"]*\)".*/\1/p')"
if [[ -z "$version" ]]; then
  echo "error: failed to read imx-preview package version" >&2
  exit 1
fi
target="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -n "${IMX_EXPECTED_TARGET:-}" && "$target" != "$IMX_EXPECTED_TARGET" ]]; then
  echo "error: expected Rust host target $IMX_EXPECTED_TARGET, got $target" >&2
  exit 1
fi
artifact_dir="$root/target/release-artifacts"
staging="$root/target/imx-preview-$version-$target"
archive_name="imx-preview-$version-$target.tar.gz"
archive_path="$artifact_dir/$archive_name"
rm -rf "$artifact_dir" "$staging"
mkdir -p "$artifact_dir" "$staging"

cargo build --release -p imx-cli --bin imx

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required for deterministic release packaging" >&2
  exit 1
fi

cp "$root/target/release/imx" "$staging/"
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

if command -v shasum >/dev/null 2>&1; then
  (cd "$artifact_dir" && shasum -a 256 "$archive_name" >SHA256SUMS)
else
  (cd "$artifact_dir" && sha256sum "$archive_name" >SHA256SUMS)
fi

echo "$artifact_dir"
