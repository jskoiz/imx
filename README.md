# IMX

A fast, memory-safe, **differentially-verified** image conversion library and
CLI for Rust. IMX decodes, identifies, transcodes, and resizes images through a
small `imx` binary and a set of focused codec crates, with byte-identical,
deterministic output you can rely on in pipelines and tests.

IMX is **not** a fork or port of ImageMagick. It shares no code with it, links
nothing from it, and does not provide `convert`, `magick`, or `mogrify`.
ImageMagick is used only as an external oracle in IMX's own compatibility
tests and benchmarks.

## Why trust it

This is the headline. IMX is built to be trustworthy on real and hostile input:

- **Differential testing against the real ImageMagick binary as an oracle.**
  Every codec's identify/decode/transcode behavior is checked against
  ImageMagick's output, so IMX is verified against a mature reference
  implementation rather than only against itself.
- **Per-codec fuzzing and a malformed-input corpus.** Coverage-guided fuzz
  targets exercise the BMP, FARBFELD, JPEG, QOI, PNG, and Netpbm decode/identify
  entrypoints, backed by seeded corpora and a malformed-input suite, with crash
  artifacts retained from scheduled long-running fuzz runs.
- **Deterministic, byte-identical output.** The same input always produces the
  same bytes. Output writes use a temp file plus atomic rename, so a malformed
  input never leaves a partial output behind.
- **Checked, bounded allocation.** Decoded pixel buffers are capped
  (`MAX_PIXEL_BYTES` = 512 MiB; JPEG decode capped at 128 MiB for decoder
  overhead) and every allocation goes through `try_reserve_exact`, so hostile
  dimensions return an error instead of aborting. CLI input reads are capped at
  513 MiB.
- **Memory-safe by construction.** The product decode/encode paths are safe
  Rust. Runtime dependencies are the local IMX crates plus pure-Rust PNG and
  JPEG codecs.

## Install

Install the verified tap release:

```sh
brew tap jskoiz/imx
brew install imx
imx --version
```

This uses the `jskoiz/homebrew-imx` tap formula generated from each published
release's `SHA256SUMS`. It is not a Homebrew/core formula. Published Linux
archives require glibc 2.34 or newer; release/archive smoke asserts that
published Linux binaries do not reference `GLIBC_*` symbols newer than
`GLIBC_2.34`. Hosted GitHub Actions for the tap are Linux-only; macOS install
proof must be run locally or manually after explicit approval.

Install the published release archive directly:

```sh
IMX_VERSION=v0.19.0
curl -fsSL "https://raw.githubusercontent.com/jskoiz/imx/${IMX_VERSION}/scripts/install.sh" | sh
```

The installer verifies the published `SHA256SUMS`, installs `imx`, asserts the
installed version, checks for glibc 2.34 or newer on Linux, and runs a small
`imx self-test` plus identify/report JSON and
identify/transcode/resize/resize-fit/batch-convert smoke. Hosted tag automation
publishes Linux archives for:

- `imx-preview-0.19.0-x86_64-unknown-linux-gnu.tar.gz`
- `imx-preview-0.19.0-aarch64-unknown-linux-gnu.tar.gz`

macOS archives or tap blocks require recorded local/manual proof before being
claimed. No Windows, crates.io, Homebrew/core, or package-manager distribution
beyond the `jskoiz/imx` tap is claimed for the binary.

Or install the current source tree directly:

```sh
git clone https://github.com/jskoiz/imx.git
cd imx
cargo install --path crates/cli --bin imx --locked
imx --version
```

The source install path is verified by `scripts/verify-install.sh` from a fresh
checkout in CI.

## Quick examples

```sh
# Identify an image (one stable key-value line)
imx identify photo.png
# format=PNG width=1920 height=1080 channels=RGB depth=8

# Identify as deterministic JSON
imx identify --json photo.png
# {"schema_version":1,"format":"PNG","width":1920,"height":1080,"channels":"RGB","depth":8}

# Transcode between formats
imx photo.png photo.jpg

# JPEG output quality
imx --quality 85 photo.png photo.jpg

# Exact and aspect-preserving resize
imx resize 800x600 photo.png thumb.png
imx resize-fit 800x600 photo.png thumb.png

# Geometric operations
imx crop 100x100+10+10 photo.png cropped.png
imx rotate 90 photo.png rotated.png
imx flip photo.png flipped.png
imx flop photo.png flopped.png

# Streaming through stdin/stdout with a FORMAT: prefix
cat photo.png | imx PNG:- JPEG:- > photo.jpg

# Batch conversion to a directory
imx batch-convert --to PNG --output-dir out/ a.jpg b.bmp c.qoi

# Offline self-test of the installed binary
imx self-test
```

Exact uppercase format prefixes (`BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
`PGM:`, `PNG:`, `PPM:`) may be attached to operands to assert the expected
format; they are stripped before file IO and must match the detected input
format or output extension. (`JPG:` is intentionally not a prefix.)

## Supported formats & operations

### Container formats

| Format    | Extensions / prefix      | Input | Output | Notes |
| --------- | ------------------------ | :---: | :----: | ----- |
| PNG       | `.png` / `PNG:`          |  yes  |  yes   | Static non-interlaced 8/16-bit grayscale, RGB, RGBA, grayscale-alpha. No APNG, interlace, indexed/palette, low-bit, or metadata preservation. |
| JPEG      | `.jpg`, `.jpeg` / `JPEG:`|  yes  |  yes   | 8-bit grayscale/RGB baseline + progressive input. Read-only EXIF Orientation (values 1-8) normalized on decode. Output is baseline JPEG; quality selectable via `--quality 1..=100`. |
| BMP       | `.bmp` / `BMP:`          |  yes  |  yes   | Uncompressed 24-bit BGR/RGB and 32-bit BGRA/RGBA; top-down and bottom-up input. No indexed/RLE/bitfields/OS2/high-depth. |
| QOI       | `.qoi` / `QOI:`          |  yes  |  yes   | RGB8 / RGBA8. |
| FARBFELD  | `.ff`, `.farbfeld` / `FARBFELD:` | yes | yes | RGBA16BE. |
| PBM       | `.pbm` / `PBM:`          |  yes  |  yes   | ASCII `P1` + binary `P4` bilevel in; deterministic binary `P4` out. |
| PGM       | `.pgm` / `PGM:`          |  yes  |  yes   | ASCII `P2` + binary `P5` GRAY8/GRAY16BE in; deterministic binary `P5` out. |
| PPM       | `.ppm` / `PPM:`          |  yes  |  yes   | ASCII `P3` + binary `P6` RGB8/RGB16BE in (`maxval` up to 65535); deterministic binary `P6` out. |
| WebP      | input only               |  yes  |   no   | Decode / identify / transcode-from. Encoding is not supported. |
| GIF       | input only               |  yes  |   no   | Decode / identify / transcode-from. Encoding is not supported. |

### Operations

| Operation             | Command                                | Notes |
| --------------------- | -------------------------------------- | ----- |
| Identify              | `imx identify [--json] <input>`        | Stable key-value line, or deterministic JSON. |
| Report                | `imx report --json <input>`            | Identify fields plus `status` and `diagnostic_code`. |
| Transcode             | `imx <input> <output>`                 | Cross-format and deterministic same-format rewrite. |
| Exact resize          | `imx resize <w>x<h> <input> <output>`  | Center-sampled nearest-neighbor to exact dimensions. |
| Aspect-preserving fit | `imx resize-fit <w>x<h> <in> <out>`    | Largest integer box that preserves aspect ratio. |
| Crop                  | `imx crop ...`                         | Extract a rectangular region. |
| Rotate                | `imx rotate <90\|180\|270> ...`        | Right-angle rotation. |
| Flip / Flop           | `imx flip ...` / `imx flop ...`        | Vertical / horizontal mirror. |
| JPEG quality          | `--quality <1..=100>`                  | Quality for JPEG output. |
| Streaming             | `FORMAT:-`                             | Read stdin / write stdout via a `-` operand with a `FORMAT:` prefix. |
| Batch convert         | `imx batch-convert --to <FMT> --output-dir <dir> [--resize\|--resize-fit] <input>...` | Preflighted; deterministic output names; no overwrite/recurse/glob. |
| Self-test             | `imx self-test`                        | Offline installed-binary smoke check. |

Successful transcodes and resizes are silent and write the output file. Data,
IO, malformed-input, and validation failures exit `1` with `error: ...`; usage
and unsupported command shapes exit `2`.

### Known lossy paths

- FARBFELD/PPM/PNG to QOI quantizes 16-bit samples to 8-bit.
- FARBFELD/QOI/PPM/PGM to PBM uses Rec.709 luma where needed, then thresholds
  `<128` / `<32768` to black and higher values to white.
- FARBFELD to PGM converts RGBA16BE to GRAY16BE via Rec.709 luma, ignoring
  alpha.
- 8-bit QOI/PBM/PGM/PPM to FARBFELD expands samples to 16-bit by byte
  replication and adds opaque alpha where needed; high-depth PGM/PPM keeps
  16-bit samples.
- Any output to JPEG quantizes to 8-bit grayscale or RGB and is lossy; non-opaque
  alpha is rejected for JPEG output.
- Any output to BMP preserves alpha only for RGBA output.
- Output to PPM drops alpha; output to PGM drops color/alpha; output to PBM
  drops color, alpha, and grayscale precision.

Same-format rewrites are deterministic decode/re-encode operations and do not
preserve source bytes, comments, whitespace, Netpbm ASCII/binary form, QOI
opcode choices, or other incidental representation details.

## Not yet supported

By design, IMX does not (yet) provide:

- WebP or GIF **output** (both are input-only).
- Full ImageMagick CLI parsing, the `magick`/`convert`/`mogrify` commands,
  delegates, MagickCore, or MagickWand.
- Color management, ICC profiles, and general metadata preservation (beyond
  read-only JPEG EXIF Orientation).
- APNG, interlaced PNG, indexed/palette PNG, low-bit PNG, `tRNS`.
- CMYK/YCCK JPEG, 12-bit JPEG, arithmetic-coded JPEG, lossless JPEG / JPEG-LS,
  JPEG 2000, JPEG XL.
- Indexed/compressed/bitfields/OS2/high-depth BMP.
- PAM, PFM, TIFF, and other container formats not listed above.
- Filters beyond nearest-neighbor resize and the listed geometric operations.

## Safety posture

- Product decode/encode paths are safe Rust.
- Runtime dependencies are the local IMX crates plus pure-Rust PNG and JPEG
  codec dependencies used by `crates/codecs/png` and `crates/codecs/jpeg`.
- Decoded pixel buffers are capped at 512 MiB, with JPEG decode capped at
  128 MiB to account for decoder working-memory overhead.
- CLI input reads are capped at 513 MiB.
- Output writes use a temp file plus rename; malformed input does not leave the
  requested output behind.
- Fuzz targets cover BMP, FARBFELD, JPEG, QOI, PNG, and PNM decode/identify
  entrypoints with seeded corpora.

## Library

The core image model is published as the `imx-core` crate
([`crates/core`](crates/core)). It provides the codec-free `Image` type and the
deterministic pixel-format conversions used by the CLI and codec crates:

```rust
use imx_core::{Image, PixelFormat};

let rgb = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 255, 0])?;
let gray = rgb.to_gray8()?;
assert_eq!(gray.pixels(), &[54, 182]);
# Ok::<(), imx_core::ImageError>(())
```

The codec and CLI crates depend on `imx-core` by path and are not yet published
individually.

## Release gates

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

Release archives use deterministic tar/gzip metadata and aggregate
`SHA256SUMS` entries so repeated packaging of the same built payload is
byte-for-byte comparable. For v0.6.0 and later tags, hosted Linux release
automation packages `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`
archives. No hosted macOS or iOS runner is used for release proof.

Verify source installation from a fresh checkout:

```sh
IMX_INSTALL_REPO_URL=https://github.com/jskoiz/imx.git ./scripts/verify-install.sh
```

Verify published Linux release archives after GitHub release publication:

```sh
IMX_VERSION=v0.19.0 IMX_RELEASE_TARGET=x86_64-unknown-linux-gnu ./scripts/verify-release-archive.sh
```

Verify the Homebrew tap install smoke after the tap update:

```sh
brew tap jskoiz/imx
brew install imx
brew test imx
imx --version
imx self-test
test "$(imx --version)" = "imx 0.19.0"
```

`brew test` verifies installation only. Compatibility remains covered by the CI
differential corpus, fuzz, benchmark, and conformance gates.

## Documentation

See [COMPATIBILITY.md](COMPATIBILITY.md) for the exact behavior contract and
[PRODUCTION_READINESS.md](PRODUCTION_READINESS.md) for current release evidence,
known gaps, and the next adoption milestone.

## Version history

IMX is developed one verified slice at a time. Each release adds a tightly
scoped, fully tested capability rather than a broad surface.

- **v0.19.0** — Daily-use corpus hardening gate. `scripts/daily-use-corpus.sh`
  runs a real `imx` binary against generated fixtures for JSON identify/report,
  representative prefixed transcodes, stable unsupported diagnostics, and
  `identify --json` failure JSON. A no-oracle install/package/release confidence
  gate, not a new format or command surface.
- **v0.18.0** — Machine-readable daily-use surface: `imx identify --json` and
  `imx report --json`, with deterministic output limited to the existing
  identify metadata plus report `status`/`diagnostic_code`.
- **v0.17.0** — `imx self-test`, an offline installed-binary smoke check, plus
  tightened CLI diagnostics and exit-code tests.
- **v0.16.0** — Bounded uncompressed Windows BMP support for 24-bit BGR/RGB and
  32-bit BGRA/RGBA rasters.
- **v0.15.0** — Safe `imx batch-convert` with preflighted, deterministic output
  names and no overwrite/recurse/glob.
- **v0.14.0** — `imx resize-fit`, aspect-preserving nearest-neighbor resize.
- **v0.13.0** — `imx resize`, center-sampled nearest-neighbor exact resize.
- **v0.12.0** — Representative generated/in-test intake reliability coverage for
  already-supported formats.
- **v0.11.0** — Bounded progressive JPEG input for 8-bit grayscale/RGB.
- **v0.10.0** — Read-only JPEG EXIF Orientation handling (values 1-8).
- **v0.9.0** — `.jpg`/`.jpeg` and `JPEG:` support for 8-bit grayscale/RGB JPEG
  identify and transcode.
- **v0.8.0** — Static non-interlaced PNG identify/decode/encode for 8/16-bit
  grayscale, RGB, RGBA, and grayscale-alpha rasters.
- **v0.7.0** — High-depth PPM (`maxval` up to 65535).
- **v0.6.0** — Exact uppercase format-prefix surface and the first published
  release checklist.

Per-version contracts are tracked under [docs/](docs/), including the
[v0.16.0 BMP](docs/v0.16.0-bmp.md),
[v0.17.0 self-test/diagnostics](docs/v0.17.0-self-test-diagnostics.md),
[v0.18.0 JSON identify/report](docs/v0.18.0-json-identify-report.md), and
[v0.19.0 daily-use corpus](docs/v0.19.0-daily-use-corpus.md) contracts, with the
earlier resize, JPEG, and PNG contracts alongside them.
