# IMX Multi-Frame / Animation Decode

This slice adds the ability to enumerate the frames of an animated GIF or WebP
and to decode a single selected frame as the fully composited canvas at that
point in the animation. It is a **decode-only** capability: IMX still cannot
write animation, and GIF remains an input-only format. Animated WebP output
(encode) is likewise unsupported — only frame extraction on decode is provided.

## Supported Surface

- `imx report --json [FORMAT:]<input>` reports the frame count in a new
  additive `frames` field (see [Report schema](#report-json-schema-bump)).
- `imx identify [--frame <N>] <input>` and
  `imx identify --json [--frame <N>] <input>` validate that frame `N` exists.
- `imx report --json [--frame <N>] <input>` validates frame `N`.
- `imx [--frame <N>] [--quality <Q>] <input> <output>` extracts frame `N` from
  the input before transcoding to the output format. `--frame` and `--quality`
  may be supplied in either order.
- `--frame <N>` is a 0-based frame index and defaults to `0`. It is rejected
  for negative or non-numeric values (usage error, exit 2).

Non-animated formats (BMP, farbfeld, JPEG, PNG, Netpbm, QOI, TIFF, and
still WebP/GIF) report `frames` = 1 and accept only `--frame 0`; any `N > 0`
returns a clean `FrameIndexOutOfRange` error (exit 1).

## Codec API

Both the GIF and WebP codec crates expose the same two entry points alongside
the existing single-frame `decode`:

```rust
/// Number of frames in the input (>= 1 for a well-formed image).
pub fn frame_count(input: &[u8]) -> Result<u32, ImageError>;

/// The Nth (0-based) fully composited frame as an `Image`.
pub fn decode_frame(input: &[u8], index: u32) -> Result<Image, ImageError>;
```

`decode(input)` is preserved and is equivalent to `decode_frame(input, 0)`,
returning frame 0. An out-of-range `index` returns the new error variant
`ImageError::FrameIndexOutOfRange { index, frame_count }` (diagnostic code
`image.frame_index_out_of_range`). All buffers are allocated through
`imx_core::{pixel_len, try_vec_with_capacity, MAX_PIXEL_BYTES}`, so malformed or
truncated input never panics or triggers unbounded allocation.

## GIF disposal compositing

A GIF frame is a (possibly sub-rectangle) image drawn onto the logical-screen
canvas. To produce frame `N` as the displayed canvas, IMX decodes frames
`0..=N` in order and composites each onto an RGBA8 canvas, honoring:

- **Transparency.** The `gif` crate emits RGBA where the transparent index has
  alpha 0. Fully transparent source pixels do not overwrite the canvas.
- **Disposal `Keep` / `Any`.** The frame stays on the canvas; the next frame
  draws over it.
- **Disposal `Background`.** After the frame, its rectangle is cleared back to
  transparent before the next frame draws. (A composited RGBA canvas has no
  opaque background color, so "restore to background" means transparent.)
- **Disposal `Previous`.** Before drawing a frame with this disposal, IMX
  snapshots the canvas region the frame covers; after the frame, the region is
  restored to that snapshot so the following frame sees the pre-frame state.

Disposal of frame `k` is applied immediately before frame `k+1` is composited,
which is why the requested frame is returned as soon as it is drawn (its own
disposal does not affect its displayed pixels). Frame geometry is bounds-checked
against the logical screen; a frame that extends beyond the canvas, or whose
buffer length does not match its declared dimensions, is rejected with a clean
error.

`frame_count` walks the GIF block stream via the `gif` crate's
`next_frame_info`, counting image frames without decoding pixel data.

## WebP capability and limits

WebP animation is handled by the installed `image-webp` 0.2.4, which fully
supports the animation API:

- `frame_count` returns `WebPDecoder::num_frames()` for animated WebP (clamped
  to at least 1) and `1` for still images.
- `decode_frame` uses `WebPDecoder::read_frame`, which composites each
  animation frame internally — honoring per-frame offsets, alpha blending,
  disposal, and the canvas background — and writes the displayed canvas into the
  output buffer. To reach frame `N`, IMX reads frames `0..=N` sequentially on a
  fresh decoder and keeps the last buffer.
- Still WebP is decoded via `read_image` (frame 0 only).

The output pixel format follows the WebP canvas: RGBA8 when the file declares
alpha, RGB8 otherwise. If a future pinned `image-webp` were to drop the
animation API, the graceful-degradation contract is: `frame_count` returns 1,
`decode_frame(_, 0)` returns the still image, and any `index > 0` returns
`FrameIndexOutOfRange`. With 0.2.4 the full animation path is active.

`image-webp` 0.2.4 has no animation **encoder**, so the codec's animated-WebP
test fixtures are hand-assembled from per-frame lossless VP8L bitstreams wrapped
in a `VP8X`/`ANIM`/`ANMF` container.

## Report JSON schema bump

The frame count is exposed only through `imx report --json`; the stable
`identify` line and `identify --json` output are unchanged (still
`schema_version` 1). The report schema gains an additive `frames` field and its
version is bumped to **2**:

```json
{"schema_version":2,"status":"supported","diagnostic_code":null,"format":"GIF","width":2,"height":2,"channels":"RGBA","depth":8,"frames":3}
```

Unsupported reports also carry `schema_version` 2:

```json
{"schema_version":2,"status":"unsupported","diagnostic_code":"image.frame_index_out_of_range","message":"failed to identify GIF input: frame index 9 out of range: image has 3 frame(s)"}
```

The `identify --json` error envelope is a separate schema and remains at
`schema_version` 1.

## Determinism

Frame selection is deterministic: extracting the same frame from the same input
twice yields byte-identical output. Compositing is integer-exact (no blending
math beyond GIF transparent-pixel skipping; WebP blending is performed by
`image-webp` deterministically).

## Examples

```sh
# How many frames?
imx report --json anim.gif
# -> {... ,"frames":3}

# Extract the 3rd frame (0-based) to PNG.
imx --frame 2 anim.gif frame2.png

# Out-of-range frame fails cleanly (exit 1).
imx --frame 9 anim.gif out.png
# error: failed to decode GIF input ...: frame index 9 out of range: image has 3 frame(s)
```
