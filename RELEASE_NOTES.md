# IMX v0.5.0 Developer Preview

This preview ships a standalone Rust image-tool binary named `imx`.

## Supported Commands

```sh
imx --help
imx --version
imx identify <input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.ppm>
imx <input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.ppm> \
  <output.ff|output.farbfeld|output.qoi|output.pbm|output.pgm|output.ppm>
```

## Supported Formats

- FARBFELD RGBA16BE decode/encode.
- QOI RGB8/RGBA8 decode/encode.
- PBM ASCII `P1` and binary `P4` bilevel decode; deterministic binary `P4`
  encode.
- PGM ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.
- PPM ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6` encode.

## New In v0.5.0

- Added deterministic same-format rewrites for the existing FARBFELD, QOI,
  PBM, PGM, and PPM slice. `imx input.ppm output.ppm` and the equivalent
  same-format forms now decode and re-encode to deterministic output when the
  input and output paths are different.
- Expanded the ImageMagick oracle corpus from 20 cross-format transcodes to 25
  directed transcodes, including same-format rewrites for all supported
  formats.
- Kept the release boundary unchanged: no new image formats, no format
  prefixes, no stdin/stdout streaming, no full ImageMagick CLI, no delegates,
  no MagickCore, and no MagickWand.

## Carried From v0.4.0

- Hardened the one-command installer so it verifies release checksums, asserts
  the installed binary version, and runs a small identify/transcode smoke test.
- Carried forward release-archive smoke verification. For v0.5.0, hosted
  archive smoke is Linux-only: `x86_64-unknown-linux-gnu` and
  `aarch64-unknown-linux-gnu`.
- Homebrew remains tap-only, not Homebrew/core. The v0.5.0 tap update is a
  follow-up from the published release `SHA256SUMS`.
- Added a generated conformance report (`CONFORMANCE_REPORT.md`) sourced from
  CI evidence.
- Added a corpus differential report that identifies all supported fixture
  formats and checks directed transcodes against ImageMagick decoded pixels.
- Added scheduled cargo-fuzz with retained crash artifacts and stronger fuzz
  summary metadata.
- Added benchmark threshold summaries and a v0.4.0 baseline regression report
  that records throughput ratios and enforces RSS budgets.
- No new image formats are introduced in v0.5.0; this release is a bounded
  compatibility milestone for the existing FARBFELD/QOI/PBM/PGM/PPM slice.

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
- Same-format rewrites are deterministic decode/re-encode operations and do not
  preserve source bytes, comments, whitespace, Netpbm ASCII/binary source form,
  QOI opcode choices, or incidental representation details.
- CLI input files larger than 513 MiB are rejected before reading.
- Decoded pixel buffers larger than 512 MiB are rejected.
- Homebrew support is tap-only; no Homebrew/core, crates.io, Windows, or
  unverified macOS v0.5.0 package is claimed. Linux arm64 is claimed only for
  published v0.5.0 archives and tap blocks verified from release `SHA256SUMS`.
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
regression reports, conformance reports, and packaged Linux release archives.
Hosted macOS/iOS GitHub Actions are disabled after the v0.4.0 proof; macOS
archive or tap proof must be run locally or manually after explicit approval.
After publication, release archive smoke and Homebrew tap updates must be
verified from the published v0.5.0 `SHA256SUMS`.
