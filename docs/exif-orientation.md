# IMX EXIF Orientation Auto-Rotation Slice

Status: developer-preview surface. Orientation support is claimed after core
unit tests (all eight values), per-codec unit tests, and CLI integration tests
pass.

Cameras record the physical sensor rotation in the EXIF/TIFF `Orientation` tag
(tag `0x0112` / `274`) rather than re-laying-out the pixels, so a portrait photo
is stored as a landscape raster plus an orientation flag. `imx` reads that flag
on decode and applies the matching transform so the returned image is upright by
default.

## Scope

- **JPEG**: the `Orientation` tag is read from the APP1/EXIF segment using the
  mature [`kamadak-exif`](https://crates.io/crates/kamadak-exif) crate.
- **TIFF**: the `Orientation` tag is read from the first IFD via the `tiff`
  crate's tag accessor.
- All other formats are unaffected; they carry no EXIF orientation and decode
  exactly as before.

Auto-orientation is applied on **decode**, so it flows through every command
that decodes an input: `identify`, `identify --json`, `report --json`,
`compare`, transcode, `resize`, `resize-fit`, `crop`, `rotate`, `flip`, `flop`,
and `batch-convert`.

## Default Behavior and the `--no-auto-orient` Flag

Auto-orientation is **on by default**. Pass the global `--no-auto-orient` flag
(anywhere before the operands) to disable it and operate on the raw stored
pixels and dimensions:

```sh
imx identify photo.jpg                 # upright dimensions (default)
imx --no-auto-orient identify photo.jpg  # raw stored dimensions
imx photo.jpg out.png                  # upright PNG (default)
imx --no-auto-orient photo.jpg out.png # PNG with the raw stored raster
```

Because orientations 5–8 swap the two axes, `identify` on a rotated input now
reports the **upright** width and height by default. The `stable_line()` /
JSON field format is unchanged; only the dimension values change for rotated
inputs, which is the corrected, upright result. With `--no-auto-orient`, the raw
stored dimensions are reported instead.

## The Eight Orientation Values

The transform is implemented in `imx_core::apply_exif_orientation`, built
entirely from the existing `Image` rotate/flip helpers, and shared by both the
JPEG and TIFF codecs.

| Value | Meaning             | Transform                          | Swaps axes |
|-------|---------------------|------------------------------------|------------|
| 1     | Top-left            | identity (no-op)                   | no         |
| 2     | Top-right           | mirror horizontal (`flop`)         | no         |
| 3     | Bottom-right        | rotate 180                         | no         |
| 4     | Bottom-left         | mirror vertical (`flip`)           | no         |
| 5     | Left-top            | transpose (rotate 90 CW + `flop`)  | yes        |
| 6     | Right-top           | rotate 90 CW                       | yes        |
| 7     | Right-bottom        | transverse (rotate 90 CW + `flip`) | yes        |
| 8     | Left-bottom         | rotate 270 CW                      | yes        |

Rotations are clockwise, matching `imx rotate`.

## Robustness

- A **missing** orientation tag, an **out-of-range** value (anything outside
  `1..=8`), or **malformed** EXIF/TIFF metadata is treated as orientation `1`
  (no-op). Decoding never fails on orientation metadata alone, and never panics
  — hostile or non-conforming inputs simply decode without rotation.
- All allocation and size math reuses the checked
  `pixel_count` / `pixel_len` / `try_vec_with_capacity` helpers and stays
  bounded by `MAX_PIXEL_BYTES`, since the transform is expressed through the
  existing `Image` rotate/flip methods.

## Determinism

The transform is a pure pixel permutation with no interpolation or color
management, so output is byte-for-byte deterministic. Most generated fixtures
carry orientation `1`, so their behavior is unchanged; only fixtures with a
non-identity orientation tag now decode upright.
