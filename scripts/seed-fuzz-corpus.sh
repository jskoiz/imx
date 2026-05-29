#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

corpus_root="${IMX_FUZZ_CORPUS:-$root/fuzz/corpus}"
generated_dir="$root/target/fuzz-seed-fixtures"
rm -rf "$generated_dir"
mkdir -p "$corpus_root/farbfeld_decode" "$corpus_root/qoi_decode" "$corpus_root/pnm_decode" "$corpus_root/png_decode" "$corpus_root/bmp_decode" "$corpus_root/jpeg_decode" "$corpus_root/webp_decode" "$corpus_root/gif_decode"

cargo run -p imx-cli --bin imx-generate-fixtures -- "$generated_dir" >/dev/null

cp "$generated_dir/gradient-64.ff" "$corpus_root/farbfeld_decode/gradient-64.ff"
cp "$generated_dir/quantization-2x2.ff" "$corpus_root/farbfeld_decode/quantization-2x2.ff"
cp "$generated_dir/intake-farbfeld-rgba16-2x2.ff" "$corpus_root/farbfeld_decode/intake-farbfeld-rgba16-2x2.ff"
cp "$generated_dir/gradient-64.qoi" "$corpus_root/qoi_decode/gradient-64.qoi"
cp "$generated_dir/qoi-rgba-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgba-2x2.qoi"
cp "$generated_dir/qoi-rgb-2x2.qoi" "$corpus_root/qoi_decode/qoi-rgb-2x2.qoi"
cp "$generated_dir/intake-qoi-rgb-linear-2x2.qoi" "$corpus_root/qoi_decode/intake-qoi-rgb-linear-2x2.qoi"
cp "$generated_dir/gradient-64.png" "$corpus_root/png_decode/gradient-64.png"
cp "$generated_dir/gradient-64-png16.png" "$corpus_root/png_decode/gradient-64-png16.png"
cp "$generated_dir/intake-rgba16-1x1.png" "$corpus_root/png_decode/intake-rgba16-1x1.png"
cp "$generated_dir/intake-gray8-3x1.png" "$corpus_root/png_decode/intake-gray8-3x1.png"
cp "$generated_dir/intake-rgb16-2x1.png" "$corpus_root/png_decode/intake-rgb16-2x1.png"
cp "$generated_dir/gradient-64.bmp" "$corpus_root/bmp_decode/gradient-64.bmp"
cp "$generated_dir/intake-rgb24-3x2.bmp" "$corpus_root/bmp_decode/intake-rgb24-3x2.bmp"
cp "$generated_dir/intake-rgba32-2x2.bmp" "$corpus_root/bmp_decode/intake-rgba32-2x2.bmp"
cp "$generated_dir/intake-top-down-rgb24-3x2.bmp" "$corpus_root/bmp_decode/intake-top-down-rgb24-3x2.bmp"
cp "$generated_dir/intake-top-down-rgba32-2x2.bmp" "$corpus_root/bmp_decode/intake-top-down-rgba32-2x2.bmp"
cp "$generated_dir/gradient-64.jpg" "$corpus_root/jpeg_decode/gradient-64.jpg"
cp "$generated_dir/gray-4x1.jpg" "$corpus_root/jpeg_decode/gray-4x1.jpg"
cp "$generated_dir/progressive-rgb-4x3.jpg" "$corpus_root/jpeg_decode/progressive-rgb-4x3.jpg"
cp "$generated_dir/progressive-gray-4x2.jpg" "$corpus_root/jpeg_decode/progressive-gray-4x2.jpg"
cp "$generated_dir/progressive-orientation-o6.jpg" "$corpus_root/jpeg_decode/progressive-orientation-o6.jpg"
cp "$generated_dir/jpeg-camera-exif-le-o6.jpg" "$corpus_root/jpeg_decode/jpeg-camera-exif-le-o6.jpg"
cp "$generated_dir/progressive-camera-exif-le-o6.jpg" "$corpus_root/jpeg_decode/progressive-camera-exif-le-o6.jpg"
for orientation in 1 2 3 4 5 6 7 8; do
  cp "$generated_dir/photo-orientation-o$orientation.jpg" "$corpus_root/jpeg_decode/photo-orientation-o$orientation.jpg"
done
cp "$generated_dir/gradient-64.ppm" "$corpus_root/pnm_decode/gradient-64.ppm"
cp "$generated_dir/gradient-64.pbm" "$corpus_root/pnm_decode/gradient-64.pbm"
cp "$generated_dir/gradient-64.pgm" "$corpus_root/pnm_decode/gradient-64.pgm"
cp "$generated_dir/intake-comments-2x1.ppm" "$corpus_root/pnm_decode/intake-comments-2x1.ppm"
cp "$generated_dir/intake-ppm-binary-comments-crlf-2x1.ppm" "$corpus_root/pnm_decode/intake-ppm-binary-comments-crlf-2x1.ppm"
cp "$generated_dir/intake-pgm16-2x1.pgm" "$corpus_root/pnm_decode/intake-pgm16-2x1.pgm"
cp "$generated_dir/intake-pgm-binary-comments-crlf-3x1.pgm" "$corpus_root/pnm_decode/intake-pgm-binary-comments-crlf-3x1.pgm"
cp "$generated_dir/intake-webp-rgb-2x1.webp" "$corpus_root/webp_decode/intake-webp-rgb-2x1.webp"
cp "$generated_dir/intake-webp-rgba-2x1.webp" "$corpus_root/webp_decode/intake-webp-rgba-2x1.webp"
cp "$generated_dir/intake-gif-rgba-2x1.gif" "$corpus_root/gif_decode/intake-gif-rgba-2x1.gif"

printf 'farbfeld' >"$corpus_root/farbfeld_decode/header-only.ff"
printf 'qoif\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00' >"$corpus_root/qoi_decode/header-only.qoi"
printf 'qoif\x00\x00\x00\x02\x00\x00\x00\x01\x03\x00\xfd\x00\x00\x00\x00\x00\x00\x00\x01' >"$corpus_root/qoi_decode/final-run-clips-to-image.qoi"
printf 'qoif\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00\xfe\x01\x02\x03\x00\x00\x00\x00\x00\x00\x00\x01TRAIL' >"$corpus_root/qoi_decode/trailing-bytes-after-image.qoi"
printf '\x89PNG\r\n\x1a\n' >"$corpus_root/png_decode/signature-only.png"
printf 'BM' >"$corpus_root/bmp_decode/magic-only.bmp"
printf 'BM\x36\x00\x00\x00\x00\x00\x00\x00\x36\x00\x00\x00\x28\x00\x00\x00\x01\x00\x00\x00\x01\x00\x00\x00\x01\x00\x18\x00\x01\x00\x00\x00' >"$corpus_root/bmp_decode/compressed-truncated.bmp"
printf 'BM\x36\x00\x00\x00\x00\x00\x00\x00\x36\x00\x00\x00\x28\x00\x00\x00\x01\x00\x00\x00\x01\x00\x00\x00\x01\x00\x08\x00\x00\x00\x00\x00' >"$corpus_root/bmp_decode/indexed-truncated.bmp"
printf 'RIFF' >"$corpus_root/webp_decode/riff-only.webp"
printf 'RIFF\x0c\x00\x00\x00WEBP' >"$corpus_root/webp_decode/header-only.webp"
printf 'RIFX\x00\x00\x00\x00WEBP' >"$corpus_root/webp_decode/bad-riff-magic.webp"
printf 'RIFF\x00\x00\x00\x00PNG ' >"$corpus_root/webp_decode/bad-webp-magic.webp"
printf 'GIF' >"$corpus_root/gif_decode/magic-truncated.gif"
printf 'GIF89a' >"$corpus_root/gif_decode/header-only.gif"
printf 'GIF89a\x02\x00\x01\x00\x00\x00\x00' >"$corpus_root/gif_decode/no-image-block.gif"
printf 'NOTGIF' >"$corpus_root/gif_decode/bad-magic.gif"
printf '\xff\xd8' >"$corpus_root/jpeg_decode/soi-only.jpg"
printf '\xff\xd8\xff\xd9' >"$corpus_root/jpeg_decode/soi-eoi-only.jpg"
printf '\xff\xd8\xff\xe0\x00\x01' >"$corpus_root/jpeg_decode/bad-app0-length.jpg"
printf '\xff\xd8\xff\xe1\x00\x10Exif\x00\x00ZZ\x00\x2a\x00\x00\x00\x08\xff\xd9' >"$corpus_root/jpeg_decode/bad-exif-endian.jpg"
printf '\xff\xd8\xff\xe1\x00\x20Exif\x00\x00MM\x00\x2a\x00\x00\x00\x08\x00\x01\x01\x12\x00\x03\x00\x00\x00\x01\x00\x09\x00\x00\x00\x00\x00\x00\xff\xd9' >"$corpus_root/jpeg_decode/bad-exif-orientation.jpg"
printf '\xff\xd8\xff\xc0\x00\x11\x08\x00\x00\x00\x01\x03\x01\x11\x00\x02\x11\x01\x03\x11\x01\xff\xd9' >"$corpus_root/jpeg_decode/zero-width-sof.jpg"
printf '\xff\xd8\xff\xc0\x00\x11\x08\xff\xff\xff\xff\x03\x01\x11\x00\x02\x11\x01\x03\x11\x01\xff\xd9' >"$corpus_root/jpeg_decode/oversized-sof.jpg"
printf 'P3\n# comment\n2 1\n31\n0 15 31 31 0 15\n' >"$corpus_root/pnm_decode/ascii-ppm-max31.ppm"
printf 'P6\n2 1\n255\n\xff\x00\x00\x00\x80\xff' >"$corpus_root/pnm_decode/binary-ppm-2x1.ppm"
printf 'P3\n2 1\n1023\n0 512 1023 1023 256 128\n' >"$corpus_root/pnm_decode/ascii-ppm-max1023.ppm"
printf 'P6\n2 1\n65535\n\x00\x00\x80\x00\xff\xff\xff\xff\x40\x00\x20\x00' >"$corpus_root/pnm_decode/binary-ppm-max65535.ppm"
printf 'P6\n1 1\n256\n\x00\x00\x01' >"$corpus_root/pnm_decode/binary-ppm-16bit-truncated.ppm"
printf 'P3\n1 1\n256\n0 257 1\n' >"$corpus_root/pnm_decode/ascii-ppm-sample-over-max.ppm"
printf 'P6\n1 1\n256\n\x00\x00\x01\x01\x00\x00' >"$corpus_root/pnm_decode/binary-ppm-sample-over-max.ppm"
printf 'P3\n1 1\n65536\n0 0 0\n' >"$corpus_root/pnm_decode/ascii-ppm-maxval-65536.ppm"
printf 'P6\n100000 100000\n65535\n' >"$corpus_root/pnm_decode/binary-ppm-16bit-huge-dims.ppm"
printf 'P1\n# checker\n2 2\n0 1\n1 0\n' >"$corpus_root/pnm_decode/pbm-p1-2x2.pbm"
printf 'P4\n2 2\n\x80\x40' >"$corpus_root/pnm_decode/pbm-p4-2x2.pbm"
printf 'P1\n2 2\n0 1 1\n' >"$corpus_root/pnm_decode/pbm-p1-truncated-raster.pbm"
printf 'P4\n1 1\n' >"$corpus_root/pnm_decode/pbm-p4-header-only.pbm"
printf 'P4\n9 2\n\x80\x00\x80' >"$corpus_root/pnm_decode/pbm-p4-short-second-row.pbm"
printf 'P4\n9 1\n\x80\x7f' >"$corpus_root/pnm_decode/pbm-p4-nonzero-padding-bits.pbm"
printf 'P1\n# header comment\n3 1\n0 # raster comment\n1 0\n' >"$corpus_root/pnm_decode/pbm-p1-comments.pbm"
printf 'P4\n# binary header comment\n9 1\n\x80\x00' >"$corpus_root/pnm_decode/pbm-p4-comments.pbm"
printf 'P1\n100000 100000\n0\n' >"$corpus_root/pnm_decode/pbm-p1-huge-dims.pbm"
printf 'P4\n100000 100000\n' >"$corpus_root/pnm_decode/pbm-p4-huge-dims.pbm"
printf 'P1\n2 1\n0 2\n' >"$corpus_root/pnm_decode/pbm-p1-invalid-sample-2.pbm"
printf 'P1\n2 1\n0 x\n' >"$corpus_root/pnm_decode/pbm-p1-invalid-sample-token.pbm"
printf 'P4\nx 1\n\x80' >"$corpus_root/pnm_decode/pbm-p4-invalid-width-token.pbm"
printf 'P1\n1 1\n255\n' >"$corpus_root/pnm_decode/pbm-p1-unexpected-maxval.pbm"
printf 'P4\n1 1\n255\n\x80' >"$corpus_root/pnm_decode/pbm-p4-maxval-looking-raster.pbm"
printf 'P1\n1 1\n01\n' >"$corpus_root/pnm_decode/pbm-p1-adjacent-samples.pbm"
printf 'P4\n1 1\x80' >"$corpus_root/pnm_decode/pbm-p4-missing-raster-separator.pbm"
printf 'P1\n1 1\n0 1\n' >"$corpus_root/pnm_decode/pbm-p1-trailing-token.pbm"
printf 'P4\n1 1\n\x80TRAIL' >"$corpus_root/pnm_decode/pbm-p4-trailing-bytes.pbm"
printf 'P1\n4294967296 1\n0\n' >"$corpus_root/pnm_decode/pbm-p1-dim-u32-overflow.pbm"
printf 'P4\n4294967295 4294967295\n' >"$corpus_root/pnm_decode/pbm-p4-dim-product-overflow.pbm"
printf 'P2\n# gray ramp\n2 2\n15\n0 7 15 3\n' >"$corpus_root/pnm_decode/pgm-p2-2x2-max15.pgm"
printf 'P5\n2 2\n255\n\x00\x7f\x80\xff' >"$corpus_root/pnm_decode/pgm-p5-2x2-max255.pgm"
printf 'P2\n1 3\n65535\n0 32768 65535\n' >"$corpus_root/pnm_decode/pgm-p2-1x3-max65535.pgm"
printf 'P5\n2 2\n65535\n\x00\x00\x7f\xff\x80\x00\xff\xff' >"$corpus_root/pnm_decode/pgm-p5-2x2-max65535.pgm"
printf 'P5\n1 1\n255\n' >"$corpus_root/pnm_decode/pgm-p5-header-only.pgm"
printf 'P5\n1 1\n65535\n\x12' >"$corpus_root/pnm_decode/pgm-p5-16bit-half-sample.pgm"
printf 'P2\n1 1\n10\n11\n' >"$corpus_root/pnm_decode/pgm-p2-sample-over-max.pgm"
printf 'P2\n1 1\n0\n0\n' >"$corpus_root/pnm_decode/pgm-p2-maxval-zero.pgm"
printf 'P2\n1 1\n65536\n0\n' >"$corpus_root/pnm_decode/pgm-p2-maxval-65536.pgm"
printf 'P5\n1 1\n255X' >"$corpus_root/pnm_decode/pgm-p5-missing-raster-separator.pgm"
printf 'P2\n# unterminated comment' >"$corpus_root/pnm_decode/pgm-p2-comment-eof.pgm"
printf 'P5\n100000 100000\n255\n' >"$corpus_root/pnm_decode/pgm-p5-huge-dims.pgm"

python3 - "$generated_dir" "$corpus_root" <<'PY'
import struct
import sys
import zlib
from pathlib import Path

generated = Path(sys.argv[1])
corpus = Path(sys.argv[2])
png_dir = corpus / "png_decode"
bmp_dir = corpus / "bmp_decode"


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def png(width, height, bit_depth, color_type, payload, extra=()):
    ihdr = struct.pack(">IIBBBBB", width, height, bit_depth, color_type, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + b"".join(extra)
        + chunk(b"IDAT", zlib.compress(payload))
        + chunk(b"IEND", b"")
    )


(png_dir / "indexed-valid-container.png").write_bytes(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 3, 0, 0, 0))
    + chunk(b"PLTE", b"\x00\x00\x00\xff\xff\xff")
    + chunk(b"IDAT", zlib.compress(b"\x00\x00"))
    + chunk(b"IEND", b"")
)
(png_dir / "low-bit-gray-valid-container.png").write_bytes(png(1, 1, 1, 0, b"\x00\x80"))
(png_dir / "interlaced-valid-container.png").write_bytes(
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 1))
    + chunk(b"IDAT", zlib.compress(b"\x00\xff\x00\x00"))
    + chunk(b"IEND", b"")
)
(png_dir / "trns-valid-container.png").write_bytes(
    png(1, 1, 8, 0, b"\x00\x00", [chunk(b"tRNS", b"\x00\x00")])
)
(png_dir / "apng-valid-container.png").write_bytes(
    png(1, 1, 8, 2, b"\x00\xff\x00\x00", [chunk(b"acTL", struct.pack(">II", 1, 0))])
)
(png_dir / "huge-ihdr-valid-crc.png").write_bytes(png(100000, 100000, 8, 6, b""))

bad_crc = bytearray((generated / "gradient-64.png").read_bytes())
bad_crc[32] ^= 0xFF
(png_dir / "bad-crc.png").write_bytes(bad_crc)

bmp = bytearray((generated / "intake-rgb24-3x2.bmp").read_bytes())
bmp[2:6] = struct.pack("<I", 40)
(bmp_dir / "declared-file-size-too-small.bmp").write_bytes(bmp)
PY

echo "$corpus_root"
