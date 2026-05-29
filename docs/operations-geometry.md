# IMX Geometric Operations Slice

Status: developer-preview surface. Geometry support is claimed for the existing
supported formats only, after core unit tests and ImageMagick differential
checks pass (the oracle lane skips when `magick`/`convert` is not installed).

## Supported Surface

- Commands:
  - `imx crop <width>x<height>+<x>+<y> [FORMAT:]<input> [FORMAT:]<output>`
  - `imx rotate <90|180|270> [FORMAT:]<input> [FORMAT:]<output>`
  - `imx flip [FORMAT:]<input> [FORMAT:]<output>`
  - `imx flop [FORMAT:]<input> [FORMAT:]<output>`
- Formats: BMP, FARBFELD, JPEG, QOI, PBM, PGM, PNG, and PPM only.
- Prefixes: exact uppercase `BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
  `PGM:`, `PNG:`, and `PPM:` keep the existing confirm-only behavior.

## Geometry Parsing

- `crop` uses ImageMagick-style geometry `<width>x<height>+<x>+<y>`. All four
  components are lowercase unsigned 32-bit decimals. `width` and `height` must
  be non-zero. Garbage, missing components, signed values, or non-decimal
  characters are rejected with usage exit code 2.
- `rotate` accepts exactly `90`, `180`, or `270`. Any other value, including
  `0`, `360`, or negatives, is rejected with usage exit code 2.

## Behavior Contract

Each command decodes the input, applies the operation on raw pixel bytes using
the source pixel format's `bytes_per_pixel()`, then runs the existing
destination encoder. Operations are byte-for-byte deterministic and
format-preserving (no bit-depth change, no interpolation, no color management,
no metadata preservation). Allocation and size math use the checked
`pixel_count`/`pixel_len`/`try_vec_with_capacity` helpers capped at
`MAX_PIXEL_BYTES`.

- `crop` extracts the rectangle whose top-left corner is `(x, y)` with the given
  `width` and `height`. The region must lie fully within the source bounds; a
  zero dimension or any region that extends past the right or bottom edge fails
  with `ImageError::CropOutOfBounds` (diagnostic code
  `image.crop_out_of_bounds`). Cropping does not pad or clamp; it matches
  ImageMagick `-crop WxH+X+Y +repage` for fully in-bounds regions.
- `rotate 90` and `rotate 270` rotate clockwise and swap width and height.
  `rotate 180` keeps the dimensions and reverses the pixel order. These match
  ImageMagick `-rotate 90/180/270` for these lossless right-angle cases.
- `flip` mirrors top-to-bottom (vertical flip), matching `-flip`.
- `flop` mirrors left-to-right (horizontal flop), matching `-flop`.

Existing encoder rules still decide JPEG loss, QOI/Netpbm quantization, PBM
thresholding, alpha rejection, and metadata loss. Usage errors exit 2;
operation errors (including out-of-bounds crops) exit 1.

## Evidence Requirements

- Core image tests covering width/height swaps, exact pixel placement for small
  2x3 and 3x2 cases, wide pixel-format preservation, single-pixel crops, and
  zero/out-of-bounds crop rejection.
- ImageMagick oracle proof against `-crop WxH+X+Y +repage`, `-rotate
  90/180/270`, `-flip`, and `-flop` with exact decoded pixels for the PPM
  fixture.

## Non-Goals

No new formats, stdin/stdout, batch geometry, arbitrary-angle rotation,
percentage or aspect-ratio geometry, gravity, padding/extent, negative offsets,
metadata preservation, color management, `magick` alias, full ImageMagick CLI
parsing, delegates, MagickCore, or MagickWand.
