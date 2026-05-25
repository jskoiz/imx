# IMX Developer Preview

IMX is a standalone Rust image tool built one ImageMagick-compatible slice at a
time. The current source tree is the `v0.7.0` implementation candidate: it
supports deterministic identify, cross-format transcode, same-format rewrite,
exact uppercase format-prefix workflows, and high-depth PPM for the existing
FARBFELD, QOI, and Netpbm PBM/PGM/PPM surface through the `imx` binary.

IMX is not an ImageMagick fork and does not link to MagickCore, MagickWand,
delegates, modules, `policy.xml`, or ImageMagick's build system. ImageMagick is
used only as an external oracle in compatibility tests and benchmarks.

The v0.7.0 candidate adds PPM `maxval` 256..65535 support for existing `P3` and
`P6` PPM only. It does not add PNG/JPEG/TIFF/PAM/PFM/BMP, stdin/stdout
streaming, a `magick` alias, full ImageMagick CLI parsing, delegates,
MagickCore, or MagickWand.

## Install

The latest published tap/release remains v0.6.0 until a v0.7.0 release is cut.
Install the published v0.6.0 tap release:

```sh
brew tap jskoiz/imx
brew install imx
imx --version
```

This uses the `jskoiz/homebrew-imx` tap formula generated from each published
release's `SHA256SUMS`. For v0.6.0, tap support is limited to archive targets
present in the published v0.6.0 release and verified by tap smoke. It is not a
Homebrew/core formula.

Hosted GitHub Actions for the tap are Linux-only; macOS install proof must be
run locally or manually after explicit approval.

Install the published v0.6.0 release archive directly:

```sh
IMX_VERSION=v0.6.0
curl -fsSL "https://raw.githubusercontent.com/jskoiz/imx/${IMX_VERSION}/scripts/install.sh" | sh
```

The installer verifies the published `SHA256SUMS`, installs `imx`, asserts the
installed version, and runs a small identify/transcode smoke test. Hosted
v0.6.0 tag automation publishes Linux archives for:

- `imx-preview-0.6.0-x86_64-unknown-linux-gnu.tar.gz`
- `imx-preview-0.6.0-aarch64-unknown-linux-gnu.tar.gz`

macOS v0.6.0 archives or tap blocks require recorded local/manual proof before
being claimed. No Windows, crates.io, Homebrew/core, or package-manager
distribution beyond the `jskoiz/imx` tap is claimed. The v0.6.0 release URL is:

```text
https://github.com/jskoiz/imx/releases/tag/v0.6.0
```

The release-attached `imx.rb` is the formula source used to update the
`jskoiz/homebrew-imx` tap from the published `SHA256SUMS`. For v0.6.0, Linux
x86_64 and Linux arm64 tap blocks are generated from the release checksums and
verified by Linux-only tap smoke.

Or install the current v0.7.0 candidate from source:

```sh
git clone https://github.com/jskoiz/imx.git
cd imx
cargo install --path crates/cli --bin imx --locked
imx --version
```

The source install path is verified by `scripts/verify-install.sh` from a fresh
checkout in CI.

## Supported Commands

```sh
imx --help
imx --version
imx identify [FORMAT:]<input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.ppm>
imx [FORMAT:]<input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.ppm> \
  [FORMAT:]<output.ff|output.farbfeld|output.qoi|output.pbm|output.pgm|output.ppm>
```

Supported exact prefixes are `FARBFELD:`, `QOI:`, `PBM:`, `PGM:`, and `PPM:`.
Prefixes are accepted only on `identify` and two-path transcode operands. They
are stripped before file IO, must match the detected input format or output
path extension, and do not add extensionless output selection. Unknown,
missing-path, and mismatched prefixes fail with `error: ...`; same-path
rejection still compares the stripped real paths.

Successful `identify` prints one stable key-value line:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

Successful transcodes are silent and write the output file. Data and IO
failures exit `1`; unsupported command shapes exit `2`.

Same-format rewrites are deterministic decode/re-encode operations for
different input and output paths. They do not preserve source bytes, comments,
whitespace, Netpbm ASCII/binary source form, QOI opcode choices, or other
incidental representation details.

## Format Scope

- FARBFELD: RGBA16BE identify/decode/encode.
- QOI: RGB8/RGBA8 identify/decode/encode.
- PBM: ASCII `P1` and binary `P4` bilevel decode; deterministic binary `P4`
  encode.
- PGM: ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.
- PPM: ASCII `P3` and binary `P6` RGB8/RGB16BE decode; deterministic binary
  `P6` encode with `maxval 255` for 8-bit sources and `maxval 65535` for 16-bit
  RGB/RGBA/GRAY sources.

Known lossy paths:

- FARBFELD to QOI quantizes 16-bit samples to 8-bit.
- PPM to QOI quantizes 16-bit PPM samples to 8-bit.
- FARBFELD/QOI/PPM/PGM to PBM uses Rec.709 luma where needed, then thresholds
  `<128` or `<32768` to black and all higher values to white.
- FARBFELD to PGM converts RGBA16BE to GRAY16BE using Rec.709 luma and ignores
  alpha.
- QOI/PBM/8-bit PGM/8-bit PPM to FARBFELD expands 8-bit samples to 16-bit by
  byte replication and adds opaque alpha where needed. High-depth PGM/PPM keeps
  16-bit samples.
- Any output to PPM drops alpha; any output to PGM drops color/alpha; any
  output to PBM drops color, alpha, and grayscale precision.

Unsupported by design: full ImageMagick CLI parsing, stdin/stdout streaming,
prefixes outside the exact five listed above, delegates, profiles, color
management, resizing/transforms, MagickCore, MagickWand, PAM, PFM, PNG, BMP,
and other image formats.

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

Compare current benchmark/RSS evidence against the v0.5.0 baseline:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_BENCH_BASE_REF=v0.5.0 ./scripts/bench-regression.sh
```

Package a release archive:

```sh
./scripts/package-release.sh
```

For v0.6.0 and later tags, hosted Linux release automation packages
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` archives. The Linux
arm64 archive uses the Rust cross target, QEMU smoke, architecture checks, and
`readelf` linkage checks. Linux arm64 is not claimed for the already-published
v0.4.0 release. No hosted macOS or iOS runner is used for release proof.

Release archives use deterministic tar/gzip metadata and aggregate
`SHA256SUMS` entries so repeated packaging of the same built payload is
byte-for-byte comparable.

Verify source installation from a fresh checkout:

```sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
```

Verify published Linux release archives after GitHub release publication:

```sh
IMX_VERSION=v0.6.0 IMX_RELEASE_TARGET=x86_64-unknown-linux-gnu ./scripts/verify-release-archive.sh
```

Verify the v0.6.0 Homebrew tap install smoke:

```sh
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
test "$(imx --version)" = "imx 0.6.0"
```

`brew test` verifies installation only. Compatibility remains covered by the
CI differential corpus, fuzz, benchmark, and conformance gates.

## Evidence

The hosted CI workflow builds ImageMagick as an external oracle, runs release
gates, runs fuzz targets, verifies install from a fresh checkout, packages Linux
x86_64 and Linux arm64 archives, checks hosted-built binaries for ImageMagick
linkage, generates the release conformance report, and downloads published Linux
assets back for archive smoke after a tag publish. macOS archive and tap proof
must be recorded locally or through an explicitly approved manual run before new
macOS claims are made. Hosted GitHub Actions must not run macOS or iOS jobs
without explicit approval in the current turn.

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

The v0.6.0 and later release path attaches the generated `imx.rb` Homebrew tap
formula based on whichever supported archive targets are present in that
release's `SHA256SUMS`. Tap updates are handled in `jskoiz/homebrew-imx` without
hosted macOS GitHub Actions.

See [COMPATIBILITY.md](COMPATIBILITY.md) for the exact behavior contract and
[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for current release evidence,
known gaps, and the next adoption milestone.
The v0.7.0 implementation contract is tracked in
[docs/v0.7.0-high-depth-ppm.md](docs/v0.7.0-high-depth-ppm.md). The published
v0.6.0 release checklist remains in
[docs/v0.6.0-release-ready.md](docs/v0.6.0-release-ready.md), and the bounded
prefix compatibility contract remains in
[docs/v0.6.0-compatibility-recommendation.md](docs/v0.6.0-compatibility-recommendation.md).
