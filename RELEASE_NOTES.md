# IMX Release Notes

## IMX v0.15.0 Safe Batch Conversion Slice

- Adds one explicit safe batch command:
  `imx batch-convert --to <FORMAT> --output-dir <dir>
  [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...`.
- Supports batch conversion for the existing FARBFELD, JPEG, QOI, PBM, PGM,
  PNG, and PPM surface only, using exact uppercase target formats and existing
  exact input prefixes.
- Reuses existing decode/encode, exact resize, resize-fit, prefix validation,
  and per-output atomic write behavior. Batch outputs are deterministic
  `<input-stem>.<target-extension>` files in an existing output directory.
- Fails before writing on missing inputs, duplicate planned output names,
  existing outputs, same-path outputs, invalid prefixes, invalid dimensions,
  and unsupported formats. It does not add overwrite mode or collision
  suffixing.
- Extends CLI tests, ImageMagick differential proof, differential corpus,
  install verification, package/archive smoke, benchmark smoke, conformance
  wording, and Homebrew formula/archive smoke with batch-convert evidence.
- Keeps the slice bounded: no recursion, no glob parsing beyond shell-expanded
  input arguments, no stdin/stdout, no watch mode, no parallel execution, no
  metadata preservation, no new formats, no `magick` alias, no full ImageMagick
  CLI parsing, no Homebrew/core, no crates.io, no Windows, and no hosted
  macOS/iOS Actions.

## IMX v0.14.0 Resize-Fit Slice

- Adds one explicit aspect-preserving resize command:
  `imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>`.
- Supports resize-fit for the existing FARBFELD, JPEG, QOI, PBM, PGM, PNG, and
  PPM input/output surface only, including exact uppercase prefixes.
- Computes the largest integer output dimensions that fit within the requested
  box while preserving source aspect ratio, then uses the same center-sampled
  nearest-neighbor scaling and existing destination encoders as `imx resize`.
- Extends CLI tests, core image tests, ImageMagick differential proof,
  differential corpus, install verification, package/archive smoke, conformance
  wording, and Homebrew formula/archive smoke with resize-fit evidence.
- Keeps the slice bounded: no change to exact `imx resize`, no crop, rotate,
  fill, percentages, `WIDTHx`, `xHEIGHT`, geometry flags, metadata
  preservation, color management, stdin/stdout, `magick` alias, full
  ImageMagick CLI parsing, new formats, Homebrew/core, crates.io, Windows, or
  hosted macOS/iOS Actions.

## IMX v0.13.0 Bounded Resize Slice

- Adds one explicit resize command:
  `imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>`.
- Supports resize for the existing FARBFELD, JPEG, QOI, PBM, PGM, PNG, and PPM
  input/output surface only, including exact uppercase prefixes.
- Uses center-sampled nearest-neighbor scaling to exact dimensions and leaves
  channel/depth conversion to the existing destination encoders.
- Extends CLI tests, core image tests, ImageMagick differential proof,
  install verification, package/archive smoke, conformance wording, and
  Homebrew formula smoke generation with resize evidence.
- Keeps resize bounded: no crop, rotate, percentages, aspect-ratio shorthand,
  filters beyond nearest neighbor, metadata preservation, color management,
  stdin/stdout, `magick` alias, full ImageMagick CLI parsing, new formats,
  Homebrew/core, crates.io, Windows, or hosted macOS/iOS Actions.

## IMX v0.12.0 Real-World Intake Reliability Slice

- Keeps the v0.11.0 supported format and command surface unchanged:
  FARBFELD, JPEG, QOI, PBM, PGM, PNG, PPM identify/transcode/same-format
  rewrites with exact uppercase prefixes.
- Adds a curated intake corpus covering representative already-supported cases:
  FARBFELD RGBA16, progressive grayscale JPEG, QOI RGB linear, PBM comments,
  PGM scaled and 16-bit samples, PNG grayscale-alpha/RGBA16, and PPM high
  `maxval` with comments.
- Tightens malformed diagnostics by adding operation/path context at the CLI,
  clearer PNM over-max and PBM sample errors, uppercase FARBFELD header
  wording, and precise EOF accounting for truncated QOI multi-byte opcodes.
- Adds resource-boundary proof for the 512 MiB decoded-pixel policy and
  hardens oversized CLI input rejection with a metadata size precheck before
  the bounded read fallback.
- Extends generated fixtures, fuzz seeds, CLI tests, malformed tests, curated
  corpus tests, install verification, package/archive smoke, conformance
  output, and Homebrew formula smoke with v0.12 intake evidence.
- Keeps distribution boundaries unchanged: Linux x86_64 and Linux arm64 release
  archives through Ubuntu-only hosted automation, tap-only Homebrew
  distribution, no Homebrew/core, no crates.io, no Windows, and no hosted macOS
  or iOS GitHub Actions.

## IMX v0.11.0 Progressive JPEG Input Slice

- Adds bounded progressive JPEG input support for 8-bit grayscale and RGB JPEG
  streams. Progressive input works with `.jpg`, `.jpeg`, and exact uppercase
  `JPEG:` operands for identify and transcode.
- Carries forward v0.10.0 read-only EXIF Orientation normalization for
  progressive JPEG input; oriented progressive inputs report oriented
  dimensions and transcode normalized pixels.
- Keeps JPEG output bounded and deterministic: IMX still writes fixed quality
  90 baseline JPEG and does not preserve progressive scan layout, metadata,
  profiles, color management, or source bytes.
- Extends generated fixtures, CLI tests, codec tests, malformed/truncation
  coverage, fuzz seeds, differential corpus, install verification, package
  smoke, release-archive smoke, conformance output, and Homebrew formula smoke
  with progressive JPEG RGB/gray/orientation proof.
- Keeps distribution boundaries unchanged: Linux x86_64 and Linux arm64 release
  archives through Ubuntu-only hosted automation, tap-only Homebrew
  distribution, no Homebrew/core, no crates.io, no Windows, and no hosted macOS
  or iOS GitHub Actions.

## IMX v0.10.0 Real-Photo Reliability Slice

- Adds bounded read-only JPEG EXIF Orientation handling. Orientation values 1
  through 8 normalize decoded pixels, and `identify` reports the oriented
  dimensions.
- Keeps metadata behavior narrow: EXIF is not preserved, written, or otherwise
  interpreted beyond Orientation; ICC, XMP, density, thumbnails, timestamps,
  GPS, and camera metadata remain unsupported.
- Extends generated fixtures with deterministic JPEG Orientation 1-8 cases and
  extends the differential corpus with ImageMagick `-auto-orient` oracle
  evidence plus JPEG lossy metric rows.
- Adds focused codec, CLI, malformed, install, package, release-archive, and
  Homebrew formula smoke coverage for oriented JPEG input.
- Keeps distribution boundaries unchanged: Linux x86_64 and Linux arm64 release
  archives through Ubuntu-only hosted automation, tap-only Homebrew
  distribution, no Homebrew/core, no crates.io, no Windows, and no hosted macOS
  or iOS GitHub Actions.

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
- Documents and enforces the current Linux archive baseline: published Linux
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
  unverified macOS package is claimed. Published Linux archives require glibc
  2.34 or newer; Linux arm64 is claimed only for published archives and tap
  blocks verified from release `SHA256SUMS`.
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
