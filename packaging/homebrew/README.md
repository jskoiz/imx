# Homebrew Formula Draft

IMX releases generate a binary Homebrew formula from the release archive
checksums:

```sh
scripts/generate-homebrew-formula.sh v0.4.0 SHA256SUMS imx.rb
```

The generated formula is a draft tap artifact, not a Homebrew/core submission.
It installs the prebuilt `imx` binary for macOS arm64, macOS x86_64, or Linux
x86_64, then runs a formula test that identifies PPM input and transcodes it to
QOI. The formula must be regenerated for every release from that release's
aggregate `SHA256SUMS`.
