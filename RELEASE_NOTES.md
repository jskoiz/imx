# IMX v0.4.0 Developer Preview

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

## New In v0.4.0

- Hardened the one-command installer so it verifies release checksums, asserts
  the installed binary version, and runs a small identify/transcode smoke test.
- Added published release-archive smoke verification for Linux x86_64, macOS
  arm64, and macOS x86_64 after GitHub release publication.
- Added a generated Homebrew formula draft (`imx.rb`) based on the aggregate
  release `SHA256SUMS`.
- Added a generated conformance report (`CONFORMANCE_REPORT.md`) sourced from
  CI evidence.
- Added a corpus differential report that identifies all supported fixture
  formats and checks all 20 directed cross-format transcodes against
  ImageMagick decoded pixels.
- Added scheduled cargo-fuzz with retained crash artifacts and stronger fuzz
  summary metadata.
- Added benchmark threshold summaries and a v0.3.0 baseline regression report
  that records throughput ratios and enforces RSS budgets.
- No new image formats are introduced in v0.4.0; this release is a public
  install and trust milestone for the existing FARBFELD/QOI/PBM/PGM/PPM slice.

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
fresh-install evidence, corpus differentials, benchmark evidence, benchmark
regression reports, conformance reports, and packaged release archives. Tagged
releases publish native Linux and macOS archives automatically, then download
the published assets back for checksum, no-link, identify, and transcode smoke
verification.
