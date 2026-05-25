# Homebrew Tap Formula

IMX releases generate a binary Homebrew formula from the release archive
checksums:

```sh
scripts/generate-homebrew-formula.sh v0.5.0 SHA256SUMS imx.rb
```

The generated formula is committed to the `jskoiz/homebrew-imx` tap only, not
submitted to Homebrew/core. The generator emits platform blocks only for
archives present in that release's `SHA256SUMS`; missing macOS archives do not
block a Linux-only formula, and Linux arm64 is emitted only when the release
contains a checked `aarch64-unknown-linux-gnu` URL and `sha256`.

Tap updates are automation for the `jskoiz/homebrew-imx` tap only. They must not
trigger hosted macOS or iOS GitHub Actions; macOS tap proof remains local/manual
unless explicitly approved in the current turn.

Linux arm64 may be added to the tap only after `Formula/imx.rb` contains a
published Linux arm64 release archive `url` and matching `sha256` generated from
the release `SHA256SUMS`, and the Linux-only tap smoke fetches and verifies that
formula entry. Until then, Linux arm64 is a release-archive preview only, not a
tap-supported platform.

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
