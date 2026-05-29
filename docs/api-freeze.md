# IMX API freeze gate

This gate exists to keep the pre-1.0 public surface deliberate. IMX publishes
`imx-core`, the codec crates, and `imx-cli` together with a single workspace
version, but only `imx-core` is treated as the long-term library contract for
1.0. Codec crates remain public so the CLI resolves cleanly and advanced users
can opt in, but their internals may still move within 1.x as documented in
[`v1.0-readiness.md`](v1.0-readiness.md).

## Stable core contract

`imx-core`'s stable 1.0 contract is:

- `Image`, `PixelFormat`, and `Format` for decoded raster data and identify
  metadata.
- Bounded geometry, resampling, color/tone, and pixel-format conversion methods
  on `Image`.
- `ResizeGeometry`, `ResizeFilter`, `Comparison`, and `compare_rgba8` for the
  CLI-backed operation surface.
- `ImageError::diagnostic_code()` and the documented diagnostic-code meanings.

`ImageError` is intentionally `#[non_exhaustive]`: callers may match known
variants, but must keep a wildcard arm so new precise error categories can be
added without a semver break. The diagnostic code strings are the stable machine
contract; new codes may be added, but existing codes must not change meaning
within 1.x.

## Release check

Before tagging a release candidate, capture and review the public surface:

```sh
cargo install cargo-public-api --locked
PATH="$(dirname "$(rustup which --toolchain nightly cargo)"):$PATH" \
  cargo public-api -p imx-core --simplified > target/public-api-imx-core.txt
```

Review the diff from the previous release's `target/public-api-imx-core.txt`.
Any new `pub` item needs a concrete reason, documentation, and either a stable
1.x commitment or an explicit note that it is outside the 1.0 stable contract.

The API freeze review is not a substitute for the normal gates:

```sh
cargo doc --workspace --no-deps
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
