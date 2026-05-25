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

`brew test` verifies installation only. Compatibility remains covered by the
CI differential corpus, fuzz, benchmark, and conformance gates.

The formula must be regenerated for every release from that release's aggregate
`SHA256SUMS`.
