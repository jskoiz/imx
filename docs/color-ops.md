# IMX Color/Tone Operations

Status: developer-preview surface. Color/tone operations are exposed through the
`imx pipeline` command and the corresponding `imx_core::Image` methods. Every
operation is deterministic (byte-identical across runs and platforms),
format-preserving (the pixel format and dimensions are never changed), bounded
by `MAX_PIXEL_BYTES`, and never panics.

## Pipeline ops

These slot into `imx pipeline [FORMAT:]<input> [FORMAT:]<output> --op <op>
[--op <op> ...]`. Ops apply left-to-right, so order matters.

| `--op` value | Core method | Parameter range |
|---|---|---|
| `grayscale` | `Image::grayscale()` | none |
| `invert` | `Image::invert()` | none |
| `brightness:<N>` | `Image::brightness(N)` | integer `-255..=255` |
| `contrast:<F>` | `Image::contrast(F)` | finite float `>= 0` |
| `gamma:<F>` | `Image::gamma(F)` | finite float `> 0` |
| `threshold:<N>` | `Image::threshold(N)` | integer `0..=255` |
| `levels:<black>,<white>,<gamma>` | `Image::levels(black, white, gamma)` | `0 <= black < white <= 255`, gamma finite `> 0` |

The CLI parses and validates each `--op` spec before any decoding happens. A
malformed or out-of-range spec is a usage error and exits with code 2 without
writing output. Operational errors that can only surface after decode use the
standard `failed to <op>` path and exit 1.

## Operation semantics

All operations leave the alpha channel untouched (when the pixel format has one)
and act only on color channels.

- **grayscale** — Desaturates RGB(A) using Rec.709 luma
  (`0.212656 R + 0.715158 G + 0.072186 B`, round-half-up). Each color channel is
  replaced by the per-pixel luma; alpha is preserved. Grayscale inputs
  (`Gray8`, `Gray16Be`) and `Bilevel` are already monochrome and are returned
  unchanged.
- **invert** — Replaces each color channel `v` with `255 - v` (8-bit) or
  `65535 - v` (16-bit). Alpha is not inverted.
- **brightness** — Adds the delta to each color channel and clamps to the
  channel range. Alpha is not changed.
- **contrast** — Scales each color channel around the midpoint:
  `out = (v - mid) * factor + mid`, clamped to range. `factor < 1` reduces
  contrast, `> 1` increases it, `1.0` is a no-op. Alpha is not changed.
- **gamma** — Applies `out = (v / max)^(1/value) * max` on normalized channels.
  `value > 1` brightens midtones, `value < 1` darkens them. Channel endpoints
  (0 and max) are fixed points. Alpha is not changed.
- **threshold** — Binarizes each color channel: values `< level` become the
  channel minimum (0), values `>= level` become the channel maximum. Alpha is
  not changed. This is a per-channel threshold, not a luma threshold; to
  binarize by luma, chain `grayscale` first.
- **levels** — Clamps each color channel to `[black, white]`, linearly rescales
  that window to the full range, then applies gamma. Equivalent to a black
  point, white point, and midtone gamma adjustment. Alpha is not changed.

## 8-bit vs 16-bit behavior

Parameters are expressed in 8-bit terms for a single, predictable knob across
bit depths. For 16-bit formats (`Gray16Be`, `Rgb16Be`, `Rgba16Be`) they are
scaled into 16-bit space so the visual effect is equivalent:

- **invert** uses `65535 - v` (full 16-bit range).
- **brightness** scales the delta by 257 (the 8-bit→16-bit replication factor),
  so `brightness:1` adds `257` to each 16-bit color channel.
- **contrast** scales around the 16-bit midpoint `32768`.
- **gamma** operates on channels normalized by `65535`.
- **threshold** scales `level` by 257, so `threshold:128` compares against
  `32896` in 16-bit space.
- **levels** scales `black` and `white` by 257; gamma is applied in normalized
  space.

`grayscale` collapses 16-bit color channels using a 16-bit Rec.709 luma, keeping
full precision.

## Bilevel behavior

`Bilevel` stores one byte per pixel as `0` or `255`. Because it is already
monochrome:

- `grayscale` returns it unchanged.
- `invert`, `brightness`, `contrast`, `gamma`, `threshold`, and `levels` apply
  their 8-bit transform to the stored `0`/`255` value and then re-threshold the
  result so the buffer stays a valid bilevel buffer (values remain `0` or
  `255`). For example, `invert` on bilevel swaps black and white.

## Determinism

All floating-point intermediates are rounded with a deterministic round-half-up
rule and clamped to the channel range, so repeated runs and runs on different
platforms produce byte-identical output. Running a chain of ops through a single
`imx pipeline` invocation is equivalent to applying the same ops via sequential
subcommands.
