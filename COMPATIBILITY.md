# IMX BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM Compatibility Contract

This contract covers only the standalone developer-preview slice.

## Product Boundary

- Product name: IMX Developer Preview.
- Cargo package: `imx-preview`.
- Shipped binary: `imx`.
- Runtime dependencies: none on ImageMagick, MagickCore, MagickWand, delegates,
  modules, policy.xml, or autotools.
- Oracle dependency: ImageMagick may be invoked by tests and benchmarks only.
- Public install surfaces: GitHub release archives, the one-command archive
  installer, and the `jskoiz/imx` Homebrew tap. The tap formula for a given
  release is generated only from that release's published `SHA256SUMS`; platform
  support is limited to archive targets with tap smoke proof.
- No Homebrew/core formula is claimed.

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

`identify` outputs:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

`identify --json` outputs the same proven identify fields as deterministic JSON:

```json
{"schema_version":1,"format":"PPM","width":2,"height":1,"channels":"RGB","depth":8}
```

`report --json` outputs one single-input status object. Supported inputs add
`status="supported"` and `diagnostic_code=null` to the identify fields.
Unsupported or malformed inputs exit `0` and emit `status="unsupported"`, a
stable `diagnostic_code`, and a human-readable `message`. `identify --json`
uses the same diagnostic JSON on stderr and exits `1` for data, IO, malformed
input, or validation failures. Invalid command shapes still exit `2` with usage
text.

`self-test` creates temporary fixtures and invokes the installed `imx` binary
for unprefixed and prefixed identify, JSON identify/report, cross-format
transcode, same-format resize, same-format resize-fit, and batch-convert across
BMP, FARBFELD, JPEG, QOI, PBM, PGM, PNG, and PPM. It exits `0` and prints
`self-test: passed` only
when every smoke step succeeds. It exits `1` with `error: self-test failed: ...`
if any covered command fails. `self-test` is an offline install confidence check,
not an ImageMagick oracle, fuzz target, benchmark, or full conformance proof.

## Format Prefix Behavior

IMX accepts exact uppercase ImageMagick-style prefixes for the existing
supported formats only:

- `FARBFELD:input.ff`
- `BMP:input.bmp`
- `JPEG:input.jpg`
- `QOI:input.qoi`
- `PBM:input.pbm`
- `PGM:input.pgm`
- `PNG:input.png`
- `PPM:input.ppm`

Prefixes are a CLI path adapter for `identify`, `resize`, `resize-fit`,
`batch-convert` input operands, and two-path transcodes. They are stripped
before file IO, then checked against the detected input format or output path
extension. Unknown uppercase prefixes,
empty prefixed paths, and prefix/format mismatches fail with an `error: ...`
message. Output paths still need a supported extension, so `QOI:output` is not
a supported way to select an extensionless output format. Same-path rejection
compares stripped paths. `JPG:` is not a supported prefix.

## Format Behavior

BMP:

- Magic must be exactly `BM`.
- Only Windows `BITMAPINFOHEADER` (`biSize == 40`) BMP files are supported.
- Compression must be `BI_RGB` (`0`), planes must be `1`, and color-table
  entries are rejected.
- Decode supports 24-bit BGR rows as RGB8 and 32-bit BGRA rows as RGBA8.
- Positive-height bottom-up and negative-height top-down input are accepted.
- Rows are padded to 4-byte boundaries.
- Encode writes deterministic bottom-up Windows BMP with no color table.
- RGB-like input writes 24-bit BGR BMP. RGBA input writes 32-bit BGRA BMP.
- Indexed, RLE-compressed, bitfields, OS/2 headers, color tables, masks,
  high-depth, and profile/metadata BMP semantics are outside this slice.

FARBFELD:

- Magic must be exactly `farbfeld`.
- Width and height must be non-zero.
- Pixel data is RGBA16 big-endian.
- Trailing bytes after the expected payload are accepted for compatibility.

QOI:

- Magic is accepted case-insensitively for ImageMagick compatibility.
- Width and height must be non-zero.
- Channels must be `3` or `4`.
- Colorspace must be `0` or `1`.
- Missing or trailing bytes after enough pixels have decoded are accepted for
  the current compatibility slice.
- Final runs that exceed the remaining declared pixel count are clipped to the
  declared dimensions.

JPEG:

- Magic detection requires the JPEG SOI marker followed by a marker byte:
  `ff d8 ff`.
- `.jpg` and `.jpeg` extensions both map to JPEG. Extension matching is
  case-insensitive.
- Decode and identify support are limited to 8-bit grayscale and RGB baseline
  or progressive JPEG streams. `identify` reports `channels=GRAY depth=8` or
  `channels=RGB depth=8`.
- EXIF Orientation values 1 through 8 are read before decode. IMX normalizes
  decoded pixels to that orientation, and `identify` reports the oriented
  dimensions.
- Encode support writes deterministic 8-bit baseline JPEG with fixed quality
  90.
- JPEG output rejects non-opaque alpha input instead of silently compositing or
  dropping alpha.
- Same-format JPEG rewrites are lossy decode/re-encode operations and do not
  preserve source bytes, progressive scan layout, quality,
  quantization/Huffman tables, chroma subsampling, comments, EXIF, ICC, XMP,
  density, thumbnails, timestamps, or other metadata.
- IMX rejects CMYK/YCCK JPEG and 16-bit JPEG. Arithmetic-coded, lossless
  JPEG/JPEG-LS, JPEG 2000, JPEG XL, progressive output, metadata preservation
  beyond read-only Orientation, profile interpretation, and color-management
  semantics are outside this compatibility slice.

PBM:

- ASCII `P1` and binary `P4` PBM are supported.
- Magic must be uppercase `P1` or `P4`.
- There is no `maxval`; header fields are magic, width, and height.
- Width and height must be non-zero.
- `identify` reports `format=PBM ... channels=GRAY depth=1`.
- In file raster data, `0` means white and `1` means black.
- In IMX core memory, bilevel pixels are stored as one byte per pixel:
  `255` white and `0` black.
- `P1` raster bits may be adjacent or separated by whitespace/comments.
- `P4` rows are packed independently, most-significant bit first, left to
  right.
- Unused `P4` row-padding bits are ignored on decode and zeroed on encode.
- `P4` consumes exactly one whitespace byte after dimensions as the raster
  separator. Bytes after that separator are raster bytes, not comments.
- Trailing bytes after the expected PBM raster are accepted.
- IMX rejects malformed P1 raster bytes such as `2` or `a` even though
  ImageMagick accepts some malformed values.

PGM:

- ASCII `P2` and binary `P5` PGM are supported.
- Magic must be uppercase `P2` or `P5`.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=65535`.
- `P2` samples are decimal tokens and are scaled to GRAY8 when `maxval <= 255`
  or GRAY16BE when `maxval > 255`.
- `P5` consumes exactly one whitespace byte after `maxval` as the raster
  separator. Bytes after that separator are raster bytes, not comments.
- `P5` uses one byte per sample when `maxval <= 255` and two big-endian bytes
  per sample when `maxval > 255`.
- Trailing bytes after the expected PGM raster are accepted.
- IMX rejects over-max ASCII samples, zero dimensions, `maxval=0`, and
  `maxval > 65535` even when ImageMagick accepts or clamps some malformed
  inputs.

PNG:

- PNG signature must be exact.
- Static non-interlaced PNG is supported for grayscale, RGB, RGBA, and
  grayscale-alpha color types with 8-bit or 16-bit samples.
- Grayscale-alpha input normalizes to RGBA by replicating gray into RGB and
  preserving alpha.
- Output PNG is deterministic and written without source ancillary chunks,
  profiles, gamma, text, time, EXIF, or other metadata.
- IMX rejects APNG, interlaced PNG, indexed/palette PNG, low-bit 1/2/4 sample
  PNG, `tRNS` color-key transparency, zero dimensions, oversized decoded
  rasters, invalid CRCs, and truncated PNG streams.
- IMX does not apply PNG color management, ICC profiles, gamma correction, or
  sRGB chunk semantics in this compatibility slice.

PPM:

- ASCII `P3` and binary `P6` RGB PPM are supported.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=65535`.
- `P3` samples are decimal tokens and are scaled to RGB8 when `maxval <= 255`
  or RGB16BE when `maxval > 255`.
- `P6` consumes exactly one whitespace byte after `maxval` as the raster
  separator. Bytes after that separator are raster bytes, not comments.
- `P6` uses one byte per sample when `maxval <= 255` and two big-endian bytes
  per sample when `maxval > 255`.
- Trailing bytes after the expected PPM raster are accepted.
- IMX rejects over-max ASCII or binary samples, zero dimensions, `maxval=0`, and
  `maxval > 65535` even when ImageMagick accepts or clamps some malformed
  inputs.

## Transcode Rules

Same-format rewrites:

- `imx input output` accepts same-format input and output extensions for
  BMP, FARBFELD, JPEG, QOI, PBM, PGM, PNG, and PPM when the paths are different.
- Same-format rewrites are deterministic decode/re-encode operations, not
  source preservation. They may normalize Netpbm source form to deterministic
  binary output, regenerate QOI opcode streams, re-encode BMP row order/padding,
  re-encode JPEG lossily, and drop comments, whitespace, padding-bit values,
  metadata, or other incidental representation details.

BMP to FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM:

- BMP RGB input follows the same RGB conversion rules as other RGB8 sources.
- BMP RGBA input preserves alpha only through alpha-capable destinations.
- BMP to JPEG rejects non-opaque alpha; BMP to PBM/PGM/PPM drops alpha through
  the existing threshold, luma, or RGB output rules.

FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM to BMP:

- Output BMP writes 24-bit BGR for grayscale/RGB-like inputs and 32-bit BGRA
  when the normalized source carries alpha.
- High-depth samples are quantized to 8-bit for BMP output.
- Output BMP does not preserve source metadata, comments, profiles, Netpbm
  source form, JPEG tables, PNG ancillary chunks, or QOI opcode choices.

FARBFELD/QOI/PBM/PGM/PNG/PPM to JPEG:

- JPEG output writes 8-bit grayscale for grayscale-like inputs and 8-bit RGB
  for color inputs.
- High-depth inputs are quantized to 8-bit before JPEG encode.
- Non-opaque alpha is rejected instead of composited or dropped.
- JPEG output is lossy and deterministic for the same normalized input bytes.

JPEG to FARBFELD/QOI/PBM/PGM/PNG/PPM:

- JPEG grayscale input remains gray unless the destination requires RGB/RGBA.
- JPEG RGB input expands to opaque alpha for FARBFELD, QOI, or PNG RGBA output.
- JPEG to PBM/PGM uses the same Rec.709 luma and thresholding rules as other
  color inputs.

## Resize Rules

- `imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>` resizes the
  decoded image to exact dimensions before running the existing destination
  encoder.
- `imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>` resizes
  the decoded image to the largest integer dimensions that fit inside the
  requested box while preserving source aspect ratio, then runs the existing
  destination encoder.
- Dimensions must be lowercase `<width>x<height>` with non-zero unsigned
  32-bit decimal values. `2X2`, `x2`, `2x`, percentages, aspect-ratio
  shorthand, geometry flags, and ImageMagick-style `-resize` forms are
  unsupported command shapes.
- Resize uses center-sampled nearest neighbor. For each destination coordinate,
  IMX samples `floor(((2 * dst + 1) * src_size) / (2 * dst_size))`, clamped to
  the last source coordinate.
- Resize copies the complete decoded pixel value. It does not interpolate,
  composite alpha, convert color, preserve metadata, or choose a new bit depth.
  Existing encoder rules still handle destination quantization, alpha
  rejection, luma thresholding, JPEG loss, and metadata loss.
- Supported resize inputs and outputs are only BMP, FARBFELD, JPEG, QOI, PBM,
  PGM, PNG, and PPM through the existing extension and exact-prefix contract.
- Resize-fit follows ImageMagick point-resize box rounding for this slice:
  choose the width-bound result when `box_width * source_height <= box_height *
  source_width`; otherwise choose the height-bound result. The other dimension
  is rounded half up and clamped to at least one pixel.

## Batch Conversion Rules

- `imx batch-convert --to <FORMAT> --output-dir <dir>
  [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...`
  converts one or more shell-provided input paths using the same decoders,
  optional resize operation, encoders, and exact input-prefix checks as the
  single-file commands.
- `<FORMAT>` must be exactly `BMP`, `FARBFELD`, `JPEG`, `QOI`, `PBM`, `PGM`,
  `PNG`, or `PPM`. It is not an extension alias; `JPG`, `BM`, `ff`, lowercase
  names, and other formats are rejected.
- `--output-dir` must name an existing directory. IMX does not create the
  output directory, walk directories recursively, expand globs, read stdin,
  write stdout, or run batch work in parallel.
- Output paths are derived deterministically as
  `<output-dir>/<input-file-stem>.<target-extension>` using primary extensions
  `.bmp`, `.ff`, `.jpg`, `.qoi`, `.pbm`, `.pgm`, `.png`, and `.ppm`.
- The full batch is preflighted before any output is written. Missing inputs,
  non-file inputs, invalid prefixes, duplicate planned output paths, outputs
  that already exist, and outputs that resolve to the same file as an input
  fail with `error: ...`.
- Batch output is no-overwrite. IMX does not add numeric suffixes, rename
  colliding outputs, or replace existing files. If transform or encode fails
  during preparation, no prepared earlier output is committed.

FARBFELD to QOI:

- RGBA16BE samples are quantized to RGBA8.
- QOI output uses 4 channels and sRGB colorspace.
- This path is lossy unless each 16-bit channel is a repeated 8-bit value such
  as `0x1212` or `0xffff`.

QOI to FARBFELD:

- RGB8 expands to RGBA16BE with opaque alpha.
- RGBA8 expands to RGBA16BE preserving alpha.
- 8-bit samples expand to 16-bit samples by byte replication.

PBM to FARBFELD/QOI/PGM/PPM:

- Black/white samples replicate into gray or RGB channels.
- Alpha is opaque where the destination has alpha.
- PBM to QOI emits RGBA8 QOI.
- PBM to PGM emits GRAY8 PGM.
- PBM to PPM emits RGB8 PPM.

PGM/PPM/FARBFELD/QOI to PBM:

- Alpha is ignored.
- Color converts to grayscale using Rec.709 luma:
  `0.212656 R + 0.715158 G + 0.072186 B`.
- 8-bit grayscale/luma `<128` becomes black; `>=128` becomes white.
- 16-bit grayscale/luma `<32768` becomes black; `>=32768` becomes white.
- PBM output is deterministic binary `P4`.

PPM to FARBFELD/QOI/PGM:

- PPM RGB8/RGB16 expands to opaque RGBA when the destination has alpha.
- PPM to QOI emits RGBA8 QOI; high-depth PPM samples are quantized to 8-bit.
- PPM to PGM uses the Rec.709 luma rule above and emits GRAY8 for RGB8 input or
  GRAY16BE for RGB16 input.

PGM to FARBFELD/QOI/PPM:

- Gray samples replicate into RGB channels.
- Alpha is opaque where the destination has alpha.
- PGM to QOI emits RGBA8 QOI.
- PGM to PPM emits RGB8 PPM for GRAY8 input and RGB16BE PPM for GRAY16BE input.

PNG to FARBFELD/QOI/PBM/PGM/PNG/PPM:

- PNG grayscale input stays gray unless the destination requires RGB/RGBA.
- PNG RGB input expands to opaque alpha for FARBFELD or QOI output.
- PNG RGBA and grayscale-alpha preserve alpha only when the destination has
  alpha; PPM, PGM, and PBM drop alpha.
- PNG16 to QOI or PBM is lossy because QOI is 8-bit and PBM is bilevel.
- PNG16 to FARBFELD preserves 16-bit channel precision. PNG16 to PPM/PGM
  preserves 16-bit precision for retained color or gray channels.

FARBFELD/QOI/PBM/PGM/PNG/PPM to PNG:

- Output PNG uses grayscale, RGB, or RGBA channel layout based on the normalized
  IMX image representation.
- Bilevel PBM output to PNG is encoded as 8-bit grayscale, not 1-bit PNG.
- PNG output does not preserve source comments, Netpbm source form, QOI opcode
  choices, FARBFELD source bytes, or incidental representation details.

FARBFELD/QOI to PPM:

- Alpha is dropped.
- FARBFELD/RGBA16 input emits deterministic RGB16BE PPM with `maxval 65535`.
- QOI/RGBA8 input emits deterministic RGB8 PPM with `maxval 255`.

FARBFELD/QOI to PGM:

- Alpha is ignored.
- RGB/RGBA channels convert to grayscale using the Rec.709 luma rule above.
- FARBFELD writes deterministic GRAY16BE `P5`; QOI writes deterministic GRAY8
  `P5`.

## Resource Policy

- Decoded pixel buffers are capped at 512 MiB, with JPEG decode capped at
  128 MiB to account for decoder working-memory overhead.
- CLI input files larger than 513 MiB are rejected by metadata before the
  bounded read fallback, when metadata is available.
- The cap is an IMX safety policy, not ImageMagick parity.

## Representative Intake Reliability Coverage

The v0.12.0 intake reliability claim does not add formats or command syntax. It
adds evidence for generated and in-test fixtures that represent patterns seen
in real files or failure reports. No externally sourced real-world file corpus
is claimed.

- FARBFELD RGBA16 input with nontrivial channel values.
- BMP bottom-up and top-down RGB24/RGBA32 input.
- Progressive grayscale JPEG and camera-style EXIF Orientation input.
- QOI RGB input with linear colorspace.
- PBM ASCII input with comments and adjacent raster samples.
- PGM scaled ASCII, binary CRLF/comment, and binary 16-bit input.
- PNG grayscale, grayscale-alpha, RGB16, and RGBA16 input.
- PPM ASCII high-`maxval` and binary CRLF/comment input.
- Malformed BMP, FARBFELD, QOI, PBM, PGM, PPM, PNG, and JPEG diagnostics with clear
  operation/path context at the CLI.
- Resource-boundary checks for the 512 MiB decoded-pixel cap without requiring
  large allocations.

This corpus is generated or embedded in tests so no unclear-license external
fixtures are vendored.

## Corpus Differential Coverage

The compatibility lane keeps `scripts/differential-corpus.sh` as a
report-producing ImageMagick oracle lane. It generates the deterministic fixture
corpus, runs `imx identify` metadata parity for BMP, FARBFELD, JPEG, QOI, PBM,
PGM, PNG, and PPM fixtures, runs prefixed identify cases for the same eight
formats, runs additional high-depth PPM and PNG identify cases, and adds
representative intake identify/pixel parity for generated and in-test fixture
families. CLI tests and smoke scripts cover `imx identify --json` and
`imx report --json` as deterministic projections of the same identify metadata,
including supported/unsupported status and diagnostic codes. It then checks directed transcodes between the eight supported
formats plus a prefixed transcode ring that exercises every supported prefix as
input and output. It also checks plain and prefixed nearest-neighbor resize for
every supported format. It runs plain and prefixed nearest-neighbor resize-fit
for every supported format. It runs high-depth PPM and PNG transcode cases for
16-bit preserving destinations and records an `imx self-test` result row.

Most transcode results are decoded through ImageMagick to canonical 8-bit RGBA
raw pixels and compared with the ImageMagick oracle output for the same source
and destination format. High-depth PPM cases that should preserve precision are
decoded to canonical 16-bit raw RGB or GRAY samples before comparison. JPEG
cases are decoded to canonical RGB8 and checked with recorded lossy tolerance
metrics instead of byte equality. Orientation JPEG cases are compared against
ImageMagick `-auto-orient` with the same metric recorder. The report emits:

- `manifest.json` from the generated fixture corpus.
- `results.jsonl` with one row per identify, transcode, resize, resize-fit,
  batch-convert, and batch safety case.
- `jpeg-metrics.jsonl` with max absolute difference, MAE, RMSE, PSNR, p99, and
  threshold counts for JPEG-involved cases.
- `summary.json` with pass/fail counts and evidence paths.

Malformed-input conformance remains covered by golden/malformed unit tests and
fuzz targets rather than by ImageMagick byte-for-byte compatibility.
CLI diagnostic tests cover exit code and `error:` prefix behavior for unknown
prefixes, mismatched prefixes, missing paths, unsupported variants, invalid
geometry, same-path outputs, batch output-directory failures, and unsupported
command-shape usage. JSON diagnostic tests cover unknown prefixes, missing
prefixed paths, prefix mismatches, missing inputs, malformed QOI, and JSON
identify error output.
`scripts/curated-corpus.sh` records the v0.12.0 intake corpus summary at
`target/curated-corpus/summary.json` and is run by the local/hosted release
gate. IMX intentionally rejects several malformed inputs that ImageMagick may
accept or clamp.
`scripts/daily-use-corpus.sh` records `target/daily-use-corpus/summary.json`
and proves the current binary against generated fixtures for JSON
identify/report, representative prefixed transcodes, stable unsupported
`report --json` diagnostics, and `identify --json` failure JSON on stderr. This
gate is a no-oracle install/package/release confidence check and does not add
new formats or command shapes.

## Unsupported Surface

- No full ImageMagick command parser.
- No ImageMagick JSON schema compatibility or verbose metadata report.
- No `magick` binary alias; the shipped command is `imx`.
- No stdin/stdout streaming.
- No prefixes beyond exact `BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
  `PGM:`, `PNG:`, and `PPM:`.
- No PAM/PFM support.
- No delegates, profiles, color management, transform operations beyond the
  explicit nearest-neighbor resize commands and safe batch composition,
  MagickCore API, or MagickWand API.
- No APNG, indexed/palette PNG, low-bit PNG, PNG metadata/profile preservation,
  or PNG color-management/profile semantics.
- No CMYK/YCCK JPEG, 12-bit JPEG, arithmetic-coded JPEG, lossless
  JPEG/JPEG-LS, JPEG 2000, JPEG XL, progressive JPEG output, JPEG
  metadata/profile preservation beyond read-only Orientation, or JPEG
  color-management semantics.
- No format beyond BMP, FARBFELD, JPEG, QOI, PBM, PGM, PNG, and PPM, and no BMP
  variants beyond uncompressed Windows 24-bit BGR/RGB and 32-bit BGRA/RGBA.
- No Windows, crates.io, Homebrew/core, or package-manager distribution beyond
  the `jskoiz/imx` Homebrew tap is claimed for this slice. v0.19.0 Linux x86_64
  and Linux arm64 archives require glibc 2.34 or newer; Linux arm64 support is
  claimed only for published archives and release-attached formula blocks
  verified from release `SHA256SUMS`. The v0.19.0 tap claim is verified through
  `jskoiz/homebrew-imx` from those checksums and Linux-only tap smoke.
  Release/archive smoke checks that published Linux binaries do not reference
  `GLIBC_*` symbols newer than `GLIBC_2.34`.
