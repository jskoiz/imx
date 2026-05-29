# Resampling filters

`imx resize` and `imx resize-fit` support a family of separable resampling
filters selected with the global `--filter` flag:

```
imx [--filter <point|box|triangle|catmull-rom|lanczos3>] resize <geometry> <input> <output>
imx [--filter <point|box|triangle|catmull-rom|lanczos3>] resize-fit <width>x<height> <input> <output>
```

The flag is also honored by the `pipeline` `resize:`/`resize-fit:` ops and by
`batch-convert --resize`/`--resize-fit`, so a single `--filter` choice applies to
every resize in the invocation.

## Default

The default filter is **`lanczos3`**, a high-quality windowed-sinc kernel. This
is a deliberate product decision: most callers want professional-quality
resampling, not nearest-neighbor. Callers that need the previous byte-exact
behavior pass `--filter point` explicitly.

## Kernels

Filtering is implemented as a separable two-pass resample — horizontal first,
then vertical — using the named reconstruction kernel. Each output sample is a
normalized weighted sum of the contributing source samples; weights always sum
to one so flat regions are preserved exactly.

| Filter        | Kernel                         | Support | Notes |
|---------------|--------------------------------|---------|-------|
| `point`       | nearest neighbor               | n/a     | Byte-exact; routes to the existing center-sampled `resize_nearest`. |
| `box`         | box / averaging                | 0.5     | Simple area average; good for integer downscales. |
| `triangle`    | linear (bilinear)              | 1.0     | Smooth, low ringing. |
| `catmull-rom` | Catmull-Rom bicubic (B=0, C=½) | 2.0     | Sharper interpolating cubic. |
| `lanczos3`    | windowed sinc (a = 3)          | 3.0     | Highest quality; mild ringing on hard edges. |

When downscaling (target smaller than source), the kernel support is widened by
`1/scale` so the filter averages the correct source neighborhood and avoids
aliasing. When upscaling, the unit-support kernel is used directly. Source edges
are handled by clamping the contributing index range to the valid source span
and renormalizing the surviving weights.

## Byte-exact `point` path

`--filter point` is guaranteed to produce byte-for-byte identical output to the
historical nearest-neighbor `resize`/`resize-fit`. Internally it routes directly
to `Image::resize_nearest`, which uses center-sampled coordinate mapping. This
path is covered by core unit tests
(`resize_filtered_point_matches_resize_nearest_byte_for_byte`) and by CLI tests
that assert exact pixels with `--filter point`.

## Determinism

All filtered paths are fully byte-deterministic across platforms and runs:

- Samples are processed in a normalized floating-point working space (8-bit
  formats in `0..=255`, 16-bit big-endian formats in `0..=65535`).
- The pixel format and channel count of the input are preserved on output.
- Results are quantized back to the source bit depth with a fixed **round
  half-up** rule (`floor(value + 0.5)`) and clamped to the channel range. There
  is no platform-dependent float rounding in the quantization step.
- `Bilevel` inputs are resampled in their `0`/`255` byte space and then
  re-thresholded so the output remains a valid bilevel buffer.

Identical inputs and arguments always yield identical output bytes; this is
asserted by `resize_filtered_is_byte_deterministic` (core) and
`resize_filter_is_byte_deterministic` (CLI).

## Bounded allocation

Both the floating-point working buffers and the output pixel buffer are sized
through checked reservation and bounded by `MAX_PIXEL_BYTES`, mirroring every
other `imx-core` operation. Hostile or overflowing dimensions return an
`ImageError` rather than panicking or allocating without limit.

## Differential-tolerance rationale

The differential test suite compares each filter against ImageMagick's matching
`-filter` (`Box`, `Triangle`, `Catrom`, `Lanczos`). Matching ImageMagick
byte-for-byte on non-`Point` kernels is impractical and not a goal:

- ImageMagick resamples in a linear-light pipeline with its own windowing,
  EWA/cylindrical handling, and quantum rounding.
- `imx` uses a deterministic separable two-pass resampler in the gamma-encoded
  sample space with round-half-up quantization.

The tests therefore assert a bounded **max-abs-diff** and **mean-absolute-error
(MAE)** rather than exact equality. Current bounds (8-bit RGB, decoded via the
oracle):

| Filter        | max-abs-diff | MAE |
|---------------|--------------|-----|
| `box`         | 24           | 4.0 |
| `triangle`    | 24           | 4.0 |
| `catmull-rom` | 32           | 5.0 |
| `lanczos3`    | 32           | 5.0 |

These bounds are loose enough to absorb pipeline differences yet tight enough to
catch kernel selection, axis-orientation, or normalization regressions. The
`point` path is held to exact equality against ImageMagick `-filter Point`.
The oracle tests skip cleanly when the `magick` binary is not present.
