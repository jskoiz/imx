# IMX WebP and GIF Input Support

This slice adds WebP and GIF as **input-only** formats: IMX can identify and
decode them, and transcode from them into any existing supported output format.
Neither format can be used as an output target.

## Supported Surface

- `imx identify input.webp` and `imx identify WEBP:input.webp`.
- `imx identify input.gif` and `imx identify GIF:input.gif`.
- `imx identify --json` and `imx report --json` over WebP and GIF inputs.
- `imx input.webp output.{bmp,ff,jpg,qoi,pbm,pgm,png,ppm}`.
- `imx input.gif output.{bmp,ff,jpg,qoi,pbm,pgm,png,ppm}`.
- `imx resize`, `imx resize-fit`, and `imx batch-convert` with WebP or GIF
  inputs (the output format must be a supported writable format).
- Magic-byte detection (`RIFF....WEBP`, `GIF87a`/`GIF89a`) and extension
  fallback (`.webp`, `.gif`).

## WebP Contract

- Decoding wraps the pure-Rust `image-webp` crate (lossy and lossless).
- The RIFF container must begin with `RIFF`, with a `WEBP` form type at offset 8.
- Images without alpha decode to `RGB8`; images with alpha decode to `RGBA8`,
  matching `image-webp`'s reported pixel layout.
- Animated WebP is decoded as its first frame only.
- The decoder memory limit is set to `MAX_PIXEL_BYTES`, and dimensions are
  validated with checked size math before any pixel buffer is allocated.

## GIF Contract

- Decoding wraps the `gif` crate with RGBA color output.
- The header must be `GIF87a` or `GIF89a`.
- **Only the first frame is decoded.** Animation and multi-frame GIFs are out of
  scope: subsequent frames are ignored, and frame delays, disposal methods, and
  loop counts are not interpreted.
- The first frame is composited at its declared offset onto a transparent
  logical-screen canvas, so `identify` and `decode` always agree on the reported
  width and height. The output pixel format is always `RGBA8`.
- A per-frame memory limit of `MAX_PIXEL_BYTES` is enforced.

## Safety and Determinism

- All pixel buffers are allocated through `try_vec_with_capacity` after checked
  `pixel_len` validation, honoring `MAX_PIXEL_BYTES`.
- Decoder errors are mapped into `ImageError`; oversized images map to
  `ImageError::ImageTooLarge`, and unrecognized headers map to
  `ImageError::InvalidHeader`.
- Decoding is deterministic: the same bytes always produce the same image.

## Non-Goals

- No WebP or GIF **encoding**; both are input-only.
- No GIF animation, multi-frame composition, frame-delay handling, or loop
  semantics.
- No metadata, ICC profile, EXIF, or XMP preservation.
- No new command shapes beyond the existing identify/transcode/resize,
  resize-fit, and batch-convert surface.

## Required Evidence

- Codec unit tests for RGB and RGBA WebP decode, first-frame GIF decode,
  multi-frame GIF first-frame selection, offset-frame compositing, and
  malformed/truncated rejection for both formats.
- CLI tests for `.webp`/`.gif` and `WEBP:`/`GIF:` identify, transcode into PNG,
  and rejection of WebP/GIF as transcode and batch-convert output targets.
- Malformed-input coverage in `tests/malformed/mod.rs` and per-codec fuzz
  targets `fuzz/fuzz_targets/webp_decode.rs` and
  `fuzz/fuzz_targets/gif_decode.rs`.
