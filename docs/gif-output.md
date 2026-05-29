# GIF output

GIF is now a full input/output format in IMX. This document describes the
encoder added in `imx-codec-gif` and how it is wired into the CLI.

## Scope

- **Single still frame through normal transcodes.** `imx <input> <output.gif>`
  writes one frame with a deterministic palette.
- **Animated GIF through `assemble`.** `imx assemble --delay <centiseconds>
  [--loop <n>] <output.gif|GIF:-> <frame0> <frame1> ...` writes a deterministic
  animated GIF from same-size input frames.
- GIF is a valid output target for `imx <input> <output>`, geometry operations
  (`resize`, `resize-fit`, `crop`, `rotate`, `flip`, `flop`), `pipeline`, and
  `batch-convert --to GIF`.
- GIF decode supports composited frame selection through `--frame`; animation
  playback timing is not interpreted on decode.

## Pipeline

1. The source image is normalized to RGBA8 via `Image::to_rgba8()` (mirroring the
   WebP encode path), so every codec input format funnels through one
   representation.
2. Pixels are mapped onto an indexed palette of at most 256 colors.
3. The palette and per-pixel indices are written as a single frame using the
   `gif` crate's `Encoder`.

All allocations are bounded and checked through
`imx_core::{pixel_len, try_vec_with_capacity, MAX_PIXEL_BYTES}`; the encoder never
panics on hostile input. GIF dimensions are 16-bit, so widths or heights beyond
65535 are rejected with an `UnsupportedFormat` error rather than truncated.

## Palette and quantization

The encoder chooses between two deterministic strategies:

- **Exact palette (lossless).** Distinct opaque colors are collected in sorted
  order. If the source uses at most 256 colors (255 when transparency is present,
  to reserve a slot), those exact RGB values become the palette and every pixel
  maps to its exact color. Small known-palette images therefore round-trip
  byte-for-byte in color.
- **NeuQuant (lossy).** When the source exceeds the palette budget, a
  `color_quant::NeuQuant` color map is built with a **fixed sample factor** (10).
  NeuQuant trains only on opaque pixels so transparent pixels do not pull palette
  colors toward the placeholder. A fixed sample factor makes the network training
  fully deterministic.

## Determinism

Encoding the same image twice always produces byte-identical output:

- Distinct colors are gathered in a stable sorted order, so the exact palette and
  the index assignment do not depend on iteration or hashing order.
- NeuQuant is seeded only by the input pixels and a constant sample factor, with
  no randomness or time/thread-dependent state.

This is asserted by unit tests in `crates/codecs/gif/src/lib.rs`
(`encode_is_deterministic`, `encode_quantizes_many_colors_deterministically`) and
by the CLI integration test `gif_output_is_deterministic`.

## Transparency

GIF supports a single fully transparent palette index (no partial alpha):

- If any source pixel is fully transparent (alpha 0), one palette slot is reserved
  and flagged transparent; all alpha-0 pixels map to it.
- Opaque pixels (alpha > 0) keep their RGB; the alpha channel is dropped (GIF has
  no partial transparency).
- If there are no fully transparent pixels, no transparency is emitted.
