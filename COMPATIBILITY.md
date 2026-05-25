# IMX FARBFELD/QOI/PPM/PGM Compatibility Contract

This contract covers only the standalone developer-preview slice.

## Product Boundary

- Product name: IMX Developer Preview.
- Cargo package: `imx-preview`.
- Shipped binary: `imx`.
- Runtime dependencies: none on ImageMagick, MagickCore, MagickWand, delegates,
  modules, policy.xml, or autotools.
- Oracle dependency: ImageMagick may be invoked by tests and benchmarks only.

## Supported Commands

```sh
imx identify input.ff
imx identify input.qoi
imx identify input.ppm
imx identify input.pgm
imx input.ff output.qoi
imx input.ff output.ppm
imx input.ff output.pgm
imx input.qoi output.ff
imx input.qoi output.ppm
imx input.qoi output.pgm
imx input.ppm output.ff
imx input.ppm output.qoi
imx input.ppm output.pgm
imx input.pgm output.ff
imx input.pgm output.qoi
imx input.pgm output.ppm
```

`identify` outputs:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<8|16>
```

## Format Behavior

FARBFELD:

- Magic must be exactly `farbfeld`.
- Width and height must be non-zero.
- Pixel data is RGBA16 big-endian.
- Trailing bytes after the expected payload are accepted for compatibility.

QOI:

- Magic is accepted case-insensitively for ImageMagick compatibility.
- Width and height must be non-zero.
- Channels must be `3` or `4`.
- Colorspace must be `0` or `1`.
- Missing or trailing bytes after enough pixels have decoded are accepted for
  the current compatibility slice.
- Final runs that exceed the remaining declared pixel count are clipped to the
  declared dimensions.

PPM:

- ASCII `P3` and binary `P6` RGB8 PPM are supported.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=255`; samples are scaled to 8-bit when `maxval` is
  below 255.
- High-depth PPM is intentionally out of scope.

PGM:

- ASCII `P2` and binary `P5` PGM are supported.
- Magic must be uppercase `P2` or `P5`.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=65535`.
- `P2` samples are decimal tokens and are scaled to GRAY8 when `maxval <= 255`
  or GRAY16BE when `maxval > 255`.
- `P5` consumes exactly one whitespace byte after `maxval` as the raster
  separator. Bytes after that separator are raster bytes, not comments.
- `P5` uses one byte per sample when `maxval <= 255` and two big-endian bytes
  per sample when `maxval > 255`.
- Trailing bytes after the expected PGM raster are accepted.
- IMX rejects over-max ASCII samples, zero dimensions, `maxval=0`, and
  `maxval > 65535` even when ImageMagick accepts or clamps some malformed
  inputs.

## Transcode Rules

FARBFELD to QOI:

- RGBA16BE samples are quantized to RGBA8.
- QOI output uses 4 channels and sRGB colorspace.
- This path is lossy unless each 16-bit channel is a repeated 8-bit value such
  as `0x1212` or `0xffff`.

QOI to FARBFELD:

- RGB8 expands to RGBA16BE with opaque alpha.
- RGBA8 expands to RGBA16BE preserving alpha.
- 8-bit samples expand to 16-bit samples by byte replication.

PPM to FARBFELD/QOI/PGM:

- PPM RGB8 expands to opaque RGBA when the destination has alpha.
- PPM to QOI emits RGB8 QOI.
- PPM to PGM uses Rec.709 luma: `0.212656 R + 0.715158 G + 0.072186 B`.

PGM to FARBFELD/QOI/PPM:

- Gray samples replicate into RGB channels.
- Alpha is opaque where the destination has alpha.
- PGM to QOI emits RGB8 QOI.
- PGM to PPM emits RGB8 PPM.

FARBFELD/QOI to PPM:

- Alpha is dropped.
- FARBFELD 16-bit samples are quantized to 8-bit.

FARBFELD/QOI to PGM:

- Alpha is ignored.
- RGB/RGBA channels convert to grayscale using the Rec.709 luma rule above.
- FARBFELD writes deterministic GRAY16BE `P5`; QOI writes deterministic GRAY8
  `P5`.

## Resource Policy

- Decoded pixel buffers are capped at 512 MiB.
- CLI input files larger than 513 MiB are rejected before reading.
- The cap is an IMX safety policy, not ImageMagick parity.

## Unsupported Surface

- No full ImageMagick command parser.
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No PBM/PAM/PFM support yet.
- No high-depth PPM support yet.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- No format beyond FARBFELD, QOI, PPM, and PGM.
