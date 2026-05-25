#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: generate-homebrew-formula.sh <version> <SHA256SUMS> <output.rb>" >&2
  exit 2
fi

version="$1"
checksums="$2"
output="$3"
repo="${IMX_REPO:-jskoiz/imx}"

if [[ "$version" == v* ]]; then
  tag="$version"
  formula_version="${version#v}"
else
  tag="v$version"
  formula_version="$version"
fi

checksum_for() {
  local archive="$1"
  awk -v archive="$archive" '$2 == archive { print $1 }' "$checksums"
}

linux_intel_archive="imx-preview-$formula_version-x86_64-unknown-linux-gnu.tar.gz"
linux_arm_archive="imx-preview-$formula_version-aarch64-unknown-linux-gnu.tar.gz"
mac_arm_archive="imx-preview-$formula_version-aarch64-apple-darwin.tar.gz"
mac_intel_archive="imx-preview-$formula_version-x86_64-apple-darwin.tar.gz"

linux_intel_sha="$(checksum_for "$linux_intel_archive")"
linux_arm_sha="$(checksum_for "$linux_arm_archive")"
mac_arm_sha="$(checksum_for "$mac_arm_archive")"
mac_intel_sha="$(checksum_for "$mac_intel_archive")"

if [[ -z "$linux_intel_sha" && -z "$linux_arm_sha" && -z "$mac_arm_sha" && -z "$mac_intel_sha" ]]; then
  echo "error: SHA256SUMS does not contain any supported IMX release archives" >&2
  exit 1
fi

mkdir -p "$(dirname "$output")"
{
cat <<EOF
class Imx < Formula
  desc "Standalone Rust image tool for FARBFELD, QOI, and Netpbm transcodes"
  homepage "https://github.com/$repo"
  license "ImageMagick"
EOF

if [[ -n "$mac_arm_sha" || -n "$mac_intel_sha" ]]; then
  cat <<EOF

  on_macos do
EOF
  if [[ -n "$mac_arm_sha" ]]; then
    cat <<EOF
    on_arm do
      url "https://github.com/$repo/releases/download/$tag/$mac_arm_archive"
      sha256 "$mac_arm_sha"
    end
EOF
  fi

  if [[ -n "$mac_arm_sha" && -n "$mac_intel_sha" ]]; then
    echo
  fi

  if [[ -n "$mac_intel_sha" ]]; then
    cat <<EOF
    on_intel do
      url "https://github.com/$repo/releases/download/$tag/$mac_intel_archive"
      sha256 "$mac_intel_sha"
    end
EOF
  fi

  cat <<'EOF'
  end
EOF
fi

if [[ -n "$linux_intel_sha" || -n "$linux_arm_sha" ]]; then
  cat <<EOF

  on_linux do
EOF
  if [[ -n "$linux_intel_sha" ]]; then
    cat <<EOF
    on_intel do
      url "https://github.com/$repo/releases/download/$tag/$linux_intel_archive"
      sha256 "$linux_intel_sha"
    end
EOF
  fi

  if [[ -n "$linux_intel_sha" && -n "$linux_arm_sha" ]]; then
    echo
  fi

  if [[ -n "$linux_arm_sha" ]]; then
    cat <<EOF
    on_arm do
      url "https://github.com/$repo/releases/download/$tag/$linux_arm_archive"
      sha256 "$linux_arm_sha"
    end
EOF
  fi

  cat <<'EOF'
  end
EOF
fi

cat <<'EOF'

  def install
    bin.install "imx"
    prefix.install "README.md", "COMPATIBILITY.md", "RELEASE_NOTES.md", "PRODUCTION_READINESS.md"
  end

  test do
    (testpath/"input.ppm").write "P3\n2 1\n255\n255 0 0 0 0 255\n"
    assert_match "format=PPM width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify input.ppm")
    system bin/"imx", "input.ppm", "output.qoi"
    assert_match "format=QOI width=2 height=1 channels=RGBA depth=8", shell_output("#{bin/"imx"} identify output.qoi")
  end
end
EOF
} >"$output"
