# IMX ICC Color-Profile Passthrough Slice

Status: developer-preview surface. ICC support is claimed after core unit tests
(field round-trip, equality, geometry-preserves, conversion-drops), per-codec
round-trip unit tests, and CLI integration tests pass.

An ICC color profile describes how the raw pixel samples in an image map to
absolute color. Containers embed it as opaque bytes; `imx` carries those bytes
through unchanged so a transcode or geometry edit does not silently discard the
image's color management. `imx` never parses, validates, or applies the
profile — it is preserved verbatim.

## Per-format support

| Format | Decode (extract) | Encode (write back) | Container location |
|--------|------------------|---------------------|--------------------|
| PNG    | yes              | yes                 | `iCCP` chunk |
| JPEG   | yes              | yes                 | `APP2` segments prefixed `ICC_PROFILE\0` (chunked at ≤65519 bytes each) |
| TIFF   | yes              | yes                 | tag `34675` (`ICCProfile`) |

All other formats (BMP, farbfeld, GIF, Netpbm, QOI, WebP) carry no ICC profile
in `imx`; they decode with no profile and encode without one.

### Notes and limitations

- **JPEG**: the profile is reassembled from one or more `APP2` segments on
  decode (via the `jpeg-decoder` crate) and re-chunked into `APP2` segments on
  encode (via the `jpeg-encoder` crate's `add_icc_profile`). Profiles requiring
  255 or more segments (roughly larger than 16 MB) cannot be written and surface
  as a normal encode error. Because JPEG encoding is lossy, only the profile
  bytes round-trip exactly, not the pixels.
- **TIFF**: the profile is written as a `BYTE`-typed array under tag `34675`.
  The `tiff` crate reads it back verbatim through `into_u8_vec`.
- **PNG**: the `png` 0.18 `Encoder` has no direct profile setter, so the profile
  is threaded through an `Info` value via `Encoder::with_info`.

## Decode / encode behavior

- **Decode** attaches the embedded profile to the in-memory `Image` for the
  three supported formats. A missing, empty, or malformed profile tag yields no
  profile rather than failing the decode.
- **Encode** writes `image.icc()` back when present and the output format
  supports it. Encoding a profile-carrying image to a format without ICC support
  simply omits it.

## Geometry preserves, colorspace conversion drops

The profile describes the *current* pixel encoding, so:

- **Geometry transforms preserve it.** `resize`, `resize-fit`, `crop`, `rotate`,
  `flip`, and `flop` rearrange pixels without changing their encoding, so the
  profile remains valid and is carried forward. EXIF/TIFF auto-orientation, which
  is implemented in terms of these transforms, likewise preserves the profile.
- **Colorspace conversions drop it.** `to_rgba8`, `to_rgba16be`, `to_rgb8`,
  `to_rgb16be`, `to_gray8`, `to_gray16be`, and `to_bilevel` re-encode the samples
  into a different representation that the source profile no longer describes, so
  the profile is intentionally dropped. (An identity conversion — converting to
  the format the image already has — is a no-op clone and keeps the profile.)

## The `--strip` flag

Pass the global `--strip` flag (anywhere before the operands) to drop the
embedded ICC profile before encoding:

```
imx --strip in.png out.png        # transcode without the profile
imx --strip rotate 90 in.tiff out.tiff
imx --strip batch-convert --to PNG --output-dir out/ *.jpg
```

`--strip` is consulted in the encode path, so it applies uniformly to the single
transcode, the geometry subcommands, `pipeline`, and `batch-convert`. It has no
effect on `identify`, `report`, or `compare`, which do not encode.
