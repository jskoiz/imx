# IMX Release Notes

## IMX v0.9.0 JPEG Usability Slice

- Adds bounded Rust-native JPEG support for `.jpg` and `.jpeg` files plus exact
  uppercase `JPEG:` prefix handling. `JPG:` remains unsupported by design.
- Supports JPEG identify/decode/encode for 8-bit grayscale and RGB streams.
  `identify` reports `format=JPEG ... channels=GRAY|RGB depth=8`.
- Encodes JPEG deterministically at fixed quality 90 from normalized IMX
  grayscale/RGB pixels. Non-opaque alpha inputs are rejected instead of silently
  composited or dropped.
- Extends CLI, codec, golden, malformed, fuzz-smoke, cargo-fuzz,
  differential-corpus, benchmark, install, package, archive-smoke, conformance,
  and tap evidence to include JPEG.
- Adds JPEG lossy oracle evidence with recorded RGB8 tolerance metrics rather
  than byte-equality claims.
- Applies a conservative 128 MiB JPEG decoded-pixel cap before decode
  allocation to keep fuzz/resource-safety proof inside the existing 512 MiB
  general image-buffer policy.
- Keeps JPEG bounded: no progressive JPEG, CMYK/YCCK JPEG, 12-bit JPEG,
  arithmetic-coded JPEG, lossless JPEG/JPEG-LS, JPEG 2000, JPEG XL, metadata
  preservation, profiles, color management, orientation handling, or full
  ImageMagick JPEG behavior.
- Keeps distribution boundaries unchanged: Linux x86_64 and Linux arm64 release
  archives through Ubuntu-only hosted automation, generated `SHA256SUMS`,
  `imx.rb`, and conformance report assets, tap-only Homebrew distribution, no
  crates.io, no Homebrew/core, no Windows, and no hosted macOS or iOS GitHub
  Actions.

## IMX v0.8.1 Release Hardening

- Tightens PNG diagnostics so malformed PNG identify paths report identify
  failures and PNG decoder limit failures surface as image-size limit failures.
- Adds regression coverage for grayscale-alpha PNG identify/decode/transcode and
  for rejected PNG non-goals: interlacing, APNG, `tRNS`, and oversized rasters.
- Updates generated conformance wording to include high-depth PNG evidence and
  to distinguish pre-publish package smoke from post-publish archive smoke.
- Documents and enforces the current Linux archive baseline: published v0.8.x
  binary archives require glibc 2.34 or newer.
- Hardens Ubuntu-only CI and tap proof with broader workflow path coverage,
  explicit timeouts/concurrency, release tag/version validation, checked-out
  tap formula install tests, and no-macOS tap formula guards.

## IMX v0.8.0 PNG Raster Slice

- Adds Rust-native PNG support for a bounded raster surface: static
  non-interlaced grayscale, RGB, RGBA, and grayscale-alpha PNG inputs with 8-bit
  or 16-bit samples.
- Adds `.png` detection by PNG signature before extension fallback, exact
  uppercase `PNG:` prefix handling, PNG identify, PNG transcodes to/from the
  existing FARBFELD/QOI/PBM/PGM/PPM formats, and deterministic PNG same-format
  rewrites.
- Encodes PNG deterministically from the normalized IMX raster model. Source
  compression/filter choices, ancillary chunks, comments, timestamps, gamma,
  profiles, EXIF, and other metadata are not preserved.
- Extends CLI, codec, golden, malformed, fuzz-smoke, cargo-fuzz, benchmark,
  install, package, archive-smoke, differential-corpus, and conformance evidence
  to include PNG.
- Keeps PNG bounded: no APNG, indexed/palette PNG, low-bit PNG, `tRNS`
  color-key transparency, PNG color management/profile semantics, or full
  ImageMagick PNG behavior.
- Carries forward the v0.7.0 high-depth PPM behavior and the v0.6.0 exact
  uppercase prefix behavior for FARBFELD, QOI, PBM, PGM, PNG, and PPM.
- Keeps distribution boundaries unchanged: Linux x86_64 and Linux arm64 release
  archives through Ubuntu-only hosted automation, generated `SHA256SUMS`,
  `imx.rb`, and conformance report assets, tap-only Homebrew distribution, no
  crates.io, no Homebrew/core, no Windows, and no hosted macOS or iOS GitHub
  Actions.

## IMX v0.7.0 High-Depth PPM

- Adds high-depth PPM support for the existing PPM codec only: uppercase `P3`
  and `P6` with `maxval` 256..65535 identify and decode as RGB16BE.
- Carries forward the v0.6.0 exact uppercase `FARBFELD:`, `QOI:`, `PBM:`,
  `PGM:`, and `PPM:` prefix behavior for identify, transcode, and same-format
  rewrite paths.
- Adds a core RGB16BE image representation so PPM16 can preserve precision when
  transcoding to FARBFELD, PGM16, or same-format PPM output.
- Encodes PPM deterministically as binary `P6`: `maxval 255` for 8-bit sources
  and `maxval 65535` for 16-bit RGB/RGBA/GRAY sources.
- Extends CLI, codec, golden, malformed, fuzz-smoke, benchmark, install,
  package, archive-smoke, differential-corpus, and conformance evidence to cover
  the PPM16 slice.
- Keeps the boundary unchanged: no PNG/JPEG/TIFF/PAM/PFM/BMP, no stdin/stdout,
  no `magick` alias, no full ImageMagick CLI, no delegates, no MagickCore, no
  MagickWand, no crates.io, no Homebrew/core, no Windows, and no hosted macOS or
  iOS GitHub Actions.
- Publishes Linux x86_64 and Linux arm64 release archives through Ubuntu-only
  hosted automation, plus generated `SHA256SUMS`, `imx.rb`, and conformance
  report assets. The Homebrew tap update is generated from the published v0.7.0
  `SHA256SUMS` and remains tap-only, not Homebrew/core.

## IMX v0.6.0 Prefix Compatibility

- Adds exact uppercase `FARBFELD:`, `QOI:`, `PBM:`, `PGM:`, and `PPM:`
  prefixes for the existing identify and two-path transcode operands.
- Prefixes are stripped before file IO and must match detected input format or
  output path extension. Unknown prefixes, missing prefixed paths, mismatched
  prefixes, and prefixed same-path writes fail with `error: ...`.
- Extends the ImageMagick oracle corpus with prefixed identify cases and a
  prefixed transcode ring across the existing FARBFELD/QOI/PBM/PGM/PPM surface.
- Carries forward the v0.5.0 deterministic identify, cross-format transcode,
  and same-format rewrite surface for FARBFELD, QOI, PBM, PGM, and PPM.
- Publishes Linux x86_64 and Linux arm64 release archives through Ubuntu-only
  hosted automation, plus generated `SHA256SUMS`, `imx.rb`, and conformance
  report assets.
- The Homebrew tap update is generated from the published v0.6.0 `SHA256SUMS`
  and remains tap-only, not Homebrew/core.
- Keeps the boundary unchanged: no new image formats, no stdin/stdout
  streaming, no full ImageMagick CLI, no delegates, no MagickCore, and no
  MagickWand.

## IMX v0.5.0 Developer Preview

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
- v0.5.0/v0.6.0 PPM support was limited to RGB8 `P3`/`P6` with
  `maxval <= 255`; v0.7.0 extends this to RGB16BE PPM for `maxval` 256..65535.
- v0.8.0 PNG support is limited to static non-interlaced grayscale/RGB/RGBA and
  grayscale-alpha 8/16-bit rasters. It rejects APNG, indexed/palette PNG,
  low-bit PNG, `tRNS`, and PNG color-management/profile semantics.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are intentionally out of scope.
- P2 input is re-encoded as deterministic binary P5 output; source form,
  comments, and whitespace are not preserved.
- FARBFELD to QOI is lossy because 16-bit samples are quantized to 8-bit.
- QOI compatibility accepts case-insensitive magic and missing end markers after
  enough pixels decode.
- Same-format rewrites are deterministic decode/re-encode operations and do not
  preserve source bytes, comments, whitespace, Netpbm ASCII/binary source form,
  QOI opcode choices, or incidental representation details.
- CLI input files larger than 513 MiB are rejected before reading.
- Decoded pixel buffers larger than 512 MiB are rejected.
- Homebrew support is tap-only; no Homebrew/core, crates.io, Windows, or
  unverified macOS v0.8.x package is claimed. Published Linux v0.8.x archives
  require glibc 2.34 or newer; Linux arm64 is claimed only for published
  archives and tap blocks verified from release `SHA256SUMS`.
- This is not a full ImageMagick CLI, MagickCore, or MagickWand replacement.

## Release Evidence

Use:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
IMX_FUZZ_MAX_TOTAL_TIME=5 ./scripts/run-fuzz.sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_BENCH_BASE_REF=v0.5.0 ./scripts/bench-regression.sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
./scripts/package-release.sh
```

The GitHub Actions preview workflow uploads generated fixtures, fuzz results,
fresh-install evidence, corpus differentials, benchmark evidence, benchmark
regression reports, conformance reports, and packaged Linux release archives.
Hosted macOS/iOS GitHub Actions are disabled after the v0.4.0 proof; macOS
archive or tap proof must be run locally or manually after explicit approval.
After publication, release archive smoke and Homebrew tap updates must be
verified from the published release `SHA256SUMS`.
