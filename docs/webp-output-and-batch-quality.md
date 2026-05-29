# WebP output and batch-convert quality

This change closes two output-side asymmetries.

## WebP output (lossless still frames)

WEBP is now supported for identify, decode, lossless still-frame encode, and
transcode. Previously it was input-only.

`imx_codec_webp::encode(&Image)` wraps `image-webp`'s `WebPEncoder`, which emits
the lossless VP8L container. The `imx-core` `Image` is converted to the layout
the encoder requires:

- Grayscale and RGB sources (`Bilevel`, `Gray8`, `Gray16Be`, `Rgb8`, `Rgb16Be`)
  encode as `ColorType::Rgb8`.
- Alpha-bearing sources (`Rgba8`, `Rgba16Be`) encode as `ColorType::Rgba8`,
  preserving the alpha channel.

16-bit sources are downsampled to 8-bit through the existing `to_rgb8`/`to_rgba8`
conversions, matching how the WEBP decoder reports 8-bit RGB/RGBA. Encoding is
deterministic: the same `Image` always produces byte-identical output.

CLI wiring removes the input-only rejection for WEBP in `detect_output_format`,
`parse_batch_output_format`, and the `encode_image` dispatch arm. WEBP is now
listed as a supported format/prefix in `usage()` and `--help`.

### Round trip

```
imx PNG:in.png WEBP:/tmp/o.webp
imx identify /tmp/o.webp        # format=WEBP ... correct dims
imx WEBP:/tmp/o.webp PPM:/tmp/o.ppm
```

## batch-convert `--quality`

`batch-convert` accepts an optional `--quality <1..=100>` flag. It reuses the
JPEG `encode_with_quality` path (same as the single transcode) so JPEG batch
outputs honor the requested quality. The range is validated by the shared
`parse_quality`, and the flag is rejected for non-JPEG `--to` targets with the
same message as the single transcode (`--quality only applies to JPEG output`).

```
imx batch-convert --to JPEG --output-dir /tmp/out --quality 40 in.png
imx batch-convert --to JPEG --output-dir /tmp/out --quality 95 in.png
# the q40 output is smaller than the q95 output
```

## Out of scope: animated WebP output

Animated WebP output remains unsupported because the pinned encoder only writes
lossless still frames. GIF output is covered separately in
[`docs/gif-output.md`](gif-output.md) and animated GIF assembly in
[`docs/animation-output.md`](animation-output.md).
