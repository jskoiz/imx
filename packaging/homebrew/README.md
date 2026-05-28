# Homebrew Tap Formula

IMX releases generate a binary Homebrew formula from the release archive
checksums:

```sh
scripts/generate-homebrew-formula.sh v0.19.0 SHA256SUMS imx.rb
```

The generated formula is committed to the `jskoiz/homebrew-imx` tap only, not
submitted to Homebrew/core. The generator emits platform blocks only for
archives present in that release's `SHA256SUMS`; missing macOS archives do not
block a Linux-only formula, and Linux arm64 is emitted only when the release
contains a checked `aarch64-unknown-linux-gnu` URL and `sha256`.

Tap updates are automation for the `jskoiz/homebrew-imx` tap only. They must not
trigger hosted macOS or iOS GitHub Actions; macOS tap proof remains local/manual
unless explicitly approved in the current turn.

For v0.19.0, the generated tap formula includes Linux archive blocks present in
the published `SHA256SUMS`, and Linux-only tap smoke verifies the selected
archive plus the current FARBFELD/QOI/PBM/PGM/PNG/PPM/JPEG command slice,
including JPEG EXIF Orientation, progressive JPEG, v0.12 intake smoke, and the
v0.13 resize, v0.14 resize-fit, v0.15 batch-convert, v0.16 BMP, v0.17
self-test, v0.18 JSON identify/report smoke, and v0.19 daily-use diagnostic
smoke, before support is claimed.
For v0.19.0 and newer, the generator also emits direct JSON diagnostic smoke
for unsupported prefixes and prefix mismatches so tap tests cover the daily-use
diagnostic contract without requiring source-tree fixtures.

`brew test` verifies installation only. Compatibility remains covered by the
IMX differential corpus, fuzz, benchmark, and conformance gates; macOS tap
support requires recorded local macOS or explicitly approved manual smoke
evidence.

Before pushing tap or workflow changes, verify that hosted workflows do not
reference macOS or iOS runners:

```sh
bash scripts/check-no-hosted-apple-actions.sh
```

The formula must be regenerated for every release from that release's aggregate
`SHA256SUMS`.
