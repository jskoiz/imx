# CI coverage for the newly merged surface

This note records how the verification harness was extended so the gates cover
the features merged into `main`: geometry operations (crop/rotate/flip/flop),
stdin/stdout streaming, JPEG `--quality`, and the WebP/GIF input codecs.

The goal is that the moat (differential testing, per-codec fuzzing, malformed
corpus, install/confidence corpus) guards the new surface, not just the
pre-existing formats and commands.

## Fixture generation

`crates/cli/src/bin/imx-generate-fixtures.rs` now emits deterministic WebP and
GIF inputs alongside the existing fixtures:

- `intake-webp-rgb-2x1.webp` — lossless WebP, RGB8.
- `intake-webp-rgba-2x1.webp` — lossless WebP, RGBA8.
- `intake-gif-rgba-2x1.gif` — single-frame GIF (decoded as RGBA8).

These feed every downstream corpus and fuzz-seed script, so all gates share the
same byte-exact, manifest-hashed fixtures. `image-webp` and `gif` were promoted
from CLI dev-dependencies to regular dependencies because the fixture generator
is a binary target and cannot use dev-dependencies; both crates were already in
the dependency graph via `imx-codec-webp` / `imx-codec-gif`, so no new crate is
pulled in.

## Fuzzing

- `fuzz/fuzz_targets/webp_decode.rs` and `gif_decode.rs` (and their entries in
  `fuzz/Cargo.toml`) already existed but were not enumerated by the scripts.
- `scripts/run-fuzz.sh` now loops over `webp_decode` and `gif_decode` in
  addition to the original six targets.
- `scripts/seed-fuzz-corpus.sh` now creates `webp_decode` / `gif_decode` corpus
  directories, copies the generated WebP/GIF fixtures, and writes structured
  malformed seeds (bad RIFF/WEBP magic, header-only, truncated GIF magic,
  no-image-block GIF, bad GIF magic).

The fuzz target sources compile and register under the nightly toolchain
(`RUSTFLAGS="--cfg fuzzing" cargo build --bin webp_decode --bin gif_decode`).
`cargo fuzz build` requires a sanitizer-capable nightly; where that is
available the new targets build the same way as the existing ones.

## Malformed / smoke tests (run by `cargo test`)

`tests/malformed/fuzz_smoke.rs` now exercises `imx_codec_webp::decode` and
`imx_codec_gif::decode`:

- In the random-bytes loop (lengths 0..2048), neither decoder may panic.
- A structured truncation loop encodes a real WebP and GIF fixture and decodes
  every truncation prefix without panicking.

## Curated corpus (`scripts/curated-corpus.sh` → `tests/curated_corpus.rs`)

The representative intake test gained WebP RGB8/RGBA8 and GIF RGBA8 cases that
assert the exact `identify` stable line and a successful decode. The script's
coverage manifest lists the WebP/GIF intake families.

## Daily-use / install confidence corpus (`scripts/daily-use-corpus.sh`)

This is the install/confidence gate that drives the real `imx` binary. It now:

- Runs `identify --json` and `report --json` for the generated WebP RGB/RGBA and
  GIF fixtures, asserting exact metadata.
- Transcodes WebP→PNG and GIF→PNG (decode-only inputs into a supported output)
  and verifies the output metadata.
- Adds a `run_geometry_case` helper and exercises `crop`, `rotate 90`, `flip`,
  and `flop` end to end, asserting the resulting PPM metadata via
  `report --json`.
- Reclassifies the `unsupported-prefix` diagnostic case to use `TGA:` because
  `GIF:`, `TIFF:`, and `WEBP:` are now recognized prefixes; without this the
  case would report `input.format_prefix_mismatch` instead of
  `input.unsupported_format_prefix`. The summary now reports `geometry_cases`.

## Differential tests (oracle-gated)

Geometry differential tests (`standalone_crop_matches_imagemagick_decoded_pixels`
and the rotate/flip/flop variants in `tests/differential/mod.rs`) already exist
and run as part of `cargo test --workspace`. They skip cleanly when the
ImageMagick oracle or standalone binary is absent (`require_or_skip` /
`assert_success_or_skip`), and `scripts/ci.sh` additionally runs the full
differential suite under `IMX_REQUIRE_ORACLE=1`. The no-oracle skip behavior is
preserved.

## What was not weakened

No existing gate was removed or loosened. The only behavioral edit to an
existing case was the `unsupported-prefix` reclassification above, which is a
correctness fix forced by GIF becoming a supported input prefix.
