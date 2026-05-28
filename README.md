# IMX Developer Preview

IMX is a standalone Rust image tool built one ImageMagick-compatible slice at a
time. The current developer-preview version is `v0.18.0`: it supports
deterministic identify, cross-format transcode, same-format rewrite, exact
uppercase format-prefix workflows, bounded nearest-neighbor exact resize,
aspect-preserving resize-fit, safe batch conversion, an offline installed-binary
self-test, machine-readable identify/report JSON, high-depth PPM, a bounded PNG
raster surface, bounded uncompressed BMP, plus bounded 8-bit
baseline/progressive JPEG grayscale/RGB support, for BMP, FARBFELD, JPEG, QOI,
PNG, and Netpbm PBM/PGM/PPM through the `imx` binary.

IMX is not an ImageMagick fork and does not link to MagickCore, MagickWand,
delegates, modules, `policy.xml`, or ImageMagick's build system. ImageMagick is
used only as an external oracle in compatibility tests and benchmarks.

The v0.8.0 release adds static non-interlaced PNG identify/decode/encode for
8-bit and 16-bit grayscale, RGB, RGBA, and grayscale-alpha rasters. It does not
add APNG, indexed/palette PNG, low-bit PNG, PNG metadata/profile preservation,
color management, JPEG/TIFF/PAM/PFM/BMP, stdin/stdout streaming, a `magick`
alias, full ImageMagick CLI parsing, delegates, MagickCore, or MagickWand.

The v0.9.0 release adds `.jpg`/`.jpeg` and exact `JPEG:` support for 8-bit
grayscale/RGB JPEG identify and transcode. JPEG output uses fixed quality 90
encoding, rejects non-opaque alpha inputs, and does not preserve metadata,
profiles, chroma subsampling, quantization tables, scan layout, or source
bytes.

The v0.10.0 release adds bounded read-only JPEG EXIF Orientation handling.
Orientation values 1 through 8 normalize decoded pixels, and `identify` reports
the oriented dimensions. All other EXIF, ICC, XMP, density, thumbnail, and
camera metadata remains unpreserved and uninterpreted.

The v0.11.0 release adds bounded progressive JPEG input support for 8-bit
grayscale/RGB streams. It preserves the v0.10.0 Orientation behavior on
progressive input and still writes deterministic quality-90 baseline JPEG
output.

The v0.12.0 release keeps the v0.11.0 format and command surface and adds a
bounded representative intake reliability slice for already-supported input
families. The claim covers only the tested generated/in-test intake fixtures,
diagnostics, and resource-boundary cases; no externally sourced real-world file
corpus is claimed, and it does not add formats, transforms, streaming, metadata
preservation, or broader ImageMagick CLI behavior.

The v0.13.0 release adds one explicit resize command:
`imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>`. Resize uses
center-sampled nearest-neighbor scaling to exact dimensions, preserves the
decoded pixel format until the existing output encoder runs, and supports only
the already-supported FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM formats. It does not
add crop, rotate, aspect-ratio shorthand, filters beyond nearest neighbor,
metadata preservation, color management, stdin/stdout, or full `magick` CLI
parsing.

The v0.14.0 release adds one explicit aspect-preserving resize command:
`imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>`.
Resize-fit selects the largest integer output dimensions that fit inside the
requested box while preserving source aspect ratio, then uses the same
center-sampled nearest-neighbor scaling and existing encoder rules as exact
resize. It does not change `imx resize`, add crop/fill/percentage geometry, or
broaden into ImageMagick `-resize` syntax.

The v0.15.0 release adds one explicit safe batch command:
`imx batch-convert --to <FORMAT> --output-dir <dir>
[--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...`.
Batch conversion uses existing decoders, encoders, exact resize, resize-fit,
and prefix validation. It requires an existing output directory, derives
deterministic output names from input stems and the target format, rejects
collisions and existing outputs before writing, and does not add recursion,
glob parsing, overwrite mode, rename suffixes, stdin/stdout, or parallel
execution.

The v0.16.0 release adds bounded uncompressed Windows BMP support for 24-bit
BGR/RGB and 32-bit BGRA/RGBA rasters. It works through `.bmp` paths and exact
`BMP:` prefixes for identify, transcode, resize, resize-fit, batch-convert,
and deterministic same-format rewrites. It does not add indexed, RLE,
bitfields, OS/2, color-table, high-depth BMP, or broader ImageMagick BMP
semantics.

The v0.17.0 release adds `imx self-test`, an offline installed-binary smoke
check that creates temporary fixtures and exercises identify, transcode, exact
resize, resize-fit, and batch-convert across BMP, FARBFELD, JPEG, QOI, PBM,
PGM, PNG, and PPM. It also tightens CLI diagnostics and exit-code tests for
unknown prefixes, mismatched prefixes, missing paths, unsupported variants,
invalid geometry, same-path outputs, and batch-convert failures. It does not
replace ImageMagick differential, fuzz, benchmark, or release archive gates.

The v0.18.0 release adds one machine-readable daily-use surface:
`imx identify --json [FORMAT:]<input>` and
`imx report --json [FORMAT:]<input>`. JSON is deterministic and limited to the
existing identify metadata: `schema_version`, `format`, `width`, `height`,
`channels`, and `depth`. JPEG `width` and `height` are the existing
Orientation-normalized dimensions where EXIF Orientation applies. `report`
adds `status` and `diagnostic_code` for supported/unsupported outcomes. It does
not add new metadata extraction, ImageMagick JSON compatibility, profiles,
color management, file hashes, recursive batch reporting, or new formats.

The v0.19.0 development track adds a daily-use corpus hardening gate for the
same existing surface. `scripts/daily-use-corpus.sh` runs a real `imx` binary
against generated fixtures for JSON identify/report, representative prefixed
transcodes, stable unsupported diagnostics, and `identify --json` failure JSON.
It is a no-oracle install/package/release confidence gate, not a new format or
command surface.

## Install

Install the verified v0.18.0 tap release:

```sh
brew tap jskoiz/imx
brew install imx
imx --version
```

This uses the `jskoiz/homebrew-imx` tap formula generated from each published
release's `SHA256SUMS`. For v0.18.0, the `jskoiz/homebrew-imx` tap was updated
from the published release checksums and passed Linux-only tap smoke. It is not
a Homebrew/core formula. Published Linux archives require glibc 2.34 or newer.
Release/archive smoke checks this by asserting that published Linux binaries do
not reference `GLIBC_*` symbols newer than `GLIBC_2.34`; the v0.18.0 release
workflow verifies that ceiling from the published assets.

Hosted GitHub Actions for the tap are Linux-only; macOS install proof must be
run locally or manually after explicit approval.

Install the published v0.18.0 release archive directly:

```sh
IMX_VERSION=v0.18.0
curl -fsSL "https://raw.githubusercontent.com/jskoiz/imx/${IMX_VERSION}/scripts/install.sh" | sh
```

The installer verifies the published `SHA256SUMS`, installs `imx`, asserts the
installed version, checks for glibc 2.34 or newer on Linux, and runs a small
`imx self-test` plus identify/report JSON and
identify/transcode/resize/resize-fit/batch-convert smoke. Hosted v0.18.0 tag
automation publishes Linux archives for:

- `imx-preview-0.18.0-x86_64-unknown-linux-gnu.tar.gz`
- `imx-preview-0.18.0-aarch64-unknown-linux-gnu.tar.gz`

macOS v0.18.0 archives or tap blocks require recorded local/manual proof before
being claimed. No Windows, crates.io, Homebrew/core, or package-manager
distribution beyond the `jskoiz/imx` tap is claimed. The v0.18.0 release URL is:

```text
https://github.com/jskoiz/imx/releases/tag/v0.18.0
```

The release-attached `imx.rb` is the formula source used to update the
`jskoiz/homebrew-imx` tap from the published `SHA256SUMS`. For v0.18.0, Linux
x86_64 and Linux arm64 tap blocks were generated from the release checksums and
verified by Linux-only tap smoke.

Or install the current source tree directly:

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
imx identify [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>
imx identify --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>
imx report --json [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>
imx resize <width>x<height> [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm> \
  [FORMAT:]<output.bmp|output.ff|output.farbfeld|output.jpg|output.jpeg|output.qoi|output.pbm|output.pgm|output.png|output.ppm>
imx resize-fit <width>x<height> [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm> \
  [FORMAT:]<output.bmp|output.ff|output.farbfeld|output.jpg|output.jpeg|output.qoi|output.pbm|output.pgm|output.png|output.ppm>
imx batch-convert --to <BMP|FARBFELD|JPEG|QOI|PBM|PGM|PNG|PPM> --output-dir <dir> \
  [--resize <width>x<height>|--resize-fit <width>x<height>] \
  [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm>...
imx self-test
imx [FORMAT:]<input.bmp|input.ff|input.farbfeld|input.jpg|input.jpeg|input.qoi|input.pbm|input.pgm|input.png|input.ppm> \
  [FORMAT:]<output.bmp|output.ff|output.farbfeld|output.jpg|output.jpeg|output.qoi|output.pbm|output.pgm|output.png|output.ppm>
```

Supported exact prefixes are `BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
`PGM:`, `PNG:`, and `PPM:`. `JPG:` is intentionally not a supported prefix. Prefixes
are accepted only on `identify`, `resize`, `resize-fit`, `batch-convert` input
operands, and two-path transcode operands. They
are stripped before file IO, must match the detected input format or output
path extension, and do not add extensionless output selection. Unknown,
missing-path, and mismatched prefixes fail with `error: ...`; same-path
rejection still compares the stripped real paths.

Successful `identify` prints one stable key-value line:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

Successful `identify --json` prints one deterministic JSON object:

```json
{"schema_version":1,"format":"PPM","width":2,"height":1,"channels":"RGB","depth":8}
```

`report --json` prints a single-input status object. On supported inputs it
adds `status="supported"` and `diagnostic_code=null` to the same identify
fields. On unsupported or malformed inputs it exits `0` and prints
`status="unsupported"`, a stable `diagnostic_code`, and a human-readable
`message`. `identify --json` prints the same diagnostic JSON to stderr and
exits `1` on data, IO, malformed input, or validation failures. The JSON schema
does not include file paths, file size, hashes, color profiles, EXIF fields, or
ImageMagick verbose metadata.

Successful transcodes are silent and write the output file. `imx self-test`
prints `self-test: ... ok` progress lines and `self-test: passed` on success.
Data, IO, malformed input, and validation failures exit `1` with `error: ...`;
usage and unsupported command shapes exit `2`.

Successful resize writes the output file silently. Dimensions must be lowercase
`<width>x<height>` with non-zero unsigned 32-bit decimal values. Exact resize
uses the requested dimensions. Resize-fit computes the largest integer output
size that fits inside the requested box while preserving source aspect ratio.
Both commands sample the source pixel at
`floor(((2 * dst + 1) * src) / (2 * dst_size))` on each axis, clamp to the last
source row/column, and copy the full decoded pixel without interpolation.
Existing encoder rules still decide any destination quantization, alpha
rejection, thresholding, or metadata loss.

Successful batch conversion writes one output per input silently. `--to`
accepts only exact uppercase `BMP`, `FARBFELD`, `JPEG`, `QOI`, `PBM`, `PGM`,
`PNG`, or `PPM`. `--output-dir` must name an existing directory. Output names are
`<input-file-stem>.<target-extension>` inside that directory. The full batch is
preflighted before writes: missing inputs, invalid prefixes, duplicate planned
outputs, existing outputs, and same-path outputs fail without committing prior
outputs. Batch conversion does not recurse, expand globs, read stdin, write
stdout, overwrite, or invent collision suffixes.

Same-format rewrites are deterministic decode/re-encode operations for
different input and output paths. They do not preserve source bytes, comments,
whitespace, Netpbm ASCII/binary source form, QOI opcode choices, or other
incidental representation details.

## Format Scope

- BMP: uncompressed Windows BMP identify/decode/encode for 24-bit BGR/RGB and
  32-bit BGRA/RGBA rasters. Top-down and bottom-up input are accepted. Output
  BMP is deterministic bottom-up BI_RGB, with RGB8 written as 24-bit BMP and
  RGBA8/RGBA16 input written as 32-bit BMP. Indexed, RLE, bitfields, OS/2,
  color-table, and high-depth BMP variants are rejected.
- FARBFELD: RGBA16BE identify/decode/encode.
- JPEG: `.jpg`/`.jpeg` identify/decode/encode for 8-bit grayscale and RGB
  baseline or progressive JPEG streams. EXIF Orientation values 1 through 8 are
  read before decode and applied to the returned pixels for both baseline and
  progressive input; `identify` reports oriented dimensions. Output JPEG uses
  fixed quality 90 baseline encoding. Non-opaque alpha inputs are rejected
  instead of silently composited or dropped. Same-format JPEG rewrites are
  deterministic lossy decode/re-encode operations and do not preserve source
  bytes, progressive scan layout, quality, quantization/Huffman tables, chroma
  subsampling, comments, EXIF, ICC, XMP, density, thumbnails, timestamps, or
  other metadata.
- QOI: RGB8/RGBA8 identify/decode/encode.
- PBM: ASCII `P1` and binary `P4` bilevel decode; deterministic binary `P4`
  encode.
- PGM: ASCII `P2` and binary `P5` GRAY8/GRAY16BE decode; deterministic binary
  `P5` encode.
- PNG: static non-interlaced grayscale, RGB, RGBA, and grayscale-alpha PNG
  identify/decode/encode for 8-bit and 16-bit samples. Grayscale-alpha input
  normalizes to RGBA. PNG output is deterministic and does not preserve source
  compression, filter choices, ancillary chunks, profiles, gamma, text, time,
  EXIF, or other metadata.
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
- PNG to QOI quantizes 16-bit PNG samples to 8-bit. PNG grayscale-alpha input
  expands gray into RGB and keeps alpha.
- Any output to JPEG quantizes to 8-bit grayscale or RGB and is lossy. IMX
  rejects non-opaque alpha for JPEG output.
- Any output to BMP preserves alpha only for RGBA output; grayscale and RGB-like
  inputs write 24-bit BMP.
- Any output to PPM drops alpha; any output to PGM drops color/alpha; any
  output to PBM drops color, alpha, and grayscale precision.

Unsupported by design: full ImageMagick CLI parsing, stdin/stdout streaming,
prefixes outside the exact eight listed above, delegates, profiles, color
management, transforms beyond the explicit nearest-neighbor resize commands and
safe batch composition, MagickCore, MagickWand, APNG, indexed/palette
PNG, low-bit PNG, PNG metadata/profile preservation, CMYK/YCCK JPEG, 12-bit
JPEG, arithmetic-coded JPEG, lossless JPEG/JPEG-LS,
JPEG 2000, JPEG XL, JPEG metadata preservation beyond read-only Orientation,
PAM, PFM, indexed/compressed/bitfields/OS/2/high-depth BMP, TIFF, GIF, WebP,
and other image formats.

## Safety Posture

- Product decode/encode paths are safe Rust.
- Runtime dependencies are local IMX crates plus pure-Rust PNG and JPEG codec
  dependencies used by `crates/codecs/png` and `crates/codecs/jpeg`.
- Decoded pixel buffers are capped at 512 MiB, with JPEG decode capped at
  128 MiB to account for decoder working-memory overhead.
- CLI input reads are capped at 513 MiB.
- Output writes use a temp file plus rename, and malformed input does not leave
  the requested output behind.
- Fuzz targets cover BMP, FARBFELD, JPEG, QOI, PNG, and PNM decode/identify
  entrypoints with seeded BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM corpora.

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
IMX_VERSION=v0.18.0 IMX_RELEASE_TARGET=x86_64-unknown-linux-gnu ./scripts/verify-release-archive.sh
```

Verify the Homebrew tap install smoke after the tap update:

```sh
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
imx self-test
test "$(imx --version)" = "imx 0.18.0"
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
Linux archive smoke also records the maximum referenced `GLIBC_*` symbol version
for each published binary and fails if it exceeds `GLIBC_2.34`.

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
The v0.12.0 representative intake reliability contract is tracked in
[docs/v0.12.0-real-world-intake.md](docs/v0.12.0-real-world-intake.md), with
the generated/in-test corpus plan in
[docs/v0.12.0-curated-corpus.md](docs/v0.12.0-curated-corpus.md). The v0.16.0
BMP contract is tracked in [docs/v0.16.0-bmp.md](docs/v0.16.0-bmp.md). The
v0.17.0 self-test and diagnostics contract is tracked in
[docs/v0.17.0-self-test-diagnostics.md](docs/v0.17.0-self-test-diagnostics.md).
The
v0.18.0 JSON identify/report contract is tracked in
[docs/v0.18.0-json-identify-report.md](docs/v0.18.0-json-identify-report.md).
v0.19.0 daily-use corpus hardening is tracked in
[docs/v0.19.0-daily-use-corpus.md](docs/v0.19.0-daily-use-corpus.md).
The
v0.15.0 safe batch conversion contract is tracked in
[docs/v0.15.0-batch-convert.md](docs/v0.15.0-batch-convert.md), the v0.14.0
resize-fit contract remains in
[docs/v0.14.0-resize-fit.md](docs/v0.14.0-resize-fit.md), and the v0.13.0
bounded exact resize contract remains in
[docs/v0.13.0-resize.md](docs/v0.13.0-resize.md). The
v0.11.0 progressive JPEG contract is tracked in
[docs/v0.11.0-progressive-jpeg.md](docs/v0.11.0-progressive-jpeg.md). The
v0.10.0 JPEG EXIF Orientation fixture reliability contract is tracked in
[docs/v0.10.0-real-photo.md](docs/v0.10.0-real-photo.md). The v0.9.0 JPEG implementation contract is tracked in
[docs/v0.9.0-jpeg.md](docs/v0.9.0-jpeg.md). The v0.8.0 implementation contract is tracked in
[docs/v0.8.0-png.md](docs/v0.8.0-png.md). The v0.7.0 high-depth PPM contract is
tracked in [docs/v0.7.0-high-depth-ppm.md](docs/v0.7.0-high-depth-ppm.md). The
published v0.6.0 release checklist remains in
[docs/v0.6.0-release-ready.md](docs/v0.6.0-release-ready.md), and the bounded
prefix compatibility contract remains in
[docs/v0.6.0-compatibility-recommendation.md](docs/v0.6.0-compatibility-recommendation.md).
