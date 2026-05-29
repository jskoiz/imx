# Resize geometry shorthand

`imx resize` accepts the geometry shorthands developers expect from
ImageMagick, in addition to the original exact `<width>x<height>` form. All
forms compute concrete target dimensions and then reuse the existing
nearest-neighbor `resize_nearest` engine, so output stays byte-deterministic
and bounded by `MAX_PIXEL_BYTES`.

## Supported geometry forms

| Geometry            | Meaning                                                        | Example (`100x40` source) |
| ------------------- | -------------------------------------------------------------- | ------------------------- |
| `<width>x<height>`  | Exact target dimensions (unchanged behavior).                  | `50x20` -> `50x20`        |
| `<width>x`          | Width fixed; height derived from the source aspect ratio.      | `200x` -> `200x80`        |
| `x<height>`         | Height fixed; width derived from the source aspect ratio.      | `x10` -> `25x10`          |
| `<percent>%`        | Scale both axes uniformly by an integer percentage.            | `50%` -> `50x20`          |

Derived axes and percentage scaling use ImageMagick's round-half-up integer
rule (the same `round_scaled_dimension` helper used by `resize-fit`) and are
clamped to a minimum of one pixel, matching `magick -resize`.

## Rounding

Percent scaling computes `round(source * percent / 100)` with ties rounded up,
then clamps to at least `1`. Single-axis scaling computes the missing dimension
as `round(source_other_axis * fixed_axis / source_fixed_axis)`, again rounded
half up and clamped to at least `1`. These rules mirror ImageMagick's
`-resize` geometry handling so differential tests against the oracle match
pixel-for-pixel under the `Point` filter.

## Errors

Malformed geometry (for example `50%%`, `abc`, `0x10`, `x0`, `1.5x2`, or
trailing/leading whitespace) is rejected before any image is decoded and the
CLI exits with status `2` (usage error). The core parser surfaces a
`GeometryError` whose diagnostic code is `image.invalid_geometry`.

## Scope

The shorthands apply to `imx resize`. `imx resize-fit` and the
`batch-convert --resize`/`--resize-fit` options continue to require an exact
`<width>x<height>` bounding box, because their aspect-preserving fit semantics
already cover the single-axis use case and a percentage box would be
ambiguous against a max-dimension fit.

## Core API

The dimension math lives in `crates/core`:

- `ResizeGeometry` — parsed geometry (`Exact`, `Width`, `Height`, `Percent`).
- `ResizeGeometry::parse(&str) -> Result<ResizeGeometry, GeometryError>`.
- `ResizeGeometry::resolve(source_width, source_height) -> Result<(u32, u32), ImageError>`.
- `scale_percent(value, percent) -> u32` — round-half-up percent scaling with a
  minimum of one.

The CLI parsing stays thin: it parses the geometry, decodes the image, resolves
target dimensions against the decoded source size, and calls `resize_nearest`.
