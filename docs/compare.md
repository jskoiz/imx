# `imx compare`

`imx compare` is a deterministic, scriptable image diff. It decodes two inputs,
compares them at the pixel level, and reports how they differ. It is designed
for CI use: assert that a freshly produced image matches a golden, and inspect
the magnitude of any difference.

```
imx compare [--metric <ae|mae|psnr>] [FORMAT:]<a|FORMAT:-> [FORMAT:]<b>
```

Each operand uses the same `[FORMAT:]<path>` routing as `imx identify` and
`imx <input> <output>`, including format-prefix enforcement and `FORMAT:-`
stdin streaming. At most one of the two operands may be read from stdin.

## Comparison model

1. Both inputs are decoded to the in-memory `Image` model.
2. If the two images differ in **dimensions**, a single deterministic line is
   printed and the tool exits `1` without attempting a pixel diff:

   ```
   differ: dimensions 64x64 vs 32x32
   ```

3. If the dimensions match but the **channel layout** differs (e.g. `RGB`
   vs `GRAY`), a single line is printed and the tool exits `1`:

   ```
   differ: channels RGB vs GRAY
   ```

4. Otherwise both images are normalized to a common `RGBA8` representation and
   compared byte-for-byte across all four channels of every pixel.

   - If the normalized buffers are identical, `identical` is printed and the
     tool exits `0`.
   - Otherwise a summary line is printed and the tool exits `1`:

     ```
     differ: 3/4096 pixels ae=200 mae=0.048828
     ```

     where the fields are: number of differing pixels / total pixels, the peak
     absolute per-channel difference (`ae`), and the mean absolute error
     (`mae`) across all RGBA channels.

Normalizing to `RGBA8` means an `RGB8` image and an explicitly-opaque `RGBA8`
image with the same colors compare as identical.

## Metrics (`--metric`)

When `--metric` is supplied, only a single number is printed to stdout, which
is convenient for scripting:

| metric | meaning | identical inputs |
| ------ | ------- | ---------------- |
| `ae`   | peak absolute per-channel difference (0..=255) | `0` |
| `mae`  | mean absolute error across all RGBA channels, fixed 6 decimals | `0.000000` |
| `psnr` | peak signal-to-noise ratio in dB over 8-bit channels | `inf` |

`psnr` is defined in the standard way over 8-bit channels:
`10 * log10(255^2 / MSE)`, where `MSE` is the mean squared per-channel error.
For identical inputs the error is zero, so `psnr` prints `inf`.

The exit code with `--metric` follows the same rule as the default summary: `0`
when the inputs are identical, `1` when they differ.

## Exit codes

| code | meaning |
| ---- | ------- |
| `0`  | images are identical |
| `1`  | images differ (dimensions, channels, or pixels) or an operational/decode error occurred |
| `2`  | usage error (missing operand, unknown `--metric` value, both operands stdin) |

## Determinism guarantees

All output is byte-deterministic. There are no timestamps, no locale-dependent
number formatting, and floating-point metrics are printed with explicit fixed
precision (`{:.6}`). Running `imx compare` twice on the same inputs always
produces byte-identical stdout and the same exit code.
