# TIFF codec

`imx-codec-tiff` decodes and encodes baseline TIFF images and integrates TIFF
as a first-class `imx` format alongside BMP, PNG, JPEG, WebP, GIF, QOI,
farbfeld, and the PNM family.

## Scope

- **Single image only.** The decoder reads the first IFD and ignores any
  additional pages. Multi-page TIFFs decode to their first image.
- **Uncompressed baseline output.** The encoder writes little-endian,
  uncompressed, strip-based baseline TIFF.

## Supported pixel formats

| TIFF sample layout | `imx_core::PixelFormat` |
| ------------------ | ----------------------- |
| 8-bit grayscale    | `Gray8`                 |
| 16-bit grayscale   | `Gray16Be`              |
| 8-bit RGB          | `Rgb8`                  |
| 16-bit RGB         | `Rgb16Be`               |
| 8-bit RGBA         | `Rgba8`                 |

On decode, any other TIFF color type (palette, CMYK, YCbCr, 1-bit bilevel,
floating point, etc.) is rejected with a clean `ImageError::UnsupportedFormat`.

On encode, every `imx_core::PixelFormat` is accepted. Formats without a direct
TIFF mapping are converted to the nearest supported layout before writing:

| Input `PixelFormat` | Encoded as              |
| ------------------- | ----------------------- |
| `Bilevel`           | `Gray8`                 |
| `Gray8`             | `Gray8`                 |
| `Gray16Be`          | `Gray16` (16-bit gray)  |
| `Rgb8`              | `RGB8`                  |
| `Rgb16Be`           | `RGB16` (16-bit RGB)    |
| `Rgba8`             | `RGBA8`                 |
| `Rgba16Be`          | `RGBA8` (down-converted)|

## Determinism

Encoding is byte-deterministic: identical input always yields byte-identical
output. The encoder emits no timestamps or other nondeterministic tags. A
double-encode test in the codec asserts this invariant.

## Safety

Decoding is pure safe Rust and never panics on malformed input. Allocations are
bounded by `imx_core::MAX_PIXEL_BYTES`; oversized images are rejected with
`ImageError::ImageTooLarge`. The fuzz-smoke and truncation test suites cover
TIFF alongside the other codecs.
