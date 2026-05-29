# imx-codec-tiff

TIFF decoder and encoder for the [`imx`](https://github.com/jskoiz/imx) image
toolkit: a fast, memory-safe, differentially-verified image conversion library
and CLI for Rust.

This crate builds on [`imx-core`](https://crates.io/crates/imx-core) and turns
TIFF bytes into the format-agnostic `imx_core::Image` model and back.

## Why trust it

- **Differentially verified.** Round-trips are tested against the real
  ImageMagick binary as an oracle.
- **Deterministic.** The same input always produces byte-identical output:
  little-endian uncompressed baseline TIFF with no timestamps.
- **Memory-safe and bounded.** Pure safe Rust with allocations capped by
  `imx_core::MAX_PIXEL_BYTES`.

## Scope

Single-image TIFF only: the decoder reads the first IFD and ignores any
additional pages. Supported pixel layouts are 8-bit grayscale, 16-bit
grayscale, 8-bit RGB, 16-bit RGB, and 8-bit RGBA.

## License

Distributed under the ImageMagick License. See [`LICENSE`](LICENSE).
