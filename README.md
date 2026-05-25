# IMX Developer Preview

IMX is a standalone Rust image tool built one ImageMagick-compatible slice at a
time. The current `v0.2.0` preview supports FARBFELD, QOI, PPM, and PGM
identify/transcode workflows through the `imx` binary.

IMX is not an ImageMagick fork and does not link to MagickCore, MagickWand,
delegates, modules, `policy.xml`, or ImageMagick's build system. ImageMagick is
used only as an external oracle in compatibility tests and benchmarks.

## Install

Download the Linux release archive from:

```text
https://github.com/jskoiz/imx/releases/tag/v0.2.0
```

Or install from source:

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
imx identify input.ff
imx identify input.qoi
imx identify input.ppm
imx identify input.pgm
imx input.ff output.qoi
imx input.ff output.ppm
imx input.ff output.pgm
imx input.qoi output.ff
imx input.qoi output.ppm
imx input.qoi output.pgm
imx input.ppm output.ff
imx input.ppm output.qoi
imx input.ppm output.pgm
imx input.pgm output.ff
imx input.pgm output.qoi
imx input.pgm output.ppm
```

Successful `identify` prints one stable key-value line:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<8|16>
```

Successful transcodes are silent and write the output file. Data and IO
failures exit `1`; unsupported command shapes exit `2`.

## Format Scope

- FARBFELD: RGBA16BE identify/decode/encode.
- QOI: RGB8/RGBA8 identify/decode/encode.
- PPM: ASCII `P3` and binary `P6` RGB8 decode; deterministic binary `P6`
  encode.
- PGM: ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.

Known lossy paths:

- FARBFELD to QOI/PPM quantizes 16-bit samples to 8-bit.
- FARBFELD to PGM converts RGBA16BE to GRAY16BE using Rec.709 luma and ignores
  alpha.
- QOI/PPM/PGM to FARBFELD expands 8-bit samples to 16-bit by byte replication
  and adds opaque alpha where needed.
- Any output to PPM drops alpha and any output to PGM drops color/alpha.

Unsupported by design in `v0.2.0`: full ImageMagick CLI parsing, same-format
rewrites, stdin/stdout streaming, format prefixes such as `QOI:out.qoi`,
delegates, profiles, color management, resizing/transforms, MagickCore,
MagickWand, PBM, PAM, PFM, high-depth PPM, PNG, and BMP.

## Safety Posture

- Product decode/encode paths are safe Rust.
- Runtime dependencies are only local IMX crates.
- Decoded pixel buffers are capped at 512 MiB.
- CLI input reads are capped at 513 MiB.
- Output writes use a temp file plus rename, and malformed input does not leave
  the requested output behind.
- Fuzz targets cover FARBFELD, QOI, and PNM decode/identify entrypoints with
  seeded FARBFELD/QOI/PPM/PGM corpora.

## Release Gates

Run the local gates:

```sh
./scripts/ci.sh
```

Require ImageMagick oracle differentials:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
```

Run coverage-guided fuzz smoke:

```sh
IMX_FUZZ_MAX_TOTAL_TIME=5 ./scripts/run-fuzz.sh
```

Generate machine-readable benchmark evidence:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
```

Package a release archive:

```sh
./scripts/package-release.sh
```

Release archives use deterministic tar/gzip metadata and a relative
`SHA256SUMS` entry so repeated packaging of the same built payload is
byte-for-byte comparable.

Verify source installation from a fresh checkout:

```sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
```

## Evidence

The CI workflow builds ImageMagick as an external oracle, runs the release
gates, runs fuzz targets, verifies install from a fresh checkout, packages IMX,
checks that debug/release binaries do not link ImageMagick, and uploads release
evidence.

Benchmark runs emit:

- `metadata.txt`
- `benchmark-run.json`
- `measurements.jsonl`
- `summary.json`
- generated fixture `manifest.txt` and `manifest.json`
- raw `/usr/bin/time` outputs and output hashes

See [COMPATIBILITY.md](COMPATIBILITY.md) for the exact behavior contract and
[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for current release evidence,
known gaps, and the next adoption milestone.
