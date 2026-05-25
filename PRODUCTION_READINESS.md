# IMX v0.3.0 Release Readiness

Status: release-readiness report for the standalone `jskoiz/imx` product repo.

## Implemented Behavior

- Source of truth: `https://github.com/jskoiz/imx`.
- Product binary: `imx`.
- v0.3.0 completes the baseline Netpbm slice by adding PBM while preserving the
  existing FARBFELD, QOI, PGM, and PPM support.
- PBM support covers ASCII `P1` and binary `P4` decode, logical bilevel pixel
  buffers, deterministic binary `P4` encode, and `.pbm` CLI
  identify/transcode.
- The shared Netpbm codec crate remains `imx-codec-pnm`.
- RGB/RGBA to PBM uses Rec.709 luma and thresholding; PBM to color formats
  replicates black/white channels and adds opaque alpha where needed.

## Evidence Table

| Gate | Producer | Artifact Path | PBM Coverage | Result |
| --- | --- | --- | --- | --- |
| Release gates | `scripts/ci.sh` | terminal plus CI logs | cargo fmt, clippy, tests, fixture generation, fuzz smoke, cargo-fuzz, benchmark smoke, differential tests | local candidate passed 2026-05-25 UTC |
| ImageMagick differentials | `cargo test --test differential` | test output | PBM identify, P1/P4 decode, PBM transcodes, FARBFELD-to-PBM thresholding | 13 tests passed locally |
| Fuzz smoke | `scripts/run-fuzz.sh` | `target/fuzz-runs/20260524-183720/summary.json` | PNM target covers PBM/PGM/PPM identify and decode with PBM seeds | passed; PNM seed count 896 |
| Bench/RSS | `scripts/bench-release.sh` | `target/release-bench-20260524-183735/summary.json` | PBM decode/encode, PBM-to-FARBFELD, FARBFELD-to-PBM, oracle PBM cases | passed; standalone PBM RSS cases recorded |
| Install verify | `scripts/verify-install.sh` | `target/install-verify/install-summary.json` | PBM identify plus PBM/FARBFELD transcodes | passed from fresh checkout of committed `HEAD` |
| Package/SHA | `scripts/package-release.sh` and release workflow | `target/release-artifacts`, GitHub Release assets | Linux and macOS archives include PBM-capable `imx` | local macOS package passed; tag CI publishes Linux and macOS archives |
| No ImageMagick link | GitHub Actions `ldd`/`otool -L` checks | CI logs | release binaries checked for MagickCore/MagickWand/ImageMagick linkage | local `otool -L` clean; tag CI checks each release package |

## Local Verification

Commands run locally on 2026-05-24 HST / 2026-05-25 UTC:

```sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  IMX_REQUIRE_ORACLE=1 \
  IMX_FUZZ_MAX_TOTAL_TIME=1 \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/ci.sh
IMAGEMAGICK_MAGICK=/Users/jk/Desktop/imagemagick/utilities/magick \
  IMX_BENCH_ITERATIONS=2 \
  bash scripts/bench-release.sh
IMX_INSTALL_REPO_URL=/Users/jk/Desktop/imx \
  IMX_INSTALL_REVISION=HEAD \
  bash scripts/verify-install.sh
bash scripts/package-release.sh
otool -L target/release/imx
```

The required ImageMagick differential lane passed 13 tests covering FARBFELD,
QOI, PBM, PGM, and PPM, including P1/P4 decode, PBM transcodes,
FARBFELD-to-PBM threshold behavior, P2/P5 decode, 16-bit PGM, PGM transcodes,
FARBFELD-to-PGM luma behavior, and identify output.

Fuzz summary:

```text
target/fuzz-runs/20260524-183720
farbfeld_decode: passed, 17 seeds
qoi_decode: passed, 811 seeds
pnm_decode: passed, 896 seeds
```

Benchmark evidence:

```text
target/release-bench-20260524-183735
iterations=2
pbm_decode_mib_s=207.92
pbm_encode_mib_s=62.85
pbm_to_ff_mib_s=25.77
ff_to_pbm_mib_s=3901.92
max_rss_bytes=6242304
standalone-ff-to-pbm RSS=1818624 bytes
standalone-pbm-to-ff RSS=1769472 bytes
```

## Release Automation

- Release package script: `scripts/package-release.sh`.
- One-command installer: `scripts/install.sh`.
- CI workflow: `.github/workflows/rust-standalone-preview.yml`.
- Branch, pull-request, and tag CI build ImageMagick as an external oracle, run
  IMX release gates, generate structured benchmark evidence, package release
  artifacts, verify fresh-checkout installation, and upload evidence artifacts.
- Tag pushes matching `v*` run the preview gates, build native Linux/macOS
  release archives, generate aggregate checksums, and publish the matching
  GitHub Release.
- Release archives are written with deterministic tar/gzip metadata, sorted
  entries, fixed ownership, fixed mtimes, stable file modes, and aggregate
  `SHA256SUMS` entries.

## Confirmed Incompatibilities

- IMX is not a full ImageMagick CLI.
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- PBM source form is not preserved; output PBM is deterministic binary P4.
- PBM comments, whitespace, and padding-bit values are not preserved.
- PBM conversion from gray/color inputs is lossy thresholding.
- PPM is RGB8-only; high-depth PPM is out of scope.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are out of scope.
- P2 input is not source-preserved; output PGM is deterministic binary P5.
- PGM comments and whitespace are not preserved.
- FARBFELD to QOI/PPM is lossy for non-8-bit-representable 16-bit samples.
- Color to PGM/PBM is lossy and ignores alpha.
- No PAM, PFM, PNG, or BMP.
- Fuzzing is time-bounded in CI; longer scheduled fuzzing remains future work.

## Safety Wins

- PBM parsing is safe Rust and shares the bounded Netpbm header parser with
  PGM/PPM.
- PBM dimensions, row strides, and decoded buffers are checked before
  allocation.
- P4 bit unpacking is row-local and ignores padding bits without reading beyond
  the expected raster.
- Malformed PBM inputs fail without panics in unit tests, fuzz-smoke tests, and
  cargo-fuzz smoke runs.
- CLI writes remain atomic via temp file plus rename.

## Next Smallest Milestone

The smallest adoption-expanding next milestone is a package-distribution pass:
publish the v0.3.0 archives, verify `scripts/install.sh` against downloaded
release assets on Linux and macOS, then decide whether a Homebrew tap or an
additional `aarch64-unknown-linux-gnu` artifact has more adoption value. PAM,
PNG, and BMP remain too broad for the next milestone.
