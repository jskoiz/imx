# Homebrew Tap Formula

IMX releases generate a binary Homebrew formula from the release archive
checksums:

```sh
scripts/generate-homebrew-formula.sh v0.4.0 SHA256SUMS imx.rb
```

The generated formula is committed to the `jskoiz/homebrew-imx` tap, not
submitted to Homebrew/core. It installs the prebuilt `imx` binary for macOS
arm64, macOS x86_64, or Linux x86_64, then runs a formula test that identifies
PPM input and transcodes it to QOI.

Linux arm64 release archives are built by the IMX release workflow, but the tap
formula should not claim Linux arm64 until the formula generator and tap update
automation include the Linux arm64 URL and checksum.

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
