# IMX v0.4.0 Public Install Readiness

Status: v0.4.0 public install report after GitHub Release archive smoke and
Homebrew tap install/test verification.

## Implemented Behavior

- Source of truth: `https://github.com/jskoiz/imx`.
- Product binary: `imx`.
- v0.4.0 does not add a new image format. It hardens the v0.3.0
  FARBFELD/QOI/PBM/PGM/PPM slice into a public-install readiness release.
- Supported release targets are Linux x86_64, macOS arm64, and macOS x86_64.
- ImageMagick remains an oracle for tests and benchmarks only; shipped binaries
  must not link to ImageMagick, MagickCore, MagickWand, delegates, modules,
  `policy.xml`, or ImageMagick's build system.
- Public distribution artifacts are the three release tarballs, aggregate
  `SHA256SUMS`, `imx.rb` formula published through the `jskoiz/homebrew-imx` tap,
  `CONFORMANCE_REPORT.md`, and `conformance-summary.json`.

## Evidence Table

| Gate | Producer | Artifact Path | Coverage | Result |
| --- | --- | --- | --- | --- |
| Release gates | `scripts/ci.sh` | terminal plus CI logs | fmt, clippy, tests, fixture generation, fuzz smoke, benchmark smoke, differential tests | required before tag |
| Differential corpus | `scripts/differential-corpus.sh` | `target/differential-corpus-*/summary.json` | identify for 5 formats plus 20 directed cross-format transcodes | required before tag |
| Fuzz smoke | `scripts/run-fuzz.sh` | `target/fuzz-runs/*/summary.json` | FARBFELD, QOI, and PNM identify/decode with retained crash artifacts | required before tag |
| Scheduled fuzz | `.github/workflows/rust-fuzz-scheduled.yml` | `scheduled-fuzz-evidence` artifact | longer cargo-fuzz run with artifact retention | required CI lane |
| Bench/RSS thresholds | `scripts/bench-release.sh` | `target/release-bench-*/threshold-summary.json` | throughput and process/library RSS sanity budgets | required before tag |
| Bench regression | `scripts/bench-regression.sh` | `target/bench-regression-*/regression-report.json` | current candidate vs v0.3.0 throughput/RSS baseline; throughput ratios are tracked as warnings, RSS growth is enforced | required before tag |
| Source install verify | `scripts/verify-install.sh` | `target/install-verify/install-summary.json` | fresh checkout install plus supported identify/transcode smoke | required before tag |
| Package/SHA/no-link | `scripts/package-release.sh` and release workflow | `target/release-artifacts`, GitHub Release assets | deterministic archives, extracted archive smoke, no ImageMagick linkage | required before tag |
| Published archive smoke | `scripts/verify-release-archive.sh` | `target/release-archive-smoke/<target>/summary.json` | downloads GitHub release assets, verifies aggregate SHA256SUMS, no-link, identify/transcode smoke | required after release publish |
| Homebrew tap smoke | `brew install jskoiz/imx/imx` and `brew test jskoiz/imx/imx` | `jskoiz/homebrew-imx` workflow and local terminal output | formula SHA fetch, binary install, `imx 0.4.0` version check, PPM identify, PPM-to-QOI smoke | required for tap claim |
| Conformance report | `scripts/generate-conformance-report.sh` | `CONFORMANCE_REPORT.md`, `conformance-summary.json` | generated from CI evidence and attached to the release | required release asset |

## Local Verification

Candidate commands:

```sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  IMX_REQUIRE_ORACLE=1 \
  IMX_FUZZ_MAX_TOTAL_TIME=1 \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/ci.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  bash scripts/differential-corpus.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/bench-release.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  IMX_BENCH_BASE_REF=v0.3.0 \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/bench-regression.sh
IMX_INSTALL_REPO_URL=/Users/jk/Desktop/imx \
  IMX_INSTALL_REVISION=HEAD \
  bash scripts/verify-install.sh
bash scripts/package-release.sh
otool -L target/release/imx
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
```

After the `v0.4.0` release is published, each supported platform must run:

```sh
IMX_VERSION=v0.4.0 IMX_RELEASE_TARGET=<target> \
  bash scripts/verify-release-archive.sh
```

## Release Automation

- Release package script: `scripts/package-release.sh`.
- One-command installer: `scripts/install.sh`.
- Release archive smoke script: `scripts/verify-release-archive.sh`.
- Homebrew formula generator: `scripts/generate-homebrew-formula.sh`.
- Conformance report generator: `scripts/generate-conformance-report.sh`.
- CI workflow: `.github/workflows/rust-standalone-preview.yml`.
- Scheduled fuzz workflow: `.github/workflows/rust-fuzz-scheduled.yml`.
- Homebrew tap workflow: `jskoiz/homebrew-imx/.github/workflows/tap-smoke.yml`.
- Branch, pull-request, and tag CI build ImageMagick as an external oracle, run
  IMX release gates, generate differential corpus evidence, generate structured
  benchmark evidence, record v0.3.0 throughput ratios and enforce RSS budgets,
  package release
  artifacts, verify fresh-checkout installation, generate conformance reports,
  and upload evidence artifacts.
- Tag pushes matching `v*` run the preview gates, build native Linux/macOS
  release archives, generate aggregate checksums, generate a Homebrew formula
  snapshot, attach the conformance report, publish the GitHub Release, then
  download the published assets back for platform smoke tests. The tap formula
  is published through `jskoiz/homebrew-imx`; tap updates are committed there
  from the generated formula rather than pushed by the tag workflow.
- Release archives are written with deterministic tar/gzip metadata, sorted
  entries, fixed ownership, fixed mtimes, stable file modes, and aggregate
  `SHA256SUMS` entries.

## Confirmed Incompatibilities

- IMX is not a full ImageMagick CLI.
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- PBM source form is not preserved; output PBM is deterministic binary P4.
- PBM comments, whitespace, and padding-bit values are not preserved.
- PBM conversion from gray/color inputs is lossy thresholding.
- PPM is RGB8-only; high-depth PPM is out of scope.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are out of scope.
- P2 input is not source-preserved; output PGM is deterministic binary P5.
- PGM comments and whitespace are not preserved.
- FARBFELD to QOI/PPM is lossy for non-8-bit-representable 16-bit samples.
- Color to PGM/PBM is lossy and ignores alpha.
- No PAM, PFM, PNG, or BMP.
- No Windows, Linux arm64, crates.io, or Homebrew/core release is claimed for
  v0.4.0; Homebrew support is tap-only through `jskoiz/imx`.

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
- Release archive smoke tests verify the installed binary from the archive, not
  only a source checkout.
- Homebrew tap smoke verifies formula installation only; compatibility remains
  covered by CI differential corpus, fuzz, benchmark, and conformance gates.
- Scheduled fuzz retains crash artifacts under the uploaded fuzz evidence.

## Next Smallest Milestone

After v0.4.0, the smallest adoption-expanding next milestone is Linux arm64
release archive support or tap update automation if user demand points there.
PNG, JPEG, TIFF, delegates, MagickCore, MagickWand, and full ImageMagick CLI
compatibility remain too broad for the next milestone.
