# imx-codec-gif

GIF codec for the [`imx`](https://github.com/jskoiz/imx) image toolkit: a
fast, memory-safe, differentially-verified image conversion library and CLI for
Rust.

This crate builds on [`imx-core`](https://crates.io/crates/imx-core) and converts
between GIF bytes and the format-agnostic `imx_core::Image` model. Decoding reads
the first frame only (animation is ignored). Encoding writes a single still frame
with a deterministic palette of at most 256 colors: the exact palette is kept when
the source has 256 colors or fewer, otherwise a NeuQuant color map with a fixed
sample factor is used. Fully transparent pixels map to a single reserved
transparent palette index. Animation/multi-frame output is out of scope.

## Why trust it

- **Differentially verified.** Decoding is tested against the real ImageMagick
  binary as an oracle.
- **Deterministic.** The same input always produces byte-identical output,
  including palette-quantized encodes.
- **Memory-safe and bounded.** Pure safe Rust with allocations capped by
  `imx_core::MAX_PIXEL_BYTES`.

## License

Distributed under the ImageMagick License. See [`LICENSE`](LICENSE).
