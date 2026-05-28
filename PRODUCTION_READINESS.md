# IMX Compatibility Readiness

Status: v0.18.0 is the current developer-preview release surface. It carries
forward the v0.6.0 exact format-prefix surface, the v0.7.0 high-depth PPM
surface, the v0.8.0 bounded PNG raster slice, and the v0.9.0 bounded JPEG
slice, plus v0.10.0 bounded JPEG EXIF Orientation normalization and v0.11.0
bounded progressive JPEG input support, plus v0.12.0 representative generated
and in-test intake reliability coverage for already-supported formats. The
v0.13.0 release adds
bounded nearest-neighbor exact resize for the same supported formats, and
v0.14.0 adds aspect-preserving resize-fit. v0.15.0 adds safe batch conversion,
and v0.16.0 adds bounded uncompressed BMP support on top of the existing
identify/transcode/resize/resize-fit/batch-convert surface. v0.17.0 adds an
offline installed-binary self-test and hardens CLI diagnostic/exit-code proof.
v0.18.0 adds deterministic JSON identify/report output for the same existing
identify fields plus report status and diagnostic codes.
GitHub release archive support is claimed only for the Linux archive targets
present in the published v0.18.0 GitHub `SHA256SUMS` and verified by
Linux-only release/archive smoke. Tap support is verified through the
`jskoiz/homebrew-imx` update generated from those `SHA256SUMS` and proven by
Linux-only tap smoke. Published Linux archives require glibc 2.34 or newer,
with release/archive smoke checking that published Linux binaries do not
reference `GLIBC_*` symbols newer than `GLIBC_2.34`. Hosted release proof is
Linux-only.
Automatic hosted macOS/iOS GitHub Actions remain disabled; macOS proof is
local/manual only unless explicitly approved in the current turn.

## Implemented Behavior

- Source of truth: `https://github.com/jskoiz/imx`.
- Product binary: `imx`.
- The current surface carries forward deterministic same-format rewrites for the
  BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM slice when input and output paths are
  different. JPEG rewrites are deterministic lossy decode/re-encode operations.
- v0.6.0 added exact uppercase `FARBFELD:`, `QOI:`, `PBM:`, `PGM:`, and `PPM:`
  prefixes for existing identify/transcode operands only. Prefixes confirm
  detected input or output path format; they do not add new formats,
  extensionless output selection, stdin/stdout, or full ImageMagick CLI
  parsing.
- v0.7.0 added PPM `maxval` 256..65535 support for uppercase `P3` and `P6` PPM
  only. High-depth PPM identifies as `channels=RGB depth=16`, decodes as RGB16BE,
  preserves 16-bit samples when writing FARBFELD/PGM16/PPM16, and quantizes only
  for inherently 8-bit or bilevel destinations.
- v0.8.0 adds static non-interlaced PNG identify/decode/encode for grayscale,
  RGB, RGBA, and grayscale-alpha PNG with 8-bit or 16-bit samples. It rejects
  APNG, interlace, indexed/palette PNG, low-bit PNG, `tRNS`, and PNG
  metadata/profile/color-management semantics.
- v0.9.0 adds `.jpg`/`.jpeg` and exact `JPEG:` identify/transcode support for
  8-bit grayscale/RGB JPEG. JPEG output uses fixed quality 90, rejects
  non-opaque alpha, and does not preserve metadata, profiles, chroma
  subsampling, quantization tables, scan layout, or source bytes.
- v0.10.0 adds read-only EXIF Orientation support for JPEG input. Orientation
  values 1 through 8 normalize decoded pixels, and `identify` reports oriented
  dimensions. Other EXIF, ICC, XMP, GPS, thumbnail, density, and camera metadata
  remains unpreserved and uninterpreted.
- v0.11.0 adds 8-bit progressive grayscale/RGB JPEG input support. Progressive
  inputs work through `.jpg`, `.jpeg`, and exact `JPEG:` identify/transcode
  forms, carry forward EXIF Orientation normalization, and still re-encode JPEG
  output as deterministic baseline quality-90 JPEG.
- v0.12.0 adds a bounded representative intake reliability slice without
  adding format breadth. It proves generated/in-test corpus cases for comments,
  high-max Netpbm input, grayscale-alpha/16-bit PNG, progressive JPEG, QOI RGB
  linear input, clearer malformed diagnostics, and resource-boundary rejection;
  no externally sourced real-world file corpus is claimed.
- v0.13.0 adds `imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>`
  for exact nearest-neighbor resize across FARBFELD, JPEG, QOI, PBM, PGM, PNG,
  and PPM only. It does not add crop/rotate, new filters, metadata
  preservation, color management, stdin/stdout, or full `magick` CLI parsing.
- v0.14.0 adds
  `imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>` for
  aspect-preserving nearest-neighbor resize into a requested box across the same
  supported formats. It does not change exact `imx resize` behavior or add
  ImageMagick geometry shorthand.
- v0.15.0 adds
  `imx batch-convert --to <FORMAT> --output-dir <dir>
  [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...`
  for safe batch conversion across the same supported formats. It requires an
  existing output directory, exact uppercase target formats, deterministic
  stem-based output names, and fail-fast preflight for collisions, existing
  outputs, same-path writes, missing inputs, and invalid prefixes. It does not
  add recursion, glob expansion, overwrite mode, rename suffixes, stdin/stdout,
  or parallel execution.
- v0.16.0 adds `.bmp` paths and exact uppercase `BMP:` prefixes for
  uncompressed Windows BMP only. It supports 24-bit BGR/RGB and 32-bit
  BGRA/RGBA identify/decode/encode, top-down and bottom-up input,
  deterministic bottom-up output, resize, resize-fit, batch-convert, and
  same-format rewrite. It does not add indexed BMP, RLE, bitfields, OS/2
  headers, color tables, high-depth BMP, profiles, metadata preservation, or
  full ImageMagick BMP behavior.
- v0.17.0 adds `imx self-test`, an offline installed-binary confidence check
  that creates temporary fixtures and invokes the installed binary for
  identify, prefixed identify, transcode, resize, resize-fit, and batch-convert
  across BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM. It also tests clearer CLI
  failures for unknown prefixes, mismatched prefixes, missing paths,
  unsupported variants, invalid geometry, same-path output, batch output-dir
  failures, and unsupported command shapes.
- v0.18.0 adds `imx identify --json [FORMAT:]<input>` and
  `imx report --json [FORMAT:]<input>` for deterministic machine-readable
  identify metadata across BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM. JSON fields
  are limited to `schema_version`, `format`, `width`, `height`, `channels`,
  and `depth`; `report` adds `status` and `diagnostic_code`. JPEG dimensions
  are the existing Orientation-normalized dimensions where applicable. This
  does not add verbose ImageMagick JSON, file hashes, profiles, color
  management, metadata preservation, recursive reporting, or new formats.
- Published v0.4.0 release targets are Linux x86_64, macOS arm64, and macOS
  x86_64. The current v0.18.0 hosted release targets are Linux x86_64 and Linux
  arm64 only, without hosted macOS/iOS Actions.
- ImageMagick remains an oracle for tests and benchmarks only; shipped binaries
  must not link to ImageMagick, MagickCore, MagickWand, delegates, modules,
  `policy.xml`, or ImageMagick's build system.
- v0.18.0 distribution artifacts are the Linux x86_64 and Linux arm64 release
  tarballs, aggregate `SHA256SUMS`, generated `imx.rb`,
  `CONFORMANCE_REPORT.md`, and `conformance-summary.json`. Hosted tag
  automation is Linux-only unless a macOS run is explicitly approved in the
  current turn.

## Evidence Table

| Gate | Producer | Artifact Path | Coverage | Result |
| --- | --- | --- | --- | --- |
| Release gates | `scripts/ci.sh` | terminal plus CI logs | fmt, clippy, tests, fixture generation, self-test and diagnostics tests, fuzz smoke, benchmark smoke, differential tests | required before tag |
| Differential corpus | `scripts/differential-corpus.sh` | `target/differential-corpus-*/summary.json` | identify for 8 formats, prefixed identify for 8 formats, JSON identify/report smoke over the same metadata, high-depth PPM/PNG identify, directed transcodes across BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM, a prefixed transcode ring covering every supported prefix as input/output, plain and prefixed resize plus resize-fit for 8 formats, batch-convert runs across all destination formats plus safety cases, an `imx self-test` result row, 16-bit PPM/PNG preserving transcodes, JPEG RGB8 lossy metric evidence, EXIF Orientation cases against ImageMagick `-auto-orient`, and progressive JPEG RGB/gray/orientation cases | required before tag |
| Curated intake corpus | `scripts/curated-corpus.sh` and `cargo test --test curated_corpus` | `target/curated-corpus/summary.json` | generated/in-test representative intake cases, malformed diagnostic assertions, and resource-boundary checks for supported formats only | required before tag |
| Fuzz smoke | `scripts/run-fuzz.sh` | `target/fuzz-runs/*/summary.json` | BMP, FARBFELD, JPEG, QOI, PNG, and PNM identify/decode with retained crash artifacts | required before tag |
| Scheduled fuzz | `.github/workflows/rust-fuzz-scheduled.yml` | `scheduled-fuzz-evidence` artifact | longer cargo-fuzz run with artifact retention | required CI lane |
| Bench/RSS thresholds | `scripts/bench-release.sh` | `target/release-bench-*/threshold-summary.json` | throughput and process/library RSS sanity budgets | required before tag |
| Bench regression | `scripts/bench-regression.sh` | `target/bench-regression-*/regression-report.json` | v0.18.x vs v0.5.0 throughput/RSS baseline; newer PNG/JPEG/BMP/resize/resize-fit/batch/self-test/JSON metrics without a baseline are warnings, RSS growth is enforced where a baseline exists | required before tag |
| Source install verify | `scripts/verify-install.sh` | `target/install-verify/install-summary.json` | fresh checkout install plus `imx self-test` and supported identify/report JSON, identify/transcode/resize/resize-fit/batch-convert/prefix/BMP/PPM16/PNG/JPEG/orientation/progressive smoke | required before tag |
| Package/SHA/no-link | `scripts/package-release.sh` plus hosted Linux workflow; local macOS or explicitly approved manual evidence for macOS targets | `target/release-artifacts`, GitHub Release assets | deterministic archives, extracted archive `imx self-test`, JSON identify/report, exact-prefix, resize, resize-fit, and batch-convert smoke, BMP/PPM16/PNG/JPEG/orientation/progressive/intake smoke, no ImageMagick linkage, and max `GLIBC_* <= GLIBC_2.34` for each claimed Linux platform; v0.18.x hosted automation prepares Linux x86_64 and Linux arm64 release artifacts | required before publishing that platform archive |
| Published archive smoke | `scripts/verify-release-archive.sh` | `target/release-archive-smoke/<target>/summary.json` | downloads the selected GitHub release archive, verifies that archive against aggregate SHA256SUMS, no-link, max `GLIBC_* <= GLIBC_2.34`, and self-test/JSON identify/report/identify/transcode/resize/resize-fit/batch-convert/same-format/prefix smoke; hosted CI covers Linux only | required after release publish |
| Homebrew tap smoke | `brew tap jskoiz/imx <checkout>`, `brew install jskoiz/imx/imx`, and `brew test jskoiz/imx/imx` from the checked-out tap | `jskoiz/homebrew-imx` Linux formula/archive workflow plus local macOS or explicitly approved manual terminal output | formula URL/SHA fetch, binary version check, `imx self-test`, JSON identify/report, PPM identify, PPM-to-QOI smoke, BMP/PNG/JPEG/orientation/progressive smoke, exact-prefix smoke, resize/resize-fit/batch-convert smoke, and BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM same-format rewrite smoke; local/manual Homebrew install proof for tap claims | required for tap claim; no hosted macOS tap smoke is claimed |
| Conformance report | `scripts/generate-conformance-report.sh` | `CONFORMANCE_REPORT.md`, `conformance-summary.json` | generated from CI evidence, with strict release evidence checks for conformance release assets | required release asset |

## Local Verification

Release verification commands:

```sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imx/target/local-tools/magick-oracle \
  IMX_REQUIRE_ORACLE=1 \
  IMX_FUZZ_MAX_TOTAL_TIME=1 \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/ci.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imx/target/local-tools/magick-oracle \
  bash scripts/differential-corpus.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imx/target/local-tools/magick-oracle \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/bench-release.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imx/target/local-tools/magick-oracle \
  IMX_BENCH_BASE_REF=v0.5.0 \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/bench-regression.sh
IMX_INSTALL_REPO_URL=/Users/jk/Desktop/imx \
  IMX_INSTALL_REVISION=HEAD \
bash scripts/verify-install.sh
bash scripts/package-release.sh
cat target/release-artifacts/linkage-$(rustc -vV | sed -n 's/^host: //p').txt
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
```

After a release is published, each claimed platform must run:

```sh
IMX_VERSION=v0.18.0 IMX_RELEASE_TARGET=<target> \
  bash scripts/verify-release-archive.sh
```

The archive smoke writes `glibc-symbols.txt` for Linux targets and fails when a
published binary references a `GLIBC_*` symbol newer than `GLIBC_2.34`.

## Release Automation

- Release package script: `scripts/package-release.sh`.
- One-command installer: `scripts/install.sh`.
- Release archive smoke script: `scripts/verify-release-archive.sh`.
- Homebrew formula generator: `scripts/generate-homebrew-formula.sh`.
- Conformance report generator: `scripts/generate-conformance-report.sh`.
- Hosted Actions guard: `scripts/check-no-hosted-apple-actions.sh`.
- CI workflow: `.github/workflows/rust-standalone-preview.yml`.
- Scheduled fuzz workflow: `.github/workflows/rust-fuzz-scheduled.yml`.
- Homebrew tap workflow: `jskoiz/homebrew-imx/.github/workflows/tap-smoke.yml`
  for Linux-only hosted formula/archive smoke, plus local or explicitly
  approved manual Homebrew install smoke when making tap claims.
- Branch, pull-request, and tag CI build ImageMagick as an external oracle, run
  IMX release gates, generate differential corpus evidence, generate structured
  benchmark evidence, record v0.5.0 throughput ratios and enforce RSS budgets,
  package Linux x86_64 and Linux arm64 artifacts, verify fresh-checkout
  installation, verify the Linux glibc symbol baseline, generate conformance
  reports, and upload evidence artifacts.
- Tag pushes matching `v*` run the preview gates, build native Linux x86_64 and
  cross-built Linux arm64 release archives, generate aggregate checksums, attach
  the generated tap formula and conformance report, publish the GitHub Release,
  then download the published Linux assets back for smoke tests. The tap formula
  is published through `jskoiz/homebrew-imx`; tap updates are tap-only
  automation, not Homebrew/core submissions, and must not use hosted macOS or
  iOS GitHub Actions. Tap support is claimed only after the formula contains a
  checked URL and SHA generated from a published `SHA256SUMS` entry and
  Linux-only tap smoke verifies it.
- macOS archive or tap proof must run locally or manually after explicit
  approval in the current turn; normal pushes, tags, schedules, and tap updates
  must not start hosted macOS or iOS runners.
- Release archives are written with deterministic tar/gzip metadata, sorted
  entries, fixed ownership, fixed mtimes, stable file modes, and aggregate
  `SHA256SUMS` entries.

## Confirmed Incompatibilities

- IMX is not a full ImageMagick CLI.
- No stdin/stdout streaming.
- No prefixes beyond exact `BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
  `PGM:`, `PNG:`, and `PPM:`.
- No delegates, profiles, color management, transform operations beyond the
  explicit nearest-neighbor resize commands and safe batch composition,
  MagickCore API, or MagickWand API.
- PNG support is limited to static non-interlaced grayscale/RGB/RGBA and
  grayscale-alpha 8/16-bit rasters. APNG, indexed/palette PNG, low-bit PNG,
  `tRNS`, PNG metadata/profile preservation, and PNG color management are out
  of scope.
- JPEG support is limited to 8-bit baseline/progressive grayscale/RGB decode
  and fixed quality 90 baseline encode. CMYK/YCCK JPEG, 12-bit JPEG,
  arithmetic-coded JPEG, lossless JPEG/JPEG-LS, JPEG 2000, JPEG XL,
  metadata/profile preservation beyond read-only Orientation, color management,
  progressive output, and non-opaque alpha JPEG output are out of scope.
- BMP support is limited to uncompressed Windows 24-bit BGR/RGB and 32-bit
  BGRA/RGBA rasters. Indexed BMP, RLE, bitfields, OS/2 headers, color tables,
  high-depth BMP, masks, metadata/profile preservation, and color-management
  semantics are out of scope.
- PBM source form is not preserved; output PBM is deterministic binary P4.
- PBM comments, whitespace, and padding-bit values are not preserved.
- PBM conversion from gray/color inputs is lossy thresholding.
- PPM comments, whitespace, ASCII/binary source form, and source `maxval` are not
  preserved; output PPM is deterministic binary P6 with `maxval 255` or
  `maxval 65535`.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are out of scope.
- P2 input is not source-preserved; output PGM is deterministic binary P5.
- PGM comments and whitespace are not preserved.
- FARBFELD/PPM16/PGM16 to QOI is lossy for non-8-bit-representable 16-bit
  samples.
- Color to PGM/PBM is lossy and ignores alpha.
- No Windows, crates.io, Homebrew/core, or unverified macOS v0.18.x package is
  claimed. Linux arm64 is claimed only for published v0.18.0 archives and
  release-attached formula blocks verified from release `SHA256SUMS`; Homebrew
  tap support is tap-only through `jskoiz/imx` and is verified by Linux-only tap
  smoke for v0.18.0.

## Safety Wins

- PBM parsing is safe Rust and shares the bounded Netpbm header parser with
  PGM/PPM.
- PBM dimensions, row strides, and decoded buffers are checked before
  allocation.
- P4 bit unpacking is row-local and ignores padding bits without reading beyond
  the expected raster.
- Malformed PBM inputs fail without panics in unit tests, fuzz-smoke tests, and
  cargo-fuzz smoke runs.
- CLI writes remain atomic via temp file plus rename.
- Same-format rewrites keep the same atomic output and same-path rejection
  behavior as cross-format transcodes.
- Release archive smoke tests verify the installed binary from the archive, not
  only a source checkout, on each platform where evidence is recorded.
- `imx self-test` gives users and package scripts a no-network way to smoke the
  installed binary across all supported formats and primary commands.
- CLI diagnostic tests lock down error prefix and exit-code behavior for the
  most common user-facing failure classes.
- Homebrew tap smoke verifies formula installation only; compatibility remains
  covered by differential corpus, fuzz, benchmark, and conformance gates.
- Scheduled fuzz retains crash artifacts under the uploaded fuzz evidence.

## Next Smallest Milestone

After the v0.18.0 release/tap closure, the next milestone should improve
everyday usefulness with one bounded operation or format gap, prove it against
ImageMagick where applicable, and keep package/tap proof current. TIFF, GIF,
WebP, APNG, delegates, MagickCore, MagickWand, color management, metadata
preservation beyond declared read-only fields, and full ImageMagick CLI
compatibility remain too broad for a single next milestone.
