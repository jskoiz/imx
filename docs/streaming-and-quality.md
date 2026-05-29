# IMX Streaming and JPEG Quality

This slice adds two capabilities every real image CLI is expected to have:
stdin/stdout streaming via `-`, and explicit JPEG quality control via
`--quality`.

## Supported Surface

- `-` may be used as the input and/or output path for the existing commands:
  - `imx identify [FORMAT:]-`
  - `imx identify --json [FORMAT:]-`
  - `imx report --json [FORMAT:]-`
  - `imx resize <width>x<height> [FORMAT:]- [FORMAT:]-`
  - `imx resize-fit <width>x<height> [FORMAT:]- [FORMAT:]-`
  - `imx [--quality <1..=100>] [FORMAT:]- [FORMAT:]-` (the two-path transcode)
- `imx --quality <1..=100> [FORMAT:]<input> JPEG:<output>` controls the JPEG
  encoder quality on the single transcode.

`batch-convert` continues to reject `-`; streaming is single-input/single-output
only.

## Streaming Contract

When the input path is `-`, all bytes are read from stdin, bounded by the same
`MAX_INPUT_BYTES` limit used for file reads. When the output path is `-`, the
encoded image bytes are written to stdout and nothing else is printed to stdout,
so a stream stays byte-clean for piping.

Because `-` carries no file extension, the format MUST be supplied with a
`FORMAT:` prefix:

```sh
cat fixture.png | imx PNG:- PPM:- > out.ppm
imx identify out.ppm
# format=PPM width=2 height=1 channels=RGB depth=8
```

A `-` output without a resolvable format is rejected:

```sh
imx PNG:- - < fixture.png
# error: stdout output (-) requires a format prefix, e.g. PNG:-
```

For `-` input, the prefix is still validated against the sniffed magic bytes, so
a wrong prefix (e.g. `JPEG:-` fed PNG bytes) fails the same way a wrong prefix
fails on a file.

Output remains byte-for-byte deterministic: streaming uses the same encoders as
file output, so `PNG:- PPM:-` produces the identical bytes a `file.png file.ppm`
transcode would write.

## JPEG Quality Contract

`--quality <1..=100>` is parsed and validated at the CLI boundary. Out-of-range
or non-numeric values are rejected with `error: invalid --quality value: ...;
expected 1..=100` and exit code 1.

The flag applies only when the output format is JPEG. Supplying `--quality` with
any non-JPEG output is rejected to keep semantics tight:

```sh
imx --quality 50 fixture.png PNG:out.png
# error: --quality only applies to JPEG output, not PNG
```

The default JPEG quality is unchanged at 90. `imx_codec_jpeg::encode()` now
delegates to `encode_with_quality(image, 90)`, so default JPEG output is
byte-for-byte identical to prior releases. Lower quality yields smaller output
and higher quality yields larger output:

```sh
imx --quality 40 fixture.png JPEG:q40.jpg
imx --quality 95 fixture.png JPEG:q95.jpg
# q40.jpg is smaller than q95.jpg; both decode to the same dimensions
```

## Determinism and Safety

- stdin reads reuse the `MAX_INPUT_BYTES` cap with a bounded `take()` so an
  unbounded pipe cannot exhaust memory.
- `--quality` is range-checked at the boundary and re-checked inside
  `encode_with_quality` before any allocation.
- stdout output writes only encoded image bytes and flushes once.
- Same-path rejection skips `-` operands; stdin and stdout are distinct streams.

## Non-Goals

No new image formats, no streaming for `batch-convert`, no multi-image
concatenation on stdin, no quality controls for non-JPEG codecs, no progressive
or chroma-subsampling knobs, and no change to existing file-path behavior or the
default JPEG byte output.

## Evidence

- `imx_codec_jpeg` unit tests cover `encode_with_quality` range validation,
  default-quality byte equality with `encode()`, and quality-dependent output
  size.
- CLI tests cover a `PNG:- PPM:-` stdin-to-stdout round-trip, stdin `identify`
  text/JSON/report, streaming `resize`, rejection of prefix-less `-` output,
  `--quality` producing different JPEG sizes than default, and rejection of
  `--quality` for non-JPEG output and out-of-range values.
