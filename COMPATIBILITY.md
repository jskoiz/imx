# IMX FARBFELD/QOI/PBM/PGM/PNG/PPM Compatibility Contract

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
imx identify [FORMAT:]<input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.png|input.ppm>
imx [FORMAT:]<input.ff|input.farbfeld|input.qoi|input.pbm|input.pgm|input.png|input.ppm> \
  [FORMAT:]<output.ff|output.farbfeld|output.qoi|output.pbm|output.pgm|output.png|output.ppm>
```

`identify` outputs:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

## Format Prefix Behavior

IMX accepts exact uppercase ImageMagick-style prefixes for the existing
supported formats only:

- `FARBFELD:input.ff`
- `QOI:input.qoi`
- `PBM:input.pbm`
- `PGM:input.pgm`
- `PNG:input.png`
- `PPM:input.ppm`

Prefixes are a CLI path adapter for `identify` and two-path transcodes only.
They are stripped before file IO, then checked against the detected input format
or output path extension. Unknown uppercase prefixes, empty prefixed paths, and
prefix/format mismatches fail with an `error: ...` message. Output paths still
need a supported extension, so `QOI:output` is not a supported way to select an
extensionless output format. Same-path rejection compares stripped paths.

## Format Behavior

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
  FARBFELD, QOI, PBM, PGM, PNG, and PPM when the paths are different.
- Same-format rewrites are deterministic decode/re-encode operations, not
  source preservation. They may normalize Netpbm source form to deterministic
  binary output, regenerate QOI opcode streams, and drop comments, whitespace,
  padding-bit values, or other incidental representation details.

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

- Decoded pixel buffers are capped at 512 MiB.
- CLI input files larger than 513 MiB are rejected before reading.
- The cap is an IMX safety policy, not ImageMagick parity.

## Corpus Differential Coverage

The compatibility lane keeps `scripts/differential-corpus.sh` as a
report-producing ImageMagick oracle lane. It generates the deterministic fixture
corpus, runs `imx identify` for FARBFELD, QOI, PBM, PGM, PNG, and PPM fixtures,
runs prefixed identify cases for the same six formats, runs additional
high-depth PPM and PNG identify cases, then checks all 36 directed transcodes
between the six supported formats plus a prefixed transcode ring that exercises
every supported prefix as input and output. It also runs high-depth PPM and PNG
transcode cases for 16-bit preserving destinations.

Most transcode results are decoded through ImageMagick to canonical 8-bit RGBA
raw pixels and compared with the ImageMagick oracle output for the same source
and destination format. High-depth PPM cases that should preserve precision are
decoded to canonical 16-bit raw RGB or GRAY samples before comparison. The report
emits:

- `manifest.json` from the generated fixture corpus.
- `results.jsonl` with one row per identify/transcode case.
- `summary.json` with pass/fail counts and evidence paths.

Malformed-input conformance remains covered by golden/malformed unit tests and
fuzz targets rather than by ImageMagick byte-for-byte compatibility. IMX
intentionally rejects several malformed inputs that ImageMagick may accept or
clamp.

## Unsupported Surface

- No full ImageMagick command parser.
- No `magick` binary alias; the shipped command is `imx`.
- No stdin/stdout streaming.
- No prefixes beyond exact `FARBFELD:`, `QOI:`, `PBM:`, `PGM:`, `PNG:`, and
  `PPM:`.
- No PAM/PFM support.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- No APNG, indexed/palette PNG, low-bit PNG, PNG metadata/profile preservation,
  or format beyond FARBFELD, QOI, PBM, PGM, PNG, and PPM.
- No Windows, crates.io, Homebrew/core, or package-manager distribution beyond
  the `jskoiz/imx` Homebrew tap is claimed for this slice. v0.8.1 Linux x86_64
  and Linux arm64 archives require glibc 2.34 or newer; Linux arm64 support is
  claimed only for the published archive and tap block verified from release
  `SHA256SUMS` by Linux-only tap smoke.
