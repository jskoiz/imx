# imx-codec-webp

WebP decoder for the [`imx`](https://github.com/jskoiz/imx) image toolkit: a
fast, memory-safe, differentially-verified image conversion library and CLI for
Rust.

This crate builds on [`imx-core`](https://crates.io/crates/imx-core) and turns
WebP bytes into the format-agnostic `imx_core::Image` model. WebP is supported
as an input-only format.

## Why trust it

- **Differentially verified.** Decoding is tested against the real ImageMagick
  binary as an oracle.
- **Deterministic.** The same input always produces byte-identical output.
- **Memory-safe and bounded.** Pure safe Rust with allocations capped by
  `imx_core::MAX_PIXEL_BYTES`.

## License

Distributed under the ImageMagick License. See [`LICENSE`](LICENSE).
