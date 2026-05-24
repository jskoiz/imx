# IMX Developer Preview

IMX is a standalone Rust image-tool prototype that is being built one
ImageMagick-compatible slice at a time. The current MVP slice supports
FARBFELD, QOI, and PPM identify/transcode workflows through the `imx` binary.

This is not an ImageMagick fork and does not link to MagickCore, MagickWand, or
ImageMagick modules. ImageMagick is used only as an external oracle in
compatibility tests and benchmarks.

## Build

```sh
cargo build --workspace
```

The preview binary is:

```text
target/debug/imx
```

The public product name for this milestone is **IMX Developer Preview**. The
package name is `imx-preview`; the shipped binary is `imx` to avoid colliding
with a full ImageMagick install.

## Supported Commands

```sh
target/debug/imx identify input.ff
target/debug/imx identify input.qoi
target/debug/imx identify input.ppm
target/debug/imx input.ff output.qoi
target/debug/imx input.ff output.ppm
target/debug/imx input.qoi output.ff
target/debug/imx input.qoi output.ppm
target/debug/imx input.ppm output.ff
target/debug/imx input.ppm output.qoi
```

Success prints one stable `identify` line or writes the transcode output. Data
and IO failures exit `1`; unsupported command shapes exit `2`.

## Release Gates

```sh
./scripts/ci.sh
```

To require ImageMagick oracle differentials:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 ./scripts/ci.sh
```

To generate release benchmark evidence:

```sh
IMAGEMAGICK_MAGICK=/path/to/magick ./scripts/bench-release.sh
```

To package a downloadable archive:

```sh
./scripts/package-release.sh
```

## Fixture Generation

```sh
cargo run -p imx-cli --bin imx-generate-fixtures -- target/generated-fixtures
```

The generator writes deterministic FARBFELD, QOI, PPM, and raw RGBA fixtures
plus a manifest with byte counts and deterministic FNV-1a hashes.

## Compatibility

See [COMPATIBILITY.md](COMPATIBILITY.md) for the exact behavior contract,
accepted compatibility oddities, known lossy conversions, and unsupported
surface.
