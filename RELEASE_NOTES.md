# IMX v0.2.0 Developer Preview

This preview ships a standalone Rust image-tool binary named `imx`.

## Supported Commands

```sh
imx --help
imx --version
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

## Supported Formats

- FARBFELD RGBA16BE decode/encode.
- QOI RGB8/RGBA8 decode/encode.
- PPM ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6` encode.
- PGM ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.

## New In v0.2.0

- Added production-grade PGM as the first PNM expansion.
- Renamed the shared Netpbm codec crate to `imx-codec-pnm`.
- Added explicit `GRAY` pixel-buffer handling and Rec.709 color-to-gray
  conversion.
- Added ImageMagick differential tests for PGM identify, P2/P5 decode, 16-bit
  PGM, PGM transcodes, and FARBFELD-to-PGM luma behavior.
- Added PGM seeds to the PNM fuzz target and PGM metrics to benchmark/RSS
  evidence.

## Known Limits

- PPM support is intentionally limited to RGB8 `P3`/`P6` with `maxval <= 255`.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are intentionally out of scope.
- P2 input is re-encoded as deterministic binary P5 output; source form,
  comments, and whitespace are not preserved.
- FARBFELD to QOI/PPM is lossy because 16-bit samples are quantized to 8-bit.
- Color to PGM uses Rec.709 luma and ignores alpha.
- QOI compatibility accepts case-insensitive magic and missing end markers after
  enough pixels decode.
- CLI input files larger than 513 MiB are rejected before reading.
- Decoded pixel buffers larger than 512 MiB are rejected.
- This is not a full ImageMagick CLI, MagickCore, or MagickWand replacement.

## Release Evidence

Use:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
IMX_FUZZ_MAX_TOTAL_TIME=5 ./scripts/run-fuzz.sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
./scripts/package-release.sh
```

The GitHub Actions preview workflow uploads generated fixtures, fuzz results,
fresh-install evidence, benchmark evidence, and packaged release archives.
Tagged releases publish packaged archives automatically.

Release packaging uses deterministic tar/gzip metadata and relative checksums,
so CI can compare repeated packages from the same target payload.
