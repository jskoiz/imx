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

prefix_smoke=0
png_smoke=0
jpeg_smoke=0
jpeg_orientation_smoke=0
jpeg_progressive_smoke=0
intake_smoke=0
resize_smoke=0
if [[ "$formula_version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  major="${BASH_REMATCH[1]}"
  minor="${BASH_REMATCH[2]}"
  if ((major > 0 || minor >= 6)); then
    prefix_smoke=1
  fi
  if ((major > 0 || minor >= 8)); then
    png_smoke=1
  fi
  if ((major > 0 || minor >= 9)); then
    jpeg_smoke=1
  fi
  if ((major > 0 || minor >= 10)); then
    jpeg_orientation_smoke=1
  fi
  if ((major > 0 || minor >= 11)); then
    jpeg_progressive_smoke=1
  fi
  if ((major > 0 || minor >= 12)); then
    intake_smoke=1
  fi
  if ((major > 0 || minor >= 13)); then
    resize_smoke=1
  fi
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
  desc "Standalone Rust image tool for ImageMagick-compatible slices"
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

  def caveats
    return unless OS.linux?

    "Published Linux archives require glibc 2.34 or newer."
  end

  test do
    (testpath/"input.ppm").write "P3\n2 1\n255\n255 0 0 0 0 255\n"
    assert_match "format=PPM width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify input.ppm")
    system bin/"imx", "input.ppm", "output.qoi"
    assert_match "format=QOI width=2 height=1 channels=RGBA depth=8", shell_output("#{bin/"imx"} identify output.qoi")
EOF

if [[ "$prefix_smoke" == 1 ]]; then
  cat <<'EOF'
    assert_match "format=PPM width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PPM:input.ppm")
    assert_match "format=QOI width=2 height=1 channels=RGBA depth=8", shell_output("#{bin/"imx"} identify QOI:output.qoi")
    system bin/"imx", "PPM:input.ppm", "FARBFELD:prefix-output.ff"
    assert_match "format=FARBFELD width=2 height=1 channels=RGBA depth=16", shell_output("#{bin/"imx"} identify FARBFELD:prefix-output.ff")
EOF
fi

if [[ "$png_smoke" == 1 ]]; then
  cat <<'EOF'
    system bin/"imx", "input.ppm", "output.png"
    assert_match "format=PNG width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PNG:output.png")
    system bin/"imx", "PNG:output.png", "FARBFELD:png-output.ff"
    assert_match "format=FARBFELD width=2 height=1 channels=RGBA depth=16", shell_output("#{bin/"imx"} identify png-output.ff")
EOF
fi

if [[ "$jpeg_smoke" == 1 ]]; then
  cat <<'EOF'
    system bin/"imx", "input.ppm", "output.jpg"
    assert_match "format=JPEG width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify JPEG:output.jpg")
EOF
fi

if [[ "$jpeg_orientation_smoke" == 1 ]]; then
  cat <<'EOF'
    jpeg = File.binread("output.jpg")
    app1 = "Exif\0\0MM\0*\0\0\0\b".b + [1, 0x0112, 3, 1].pack("nnnN") + [6].pack("n") + "\0\0".b + [0].pack("N")
    segment = "\xff\xe1".b + [app1.bytesize + 2].pack("n") + app1
    File.binwrite("oriented-o6.jpg", jpeg.byteslice(0, 2) + segment + jpeg.byteslice(2, jpeg.bytesize - 2))
    assert_match "format=JPEG width=1 height=2 channels=RGB depth=8", shell_output("#{bin/"imx"} identify JPEG:oriented-o6.jpg")
    system bin/"imx", "JPEG:oriented-o6.jpg", "PPM:oriented-o6.ppm"
    assert_match "format=PPM width=1 height=2 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PPM:oriented-o6.ppm")
EOF
fi

if [[ "$jpeg_progressive_smoke" == 1 ]]; then
  cat <<'EOF'
    progressive_hex = "ffd8ffe000104a46494600010100000100010000ffdb004300030202020202030202020303030304060404040404080606050609080a0a090809090a0c0f0c0a0b0e0b09090d110d0e0f101011100a0c12131210130f101010" \
      "ffdb00430103030304030408040408100b090b1010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010ffc20011080003000403011100021101031101" \
      "ffc40014000100000000000000000000000000000006ffc4001501010100000000000000000000000000000205ffda000c030100021003100000011d347fffc4001510010100000000000000000000000000000503" \
      "ffda000801010001050265140ca7ffc4001f1100020005050000000000000000000000010200030531411112131421ffda0008010301013f01a5d427f5f9491ba616763a0f59c96636c936b0c47f" \
      "ffc4001f1100020005050000000000000000000000010200041112210305142271ffda0008010201013f01e34b6e88af39a28c56e03a2e05ec683181524fa498ffc4001c1000030002030100000000000000000000010203041100058191" \
      "ffda0008010100063f02c75ebb36f8cb680a389d0a82db2bbf8aa3ce7fffc400161001010100000000000000000000000000011100ffda0008010100013f2111a3b847819763ffda000c030100020003000000103f" \
      "ffc4001811010100030000000000000000000000000111002131ffda0008010301013f104d4241dd88295313800033ffc4001811010100030000000000000000000000000121001131ffda0008010201013f106ca63d7160487003d0573f" \
      "ffc400161001010100000000000000000000000000011121ffda0008010100013f1066307afe986b00a05ad5ffd9"
    File.binwrite("progressive-rgb.jpg", [progressive_hex].pack("H*"))
    assert_match "format=JPEG width=4 height=3 channels=RGB depth=8", shell_output("#{bin/"imx"} identify JPEG:progressive-rgb.jpg")
    system bin/"imx", "JPEG:progressive-rgb.jpg", "PPM:progressive-rgb.ppm"
    assert_match "format=PPM width=4 height=3 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PPM:progressive-rgb.ppm")
    progressive = File.binread("progressive-rgb.jpg")
    File.binwrite("progressive-o6.jpg", progressive.byteslice(0, 2) + segment + progressive.byteslice(2, progressive.bytesize - 2))
    assert_match "format=JPEG width=3 height=4 channels=RGB depth=8", shell_output("#{bin/"imx"} identify JPEG:progressive-o6.jpg")
    system bin/"imx", "JPEG:progressive-o6.jpg", "PPM:progressive-o6.ppm"
    assert_match "format=PPM width=3 height=4 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PPM:progressive-o6.ppm")
EOF
fi

if [[ "$intake_smoke" == 1 ]]; then
  cat <<'EOF'
    (testpath/"intake-comments.ppm").write "P3\n# v0.12 intake fixture\n2 1\n1023\n0 512 1023\n1023 256 128\n"
    (testpath/"intake-pgm16.pgm").write "P5\n2 1\n65535\n\x12\x34\xff\xff".b
    assert_match "format=PPM width=2 height=1 channels=RGB depth=16", shell_output("#{bin/"imx"} identify PPM:intake-comments.ppm")
    assert_match "format=PGM width=2 height=1 channels=GRAY depth=16", shell_output("#{bin/"imx"} identify PGM:intake-pgm16.pgm")
    system bin/"imx", "PPM:intake-comments.ppm", "PGM:intake-comments.pgm"
    system bin/"imx", "PGM:intake-pgm16.pgm", "FARBFELD:intake-pgm16.ff"
    assert_match "format=FARBFELD width=2 height=1 channels=RGBA depth=16", shell_output("#{bin/"imx"} identify FARBFELD:intake-pgm16.ff")
EOF
fi

if [[ "$resize_smoke" == 1 ]]; then
  cat <<'EOF'
    system bin/"imx", "resize", "1x1", "PPM:input.ppm", "PPM:resized.ppm"
    assert_match "format=PPM width=1 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify PPM:resized.ppm")
EOF
fi

if [[ "$jpeg_smoke" == 1 ]]; then
  cat <<'EOF'
    system bin/"imx", "JPEG:output.jpg", "FARBFELD:jpeg-output.ff"
    assert_match "format=FARBFELD width=2 height=1 channels=RGBA depth=16", shell_output("#{bin/"imx"} identify FARBFELD:jpeg-output.ff")
    system bin/"imx", "JPEG:output.jpg", "JPEG:rewrite.jpg"
    assert_match "format=JPEG width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify rewrite.jpg")
EOF
fi

cat <<'EOF'
    system bin/"imx", "input.ppm", "rewrite.ppm"
    assert_match "format=PPM width=2 height=1 channels=RGB depth=8", shell_output("#{bin/"imx"} identify rewrite.ppm")
  end
end
EOF
} >"$output"
