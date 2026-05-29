# imx-cli

The `imx` command-line image converter: a fast, memory-safe,
differentially-verified image conversion tool for Rust, part of the
[`imx`](https://github.com/jskoiz/imx) toolkit.

Installing this crate provides the `imx` binary:

```sh
cargo install imx-cli
```

## Usage

```sh
imx input.png output.jpg
imx --quality 90 input.png output.jpg
imx resize 320x240 input.png output.png
imx crop 100x100+10+10 input.png output.png
imx rotate 90 input.png output.png
imx identify --json input.png
```

Supported formats: bmp, farbfeld, jpeg, qoi, pbm, pgm, png, ppm (read/write);
gif and webp (input only). Run `imx --help` for the full command list.

## Why trust it

- **Differentially verified** against the real ImageMagick binary as an oracle.
- **Deterministic.** The same input always produces byte-identical output.
- **Memory-safe and bounded.** Pure safe Rust with capped allocations.

## License

Distributed under the ImageMagick License. See [`LICENSE`](LICENSE).
