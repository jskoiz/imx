# IMX v0.3.0 Developer Preview

This preview ships a standalone Rust image-tool binary named `imx`.

## Supported Commands

```sh
imx --help
imx --version
imx identify <input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.ppm>
imx <input.ff|input.farbfeld> <output.qoi|output.pbm|output.pgm|output.ppm>
imx input.qoi <output.ff|output.farbfeld|output.pbm|output.pgm|output.ppm>
imx input.pbm <output.ff|output.farbfeld|output.qoi|output.pgm|output.ppm>
imx input.pgm <output.ff|output.farbfeld|output.qoi|output.pbm|output.ppm>
imx input.ppm <output.ff|output.farbfeld|output.qoi|output.pbm|output.pgm>
```

## Supported Formats

- FARBFELD RGBA16BE decode/encode.
- QOI RGB8/RGBA8 decode/encode.
- PBM ASCII `P1` and binary `P4` bilevel decode; deterministic binary `P4`
  encode.
- PGM ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.
- PPM ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6` encode.

## New In v0.3.0

- Added PBM as the final baseline Netpbm format for this preview slice.
- Added `P1` and `P4` identify/decode plus deterministic `P4` encode.
- Added logical bilevel pixel-buffer handling with one byte per pixel in safe
  Rust core memory.
- Added PBM transcodes across FARBFELD, QOI, PGM, and PPM.
- Added ImageMagick differential tests for PBM identify, P1/P4 decode, PBM
  transcodes, and FARBFELD-to-PBM threshold behavior.
- Added PBM seeds to the PNM fuzz target and PBM metrics to benchmark/RSS
  evidence.
- Added a one-command release installer and tag-only native packaging for Linux
  and macOS release archives.

## Known Limits

- PBM input source form is not preserved; `P1` input re-encodes as binary `P4`.
- PBM comments, whitespace, and padding-bit values are not preserved.
- PBM output is lossy thresholding from gray/color inputs.
- PPM support is intentionally limited to RGB8 `P3`/`P6` with `maxval <= 255`.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are intentionally out of scope.
- P2 input is re-encoded as deterministic binary P5 output; source form,
  comments, and whitespace are not preserved.
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
IMX_FUZZ_MAX_TOTAL_TIME=5 ./scripts/run-fuzz.sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
./scripts/package-release.sh
```

The GitHub Actions preview workflow uploads generated fixtures, fuzz results,
fresh-install evidence, benchmark evidence, and packaged release archives.
Tagged releases publish native Linux and macOS archives automatically.
