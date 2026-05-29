# Animated GIF output

IMX can write animated GIFs. This closes the loop with the existing multi-frame
GIF decoder (`docs/multiframe-decode.md`): frames that can be extracted on decode
can now also be assembled into an animation. This document describes the
`encode_animation` entry point in `imx-codec-gif` and the `imx assemble` CLI
subcommand.

## The `assemble` command

```
imx [--no-auto-orient] assemble --delay <centiseconds> [--loop <n>] <output.gif|GIF:-> <frame0> <frame1> ...
```

- `--delay <centiseconds>` (required) sets a uniform inter-frame delay applied to
  every frame, in centiseconds (1/100 s). GIF stores delay as a 16-bit value, so
  `--delay 50` is half a second per frame.
- `--loop <n>` (optional, default `0`) sets the loop count written as a Netscape
  looping extension. `0` means loop forever; any other value loops that many
  times.
- The first positional argument is the GIF output path (or `GIF:-` to write to
  stdout). The output must resolve to the GIF format; any other target is an
  error.
- Every remaining positional argument is an input frame, decoded with the normal
  IMX decode machinery, so frames may be any supported input format (PNG, JPEG,
  BMP, …) and may even be GIFs — the first composited frame of each input is
  used.

Frames are written in the order given. At least one frame is required, and all
frames must share identical width and height; a mismatch is rejected with a clean
error.

### Exit codes

- Usage errors (missing `--delay`, missing output/frames, malformed flag values)
  exit `2`.
- Operational errors (a missing input file, an undecodable frame, a
  dimension mismatch, dimensions beyond the GIF 16-bit limit, write failures)
  exit `1`.

### Example

```
imx assemble --delay 50 --loop 0 out.gif f0.png f1.png f2.png
imx report --json out.gif        # "frames": 3
imx --frame 1 out.gif frame1.png # extract the middle frame back out
```

## Per-frame palette approach

Each frame is quantized **independently** to its own local palette of at most 256
colors, using exactly the same deterministic strategy as the single-frame
`encode` path (exact palette when the frame uses few enough distinct colors,
otherwise `NeuQuant` with a fixed sample factor; see `docs/gif-output.md`). The
encoder writes one image block per frame, each carrying its own color table; no
shared global palette is used.

This is the simplest correct choice and keeps every frame faithful to its own
colors: a per-frame palette never has to compromise across frames. The trade-off
is a slightly larger file than a tuned shared global palette would produce, which
is an acceptable cost for determinism and simplicity.

## Determinism

`encode_animation` is byte-deterministic: assembling the same frames with the
same `--delay` and `--loop` twice yields identical bytes. This follows from the
per-frame quantization being deterministic (stable sorted exact palettes; a fixed
NeuQuant sample factor with no randomness or time/thread state) and from the
delay and loop extension being written from fixed inputs. Unit tests in
`crates/codecs/gif/src/lib.rs` (`encode_animation_is_deterministic`,
`encode_animation_round_trips_three_frames`) and CLI integration tests cover this.

## Bounds and safety

All allocations are bounded and checked through
`imx_core::{pixel_len, try_vec_with_capacity, MAX_PIXEL_BYTES}`, the same helpers
the single-frame encoder uses. The encoder never panics on hostile input. GIF
dimensions are 16-bit, so a frame wider or taller than 65535 pixels is rejected
with an `UnsupportedFormat` error rather than truncated.

## Out of scope

- **Animated WebP output.** IMX can decode and extract WebP animation frames but
  does not write animated WebP; that remains unsupported.
- **Per-frame delays and disposal methods.** `assemble` applies a single uniform
  delay to all frames and uses the default disposal. Variable per-frame timing,
  GIF disposal-method selection, and frame offsets are out of scope.
