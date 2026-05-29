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

### Added

- **Formats (10).** BMP, FARBFELD, JPEG, PNG, PNM (PBM/PGM/PPM), QOI, TIFF, and
  WebP are supported for both decode and encode (TIFF in+out, WebP in+out);
  GIF is input-only (decode and identify, first frame only — animation/
  multi-frame is ignored). Deterministic same-format rewrites are supported for
  every output format except lossy JPEG re-encoding. WebP output is lossless.
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
- **Format prefixes.** Explicit `BMP:`, `FARBFELD:`, `JPEG:`, `QOI:`, `PBM:`,
  `PGM:`, `PPM:`, `PNG:`, `TIFF:`, `WEBP:` prefixes, plus input-only `GIF:`.
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
