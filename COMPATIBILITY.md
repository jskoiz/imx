# IMX FARBFELD/QOI/PPM Compatibility Contract

This contract covers only the standalone developer-preview MVP slice.

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
imx input.ff output.qoi
imx input.qoi output.ff
imx input.ff output.ppm
imx input.qoi output.ppm
imx input.ppm output.ff
imx input.ppm output.qoi
```

`identify` outputs:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<RGB|RGBA> depth=<8|16>
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
- PBM, PGM, PAM, PFM, and high-depth PPM are intentionally out of scope for
  this MVP.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=255`; samples are scaled to 8-bit when `maxval` is
  below 255.

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

PPM to FARBFELD/QOI:

- PPM RGB8 expands to opaque RGBA when the destination has alpha.
- PPM to QOI emits RGB8 QOI.

FARBFELD/QOI to PPM:

- Alpha is dropped.
- FARBFELD 16-bit samples are quantized to 8-bit.

## Resource Policy

- Decoded pixel buffers are capped at 512 MiB.
- CLI input files larger than 513 MiB are rejected before reading.
- The cap is an IMX safety policy, not ImageMagick parity.

## Unsupported Surface

- No full ImageMagick command parser.
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No PGM/PBM/PAM support yet.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- No format beyond FARBFELD, QOI, and PPM.
