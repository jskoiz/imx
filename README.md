# IMX Developer Preview

IMX is a standalone Rust image tool built one ImageMagick-compatible slice at a
time. The current `v0.4.0` preview supports deterministic identify and
transcode workflows across FARBFELD, QOI, and Netpbm PBM/PGM/PPM through the
`imx` binary.

IMX is not an ImageMagick fork and does not link to MagickCore, MagickWand,
delegates, modules, `policy.xml`, or ImageMagick's build system. ImageMagick is
used only as an external oracle in compatibility tests and benchmarks.

## Install

Install the latest v0.4.0 release from the Homebrew tap:

```sh
brew tap jskoiz/imx
brew install imx
imx --version
```

This uses the `jskoiz/homebrew-imx` tap formula for the prebuilt v0.4.0
archive. It supports macOS arm64, macOS x86_64, and Linux x86_64. It is not a
Homebrew/core formula.

Hosted GitHub Actions for the tap are Linux-only; macOS install proof must be
run locally or manually after explicit approval.

Or install the release archive directly:

```sh
curl -fsSL https://raw.githubusercontent.com/jskoiz/imx/v0.4.0/scripts/install.sh | sh
```

The installer verifies the published `SHA256SUMS`, installs `imx`, asserts the
installed version, and runs a small identify/transcode smoke test. Supported
archive targets are:

- `imx-preview-0.4.0-x86_64-unknown-linux-gnu.tar.gz`
- `imx-preview-0.4.0-aarch64-apple-darwin.tar.gz`
- `imx-preview-0.4.0-x86_64-apple-darwin.tar.gz`

No Windows, Linux arm64, crates.io, Homebrew/core, or package-manager
distribution beyond the `jskoiz/imx` tap is claimed for v0.4.0. Release
archives are published at:

```text
https://github.com/jskoiz/imx/releases/tag/v0.4.0
```

The release-attached `imx.rb` is the formula source published through the
`jskoiz/homebrew-imx` tap.

Or install from source:

```sh
git clone https://github.com/jskoiz/imx.git
cd imx
git checkout v0.4.0
cargo install --path crates/cli --bin imx --locked
imx --version
```

The source install path is verified by `scripts/verify-install.sh` from a fresh
checkout in CI.

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

Successful `identify` prints one stable key-value line:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

Successful transcodes are silent and write the output file. Data and IO
failures exit `1`; unsupported command shapes exit `2`.

## Format Scope

- FARBFELD: RGBA16BE identify/decode/encode.
- QOI: RGB8/RGBA8 identify/decode/encode.
- PBM: ASCII `P1` and binary `P4` bilevel decode; deterministic binary `P4`
  encode.
- PGM: ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.
- PPM: ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6`
  encode.

Known lossy paths:

- FARBFELD to QOI/PPM quantizes 16-bit samples to 8-bit.
- FARBFELD/QOI/PPM/PGM to PBM uses Rec.709 luma where needed, then thresholds
  `<128` or `<32768` to black and all higher values to white.
- FARBFELD to PGM converts RGBA16BE to GRAY16BE using Rec.709 luma and ignores
  alpha.
- QOI/PBM/PGM/PPM to FARBFELD expands 8-bit samples to 16-bit by byte
  replication and adds opaque alpha where needed.
- Any output to PPM drops alpha; any output to PGM drops color/alpha; any
  output to PBM drops color, alpha, and grayscale precision.

Unsupported by design in `v0.4.0`: full ImageMagick CLI parsing, same-format
rewrites, stdin/stdout streaming, format prefixes such as `QOI:out.qoi`,
delegates, profiles, color management, resizing/transforms, MagickCore,
MagickWand, PAM, PFM, high-depth PPM, PNG, BMP, and other image formats.

## Safety Posture

- Product decode/encode paths are safe Rust.
- Runtime dependencies are only local IMX crates.
- Decoded pixel buffers are capped at 512 MiB.
- CLI input reads are capped at 513 MiB.
- Output writes use a temp file plus rename, and malformed input does not leave
  the requested output behind.
- Fuzz targets cover FARBFELD, QOI, and PNM decode/identify entrypoints with
  seeded FARBFELD/QOI/PBM/PGM/PPM corpora.

## Release Gates

Run the local gates:

```sh
./scripts/ci.sh
```

Require ImageMagick oracle differentials:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
```

Run the corpus differential report directly:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/differential-corpus.sh
```

Run coverage-guided fuzz smoke:

```sh
IMX_FUZZ_MAX_TOTAL_TIME=5 ./scripts/run-fuzz.sh
```

Scheduled CI runs the same cargo-fuzz targets for a longer window and retains
crash artifacts under the fuzz evidence directory.

Generate machine-readable benchmark evidence:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
```

Compare current benchmark/RSS evidence against the v0.3.0 baseline:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_BENCH_BASE_REF=v0.3.0 ./scripts/bench-regression.sh
```

Package a release archive:

```sh
./scripts/package-release.sh
```

For tags created after the Linux arm64 workflow change, hosted Linux release
automation also packages `aarch64-unknown-linux-gnu` with the Rust cross target,
QEMU smoke, architecture checks, and `readelf` linkage checks. That archive is
not claimed for the already-published v0.4.0 release. No hosted macOS or iOS
runner is used for the Linux arm64 proof.

Release archives use deterministic tar/gzip metadata and aggregate
`SHA256SUMS` entries so repeated packaging of the same built payload is
byte-for-byte comparable.

Verify source installation from a fresh checkout:

```sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
```

Verify published Linux release archives after GitHub release publication:

```sh
IMX_VERSION=v0.4.0 IMX_RELEASE_TARGET=x86_64-unknown-linux-gnu ./scripts/verify-release-archive.sh
```

Verify the Homebrew tap install smoke:

```sh
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
```

`brew test` verifies installation only. Compatibility remains covered by the
CI differential corpus, fuzz, benchmark, and conformance gates.

## Evidence

The hosted CI workflow builds ImageMagick as an external oracle, runs release
gates, runs fuzz targets, verifies install from a fresh checkout, packages Linux
x86_64 release archives and post-v0.4.0 Linux arm64 preview archives, checks
hosted-built binaries for ImageMagick linkage, generates the release conformance
report, and downloads published Linux assets back for archive smoke. macOS
archive and tap proof must be recorded locally or through an explicitly approved
manual run before new macOS claims are made. Hosted GitHub Actions must not run
macOS or iOS jobs without explicit approval in the current turn.

Benchmark runs emit:

- `metadata.txt`
- `benchmark-run.json`
- `measurements.jsonl`
- `summary.json`
- `threshold-summary.json`
- generated fixture `manifest.txt` and `manifest.json`
- raw `/usr/bin/time` outputs and output hashes

Tag releases additionally attach:

- `SHA256SUMS`
- `CONFORMANCE_REPORT.md`
- `conformance-summary.json`

The v0.4.0 release also attached the `imx.rb` Homebrew tap formula snapshot.
Future tap updates are handled in `jskoiz/homebrew-imx` without hosted macOS
GitHub Actions.

See [COMPATIBILITY.md](COMPATIBILITY.md) for the exact behavior contract and
[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for current release evidence,
known gaps, and the next adoption milestone.
