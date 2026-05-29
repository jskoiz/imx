# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The workspace ships as a set of crates that share a single version
(`workspace.package.version` in the root `Cargo.toml`): `imx-core`, the codec
crates `imx-codec-{bmp,farbfeld,gif,jpeg,png,pnm,qoi,tiff,webp}`, and the
`imx-cli` binary crate (binary name `imx`). The lib-only test/bench harness
package `imx-preview` is `publish = false` and is not released.

## [Unreleased]

This batch focuses on resize quality, animated output, a color/tone pipeline,
ICC handling, and release-engineering polish ahead of a 1.0 tag.

### Added

- **Real resampling filters.** `resize`/`resize-fit` gain a `--filter` flag
  selecting the interpolation kernel (nearest, triangle/bilinear, catmull-rom,
  lanczos3), with **Lanczos3 as the default** so downscales are no longer
  nearest-neighbor aliased. Output stays deterministic and byte-identical for a
  given filter.
- **Animated GIF output (`imx assemble`).** Compose an ordered set of input
  frames into a single animated GIF with per-frame delays and a deterministic
  global/local palette, complementing the existing first-frame/composited GIF
  decode.
- **Color and tone pipeline operations.** `imx pipeline` gains color/tone ops
  (e.g. grayscale, brightness/contrast, gamma, channel swaps) alongside the
  existing geometry ops, chained left-to-right in a single deterministic
  decode/encode pass.
- **ICC profile passthrough and `--strip`.** Embedded ICC profiles are carried
  through transcodes where the output format supports them; `--strip` drops ICC
  and other ancillary metadata for a minimal, reproducible output.
- **docs.rs build metadata.** Each published crate declares
  `[package.metadata.docs.rs]` so docs.rs builds the intended feature set and
  the docs.rs badge resolves for every `imx-*` crate.
- **`tiff_decode` fuzz target.** A coverage-guided fuzz target for the TIFF
  decode/identify entrypoint joins the existing BMP, FARBFELD, GIF, JPEG, PNG,
  PNM, QOI, and WebP fuzz targets, with seeded corpora.

### Already present in this batch

- **Animated frame selection.** `--frame <N>` (0-based) selects which frame to
  decode for `identify`, `report --json`, and single-input transcode; animated
  GIF/WebP frames are composited (GIF disposal Keep/Background/Previous honored)
  so frame N is the displayed canvas. Non-animated inputs accept only
  `--frame 0`.
- **TIFF decode + encode.** First-IFD 8/16-bit grayscale, 8/16-bit RGB, and
  8-bit RGBA in; deterministic little-endian uncompressed baseline out.
- **WebP output.** Lossless WebP encode joins the existing WebP decode.
- **`imx pipeline`.** Chains ordered `--op` values in one decode/encode pass
  (`resize`, `resize-fit`, `crop`, `rotate`, `flip`, `flop`); output is
  byte-deterministic and equivalent to running the same ops as sequential
  subcommands.
- **Auto-orientation.** EXIF/TIFF Orientation (1-8) is auto-applied on decode
  for JPEG and TIFF; `--no-auto-orient` keeps the raw stored pixels.
- **`report --json` schema 2.** Adds a `frames` count and bumps
  `schema_version` to 2.

### Added (stable surface)

- **Formats (11).** BMP, FARBFELD, JPEG, PNG, PNM (PBM/PGM/PPM), QOI, TIFF, and
  WebP are supported for both decode and encode (TIFF in+out, WebP in+out, WebP
  output lossless). GIF supports decode plus single-still-frame output with a
  deterministic palette of at most 256 colors; animated GIF/WebP frames are
  composited on decode and selectable via `--frame`. Deterministic same-format
  rewrites are supported for every output format except lossy JPEG re-encoding.
- **Transcoding.** `imx [--quality <1..=100>] <input> <output>` converts between
  any supported input and output format, including GIF decode into any
  supported output format.
- **Geometry operations.** `crop <WxH+X+Y>` (bounds-checked), `rotate <90|180|270>`
  (clockwise), `flip` (vertical), and `flop` (horizontal) — all format-preserving.
- **Resize.** `resize` supports exact dimensions (`<width>x<height>`),
  single-axis aspect-preserving (`<width>x` or `x<height>`), and uniform percent
  (`<percent>%`) using nearest-neighbor sampling; `resize-fit <width>x<height>`
  does aspect-preserving fit within a bounding box.
- **Batch conversion.** `batch-convert --to <FORMAT> --output-dir <dir>` over
  shell-expanded input paths, with optional `--resize`/`--resize-fit` and
  `--quality` (JPEG output only); refuses to overwrite or rename on collision.
- **`imx compare`.** Decodes two inputs and diffs them deterministically.
  Reports differing-pixel count, peak per-channel difference (AE), and mean
  absolute error (MAE); `--metric <ae|mae|psnr>` prints a single value (PSNR is
  `inf` for identical inputs). Identical inputs exit 0, differences exit 1,
  usage errors exit 2.
- **Identify / report JSON.** `identify` and `identify --json` print
  format/width/height/channels/depth metadata; `report --json` reports
  supported/unsupported status with stable `diagnostic_code` values and a
  deterministic `schema_version`.
- **Streaming I/O.** Read from stdin and/or write to stdout via `-` with a
  `FORMAT:` prefix (e.g. `PNG:-`); only image bytes are written to stdout.
- **Format prefixes.** Explicit `BMP:`, `FARBFELD:`, `GIF:`, `JPEG:`, `QOI:`,
  `PBM:`, `PGM:`, `PPM:`, `PNG:`, `TIFF:`, `WEBP:` prefixes (`JPG:` intentionally
  excluded).
- **JPEG quality control.** `--quality <1..=100>` on single transcode and
  `batch-convert` when the output format is JPEG (default 90); rejected for
  non-JPEG output.
- **Shell completions and man page.** `imx completions <bash|zsh|fish>` prints a
  completion script to stdout; a roff man page is bundled at `man/imx.1`.
- **Self-test.** `imx self-test` runs an offline install-confidence check across
  identify/transcode/resize/resize-fit/batch-convert for supported formats.
- **Differential-verification harness.** The workspace is differentially
  verified against ImageMagick (see `tests/differential/` and the corpus
  scripts under `scripts/`).
- **Release engineering.** `scripts/publish.sh` (dry-run by default, gated
  `--execute`), `scripts/verify-publishable.sh`, and the release runbook in
  `docs/releasing.md`.

### Notes

- The crates publish bottom-up: `imx-core` first, then the codec crates, then
  `imx-cli` last. The `imx-preview` workspace root package is not published.
