# IMX v0.5.0 Compatibility Readiness

Status: v0.5.0 release surface. Hosted release proof is Linux-only; Homebrew tap
support is claimed only after the tap formula is regenerated from the published
`SHA256SUMS` and Linux tap smoke passes.
Automatic hosted macOS/iOS GitHub Actions remain disabled; macOS proof is
local/manual only unless explicitly approved in the current turn.

## Implemented Behavior

- Source of truth: `https://github.com/jskoiz/imx`.
- Product binary: `imx`.
- v0.5.0 does not add a new image format. It adds deterministic same-format
  rewrites for the existing FARBFELD/QOI/PBM/PGM/PPM slice when input and
  output paths are different.
- Published v0.4.0 release targets are Linux x86_64, macOS arm64, and macOS
  x86_64. The planned v0.5.0 hosted release targets are Linux x86_64 and Linux
  arm64 only, without hosted macOS/iOS Actions.
- ImageMagick remains an oracle for tests and benchmarks only; shipped binaries
  must not link to ImageMagick, MagickCore, MagickWand, delegates, modules,
  `policy.xml`, or ImageMagick's build system.
- Public v0.4.0 distribution artifacts are the three release tarballs,
  aggregate `SHA256SUMS`, `imx.rb` formula published through the
  `jskoiz/homebrew-imx` tap, `CONFORMANCE_REPORT.md`, and
  `conformance-summary.json`. Future hosted tag automation is Linux-only unless
  a macOS run is explicitly approved in the current turn.

## Evidence Table

| Gate | Producer | Artifact Path | Coverage | Result |
| --- | --- | --- | --- | --- |
| Release gates | `scripts/ci.sh` | terminal plus CI logs | fmt, clippy, tests, fixture generation, fuzz smoke, benchmark smoke, differential tests | required before tag |
| Differential corpus | `scripts/differential-corpus.sh` | `target/differential-corpus-*/summary.json` | identify for 5 formats plus 25 directed transcodes, including same-format rewrites | required before tag |
| Fuzz smoke | `scripts/run-fuzz.sh` | `target/fuzz-runs/*/summary.json` | FARBFELD, QOI, and PNM identify/decode with retained crash artifacts | required before tag |
| Scheduled fuzz | `.github/workflows/rust-fuzz-scheduled.yml` | `scheduled-fuzz-evidence` artifact | longer cargo-fuzz run with artifact retention | required CI lane |
| Bench/RSS thresholds | `scripts/bench-release.sh` | `target/release-bench-*/threshold-summary.json` | throughput and process/library RSS sanity budgets | required before tag |
| Bench regression | `scripts/bench-regression.sh` | `target/bench-regression-*/regression-report.json` | v0.5.0 vs v0.4.0 throughput/RSS baseline; throughput ratios are tracked as warnings, RSS growth is enforced | required before tag |
| Source install verify | `scripts/verify-install.sh` | `target/install-verify/install-summary.json` | fresh checkout install plus supported identify/transcode smoke | required before tag |
| Package/SHA/no-link | `scripts/package-release.sh` plus hosted Linux workflow; local macOS or explicitly approved manual evidence for macOS targets | `target/release-artifacts`, GitHub Release assets | deterministic archives, extracted archive smoke, no ImageMagick linkage for each claimed platform; post-v0.4.0 workflow prepares Linux arm64 preview artifacts | required before publishing that platform archive |
| Published archive smoke | `scripts/verify-release-archive.sh` | `target/release-archive-smoke/<target>/summary.json` | downloads the selected GitHub release archive, verifies that archive against aggregate SHA256SUMS, no-link, identify/transcode smoke; hosted CI covers Linux only | required after release publish |
| Homebrew tap smoke | `brew install jskoiz/imx/imx` and `brew test jskoiz/imx/imx` | `jskoiz/homebrew-imx` Linux formula/archive workflow plus local macOS or explicitly approved manual terminal output | formula URL/SHA fetch, binary version check, PPM identify, PPM-to-QOI smoke, and PPM same-format rewrite smoke; local/manual Homebrew install proof for tap claims | required for tap claim; no hosted macOS tap smoke is claimed |
| Conformance report | `scripts/generate-conformance-report.sh` | `CONFORMANCE_REPORT.md`, `conformance-summary.json` | generated from CI evidence and attached to the release | required release asset |

## Local Verification

Candidate commands:

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
  IMX_BENCH_BASE_REF=v0.4.0 \
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
IMX_VERSION=v0.5.0 IMX_RELEASE_TARGET=<target> \
  bash scripts/verify-release-archive.sh
```

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
  benchmark evidence, record v0.4.0 throughput ratios and enforce RSS budgets,
  package Linux x86_64 and Linux arm64 artifacts, verify fresh-checkout
  installation, generate conformance reports, and upload evidence artifacts.
- Tag pushes matching `v*` run the preview gates, build native Linux x86_64 and
  cross-built Linux arm64 release archives, generate aggregate checksums, attach
  the generated tap formula and conformance report, publish the GitHub Release,
  then download the published Linux assets back for smoke tests. The tap formula
  is published through `jskoiz/homebrew-imx`; tap updates are tap-only
  automation, not Homebrew/core submissions, and must not use hosted macOS or
  iOS GitHub Actions. Linux arm64 is not claimed for the already-published
  v0.4.0 assets, and tap support is claimed only after the formula contains a
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
- No Windows, crates.io, Homebrew/core, or unverified macOS v0.5.0 release is
  claimed. Linux arm64 is claimed only for published v0.5.0 archives and tap
  blocks verified from release `SHA256SUMS`; Homebrew support is tap-only
  through `jskoiz/imx`.

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
- Homebrew tap smoke verifies formula installation only; compatibility remains
  covered by differential corpus, fuzz, benchmark, and conformance gates.
- Scheduled fuzz retains crash artifacts under the uploaded fuzz evidence.

## Next Smallest Milestone

After v0.5.0, the next compatibility slice should stay similarly bounded, such
as exact format-prefix parsing for the existing five formats or one Netpbm
behavior backed by oracle fixtures. PNG, JPEG, TIFF, delegates, MagickCore,
MagickWand, and full ImageMagick CLI compatibility remain too broad for the
next milestone.
