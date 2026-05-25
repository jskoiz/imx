# IMX FARBFELD/QOI/PBM/PGM/PPM Compatibility Contract

This contract covers only the standalone developer-preview slice.

## Product Boundary

- Product name: IMX Developer Preview.
- Cargo package: `imx-preview`.
- Shipped binary: `imx`.
- Runtime dependencies: none on ImageMagick, MagickCore, MagickWand, delegates,
  modules, policy.xml, or autotools.
- Oracle dependency: ImageMagick may be invoked by tests and benchmarks only.
- Public install surfaces: GitHub release archives, the one-command archive
  installer, and the `jskoiz/imx` Homebrew tap for v0.4.0 archives.
- No Homebrew/core formula is claimed.

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

`identify` outputs:

```text
format=<FORMAT> width=<WIDTH> height=<HEIGHT> channels=<GRAY|RGB|RGBA> depth=<1|8|16>
```

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

PPM:

- ASCII `P3` and binary `P6` RGB8 PPM are supported.
- Header comments before the raster are accepted.
- `maxval` must be in `1..=255`; samples are scaled to 8-bit when `maxval` is
  below 255.
- High-depth PPM is intentionally out of scope.

## Transcode Rules

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

- PPM RGB8 expands to opaque RGBA when the destination has alpha.
- PPM to QOI emits RGBA8 QOI.
- PPM to PGM uses the Rec.709 luma rule above.

PGM to FARBFELD/QOI/PPM:

- Gray samples replicate into RGB channels.
- Alpha is opaque where the destination has alpha.
- PGM to QOI emits RGBA8 QOI.
- PGM to PPM emits RGB8 PPM.

FARBFELD/QOI to PPM:

- Alpha is dropped.
- FARBFELD 16-bit samples are quantized to 8-bit.

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

The v0.4.0 release adds `scripts/differential-corpus.sh` as a report-producing
ImageMagick oracle lane. It generates the deterministic fixture corpus, runs
`imx identify` for FARBFELD, QOI, PBM, PGM, and PPM fixtures, then checks all
20 directed cross-format transcodes between the five supported formats.

Each transcode result is decoded through ImageMagick to canonical 8-bit RGBA
raw pixels and compared with the ImageMagick oracle output for the same source
and destination format. The report emits:

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
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No PAM/PFM support.
- No high-depth PPM support.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- No format beyond FARBFELD, QOI, PBM, PGM, and PPM.
- No Windows, Linux arm64, crates.io, Homebrew/core, or package-manager
  distribution beyond the `jskoiz/imx` Homebrew tap is claimed for v0.4.0.
