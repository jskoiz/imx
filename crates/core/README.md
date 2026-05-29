# imx-core

Core image model and pixel-format conversions for the
[`imx`](https://github.com/jskoiz/imx) image toolkit: a fast, memory-safe,
differentially-verified image conversion library and CLI for Rust.

`imx-core` is the codec-free foundation the `imx` CLI and the per-format codec
crates build on. It provides the format-agnostic `Image` type plus
deterministic, byte-identical conversions between pixel formats (bilevel,
GRAY8/GRAY16BE, RGB8/RGB16BE, RGBA8/RGBA16BE) and bounded nearest-neighbor
resize.

## Why trust it

- **Differentially verified.** Conversions are tested against the real
  ImageMagick binary as an oracle, so behavior is checked against a mature
  reference implementation rather than only against itself.
- **Deterministic.** The same input always produces byte-identical output.
- **Memory-safe and bounded.** Pure safe Rust. Every allocation is capped by
  `MAX_PIXEL_BYTES` and performed through `try_reserve_exact`, so malformed or
  hostile dimensions return an error instead of aborting on a failed
  allocation.
- **Fuzzed.** The wider `imx` project runs per-codec fuzzing and a
  malformed-input corpus against the decode/identify entrypoints that feed this
  model.

## Example

```rust
use imx_core::{Image, PixelFormat};

// A 2x1 RGB8 image: one pure-red pixel, one pure-green pixel.
let rgb = Image::new(2, 1, PixelFormat::Rgb8, vec![255, 0, 0, 0, 255, 0])?;

let gray = rgb.to_gray8()?;
assert_eq!(gray.pixel_format(), PixelFormat::Gray8);
assert_eq!(gray.pixels(), &[54, 182]);
# Ok::<(), imx_core::ImageError>(())
```

## Scope

This crate operates only on already-decoded pixel buffers; it never reads or
writes files. Concrete container formats (PNG, JPEG, BMP, QOI, Netpbm,
farbfeld) are handled by the separate `imx` codec crates and the `imx` CLI.

## License

Licensed under the ImageMagick License. See the bundled `LICENSE` file.
