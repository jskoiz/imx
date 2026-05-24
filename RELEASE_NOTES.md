# IMX Developer Preview MVP

This preview ships a standalone Rust image-tool binary named `imx`.

## Supported Commands

```sh
imx --help
imx --version
imx identify input.ff
imx identify input.qoi
imx identify input.ppm
imx input.ff output.qoi
imx input.ff output.ppm
imx input.qoi output.ff
imx input.qoi output.ppm
imx input.ppm output.ff
imx input.ppm output.qoi
```

## Supported Formats

- FARBFELD RGBA16BE decode/encode
- QOI RGB8/RGBA8 decode/encode
- PPM ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6` encode

## Known Limits

- PPM support is intentionally limited to RGB8 `P3`/`P6` with `maxval <= 255`.
- FARBFELD to QOI/PPM is lossy because 16-bit samples are quantized to 8-bit.
- QOI compatibility accepts case-insensitive magic and missing end markers after
  enough pixels decode.
- CLI input files larger than 513 MiB are rejected before reading.
- Decoded pixel buffers larger than 512 MiB are rejected.
- This is not a full ImageMagick CLI, MagickCore, or MagickWand replacement.

## Release Evidence

Use:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
./scripts/package-release.sh
```

The GitHub Actions preview workflow uploads generated fixtures, benchmark
evidence, and packaged release archives.
