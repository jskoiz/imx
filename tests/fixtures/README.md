# Standalone Fixtures

Small golden fixtures are stored as whitespace-separated hex so they remain
reviewable in text diffs.

- `farbfeld-1x1-red-half-alpha.hex`: 1x1 RGBA16BE red pixel with alpha `0x8000`.
- `qoi-1x1-red-half-alpha.hex`: 1x1 RGBA8 red pixel with alpha `0x80`.
- `ppm-1x1-red.hex`: 1x1 binary `P6` RGB8 red pixel. ASCII `P3`
  coverage is inline in the golden and differential tests because it is already
  text-readable.
