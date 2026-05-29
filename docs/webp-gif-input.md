# IMX WebP and GIF Support

This slice originally added WebP and GIF intake. The current surface is broader:
IMX can identify and decode both formats, write lossless still WebP, write
single-frame GIF output through normal transcodes, and write animated GIF output
through `imx assemble`. Animated WebP output remains unsupported.

## Supported Surface

- `imx identify input.webp` and `imx identify WEBP:input.webp`.
- `imx identify input.gif` and `imx identify GIF:input.gif`.
- `imx identify --json` and `imx report --json` over WebP and GIF inputs.
- `imx input.webp output.{bmp,ff,gif,jpg,qoi,pbm,pgm,png,ppm,tif,webp}`.
- `imx input.gif output.{bmp,ff,gif,jpg,qoi,pbm,pgm,png,ppm,tif,webp}`.
- `imx resize`, `imx resize-fit`, and `imx batch-convert` with WebP or GIF
  inputs and any supported writable output format.
- `imx assemble --delay <centiseconds> [--loop <n>] output.gif frame0 frame1 ...`
  for animated GIF output from same-size frames.
- Magic-byte detection (`RIFF....WEBP`, `GIF87a`/`GIF89a`) and extension
  fallback (`.webp`, `.gif`).

## WebP Contract

- Decoding wraps the pure-Rust `image-webp` crate (lossy and lossless).
- The RIFF container must begin with `RIFF`, with a `WEBP` form type at offset 8.
- Images without alpha decode to `RGB8`; images with alpha decode to `RGBA8`,
  matching `image-webp`'s reported pixel layout.
- Animated WebP is decoded as its first frame by default; individual frames can
  be enumerated and extracted via the multi-frame decode surface (see
  [`docs/multiframe-decode.md`](multiframe-decode.md)).
- The decoder memory limit is set to `MAX_PIXEL_BYTES`, and dimensions are
  validated with checked size math before any pixel buffer is allocated.

## GIF Contract

- Decoding wraps the `gif` crate with RGBA color output.
- The header must be `GIF87a` or `GIF89a`.
- **Frame 0 is decoded by default.** Animated/multi-frame GIFs can be
  enumerated and a single composited frame extracted via the multi-frame decode
  surface (frame disposal `Keep`/`Background`/`Previous` is honored); see
  [`docs/multiframe-decode.md`](multiframe-decode.md). Frame delays and loop
  counts are still not interpreted (this is frame extraction, not playback).
- Each frame is composited at its declared offset onto a transparent
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

- No animated WebP encoding.
- No arbitrary frame-delay or loop-playback semantics for decode-time frame
  extraction; animation timing is only written by `imx assemble`.
- No metadata, ICC profile, EXIF, or XMP preservation.
- No new command shapes beyond identify, report, transcode, resize, resize-fit,
  batch-convert, and `assemble`.

## Required Evidence

- Codec unit tests for RGB and RGBA WebP decode, first-frame GIF decode,
  multi-frame GIF first-frame selection, offset-frame compositing, and
  malformed/truncated rejection for both formats.
- CLI tests for `.webp`/`.gif` and `WEBP:`/`GIF:` identify, transcodes into PNG,
  WebP/GIF output, batch-convert WebP output, and animated GIF assembly.
- Malformed-input coverage in `tests/malformed/mod.rs` and per-codec fuzz
  targets `fuzz/fuzz_targets/webp_decode.rs` and
  `fuzz/fuzz_targets/gif_decode.rs`.
