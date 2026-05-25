# IMX v0.2.0 Release Readiness

Status: release-readiness report for the standalone `jskoiz/imx` product repo.

## Implemented Behavior

- Source of truth: `https://github.com/jskoiz/imx`.
- Product binary: `imx`.
- v0.2.0 adds PGM support as the first PNM expansion while preserving the
  existing FARBFELD, QOI, and PPM slice.
- PGM support covers ASCII `P2` and binary `P5` decode, GRAY8/GRAY16BE pixel
  buffers, deterministic binary `P5` encode, and `.pgm` CLI identify/transcode.
- The shared Netpbm codec crate is now `imx-codec-pnm`.
- RGB/RGBA to PGM uses Rec.709 luma and ignores alpha. PGM to color formats
  replicates gray channels and adds opaque alpha where needed.

## Release Automation

- Release package script: `scripts/package-release.sh`.
- CI workflow: `.github/workflows/rust-standalone-preview.yml`.
- Branch, pull-request, and tag CI build ImageMagick as an external oracle, run
  IMX release gates, generate structured benchmark evidence, package release
  artifacts, verify fresh-checkout installation, and upload evidence artifacts.
- Tag pushes matching `v*` run the preview gates and then publish packaged
  archives to the matching GitHub Release.
- Release archives are written with deterministic tar/gzip metadata, sorted
  entries, fixed ownership, fixed mtimes, stable file modes, and relative
  `SHA256SUMS` entries so repeated packaging of the same built payload is
  byte-for-byte comparable.

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
```

The required ImageMagick differential lane passed 10 tests covering FARBFELD,
QOI, PPM, and PGM, including P2/P5 decode, 16-bit PGM, PGM transcodes,
FARBFELD-to-PGM luma behavior, and identify output.

## Fuzz Evidence

Local fuzz run directory:

```text
target/fuzz-runs/20260524-153403
```

Summary:

```text
farbfeld_decode: passed, 17 seeds
qoi_decode: passed, 713 seeds
pnm_decode: passed, 570 seeds
```

The PNM fuzz target covers identify and decode entrypoints for both PPM and
PGM. Seed corpora include deterministic fixtures plus malformed/header-only P2,
P3, P5, and P6 cases.

## Benchmark And RSS Evidence

Local benchmark evidence directory:

```text
target/release-bench-20260524-153417
```

The benchmark script writes:

- `metadata.txt`
- `benchmark-run.json`
- `measurements.jsonl`
- `summary.json`
- `fixtures/manifest.txt`
- `fixtures/manifest.json`
- raw process time logs
- output hashes

Local library smoke metrics:

```text
iterations=2
farbfeld_decode_mib_s=22967.03
farbfeld_encode_mib_s=10796.55
qoi_decode_mib_s=821.11
qoi_encode_mib_s=895.52
ppm_decode_mib_s=599.01
ppm_encode_mib_s=2083.01
pgm_decode_mib_s=1471.50
pgm_encode_mib_s=2218.39
ff_to_qoi_mib_s=1606.26
qoi_to_ff_mib_s=430.53
ppm_to_ff_mib_s=353.51
ff_to_ppm_mib_s=4205.48
pgm_to_ff_mib_s=406.17
ff_to_pgm_mib_s=7244.39
max_rss_bytes=6799360
```

Representative local process RSS:

```text
standalone-ff-to-qoi: 1900544 bytes
standalone-ff-to-ppm: 1818624 bytes
standalone-ff-to-pgm: 1769472 bytes
standalone-qoi-to-ff: 1605632 bytes
standalone-ppm-to-ff: 1769472 bytes
standalone-pgm-to-ff: 1753088 bytes
oracle-ff-to-qoi: 8617984 bytes
oracle-ff-to-ppm: 8388608 bytes
oracle-ff-to-pgm: 8437760 bytes
oracle-qoi-to-ff: 8404992 bytes
oracle-ppm-to-ff: 8454144 bytes
oracle-pgm-to-ff: 8454144 bytes
```

These are release-smoke measurements, not statistically significant benchmark
claims.

## Install Verification

`scripts/verify-install.sh` clones a fresh checkout, runs:

```sh
cargo install --path crates/cli --bin imx --locked
```

Then it verifies:

- `imx --version`
- identify for generated FARBFELD/QOI/PPM/PGM fixtures
- FARBFELD to QOI transcode
- FARBFELD to PGM transcode
- PPM to FARBFELD transcode
- PGM to FARBFELD transcode

Local fresh-checkout verification passed at:

```text
target/install-verify/install-summary.json
```

## Release Artifact Contents

`scripts/package-release.sh` produces a target-specific tarball containing:

```text
imx
LICENSE
NOTICE
README.md
PRODUCTION_READINESS.md
RELEASE_NOTES.md
COMPATIBILITY.md
```

## Confirmed Incompatibilities

- IMX is not a full ImageMagick CLI.
- No same-format rewrites.
- No stdin/stdout streaming.
- No format prefixes such as `QOI:out.qoi`.
- No delegates, profiles, color management, resize/transform operations,
  MagickCore API, or MagickWand API.
- PPM is RGB8-only; high-depth PPM is out of scope.
- PGM supports `maxval <= 65535`; ImageMagick's nonstandard 32-bit PGM variants
  are out of scope.
- P2 input is not source-preserved; output PGM is deterministic binary P5.
- PGM comments and whitespace are not preserved.
- FARBFELD to QOI/PPM is lossy for non-8-bit-representable 16-bit samples.
- Color to PGM is lossy and ignores alpha.
- No PBM, PAM, PFM, PNG, or BMP.
- Fuzzing is time-bounded in CI; longer scheduled fuzzing remains future work.

## Safety Wins

- PGM parsing is safe Rust and shares the hardened Netpbm token/comment parser
  with PPM.
- PGM dimensions and decoded buffers are checked before allocation.
- Malformed PGM inputs fail without panics in unit tests, fuzz-smoke tests, and
  cargo-fuzz smoke runs.
- CLI writes remain atomic via temp file plus rename.

## Next Smallest Milestone

The smallest adoption-expanding next format is PBM `P1`/`P4`, because the PNM
crate boundary and gray/RGB buffer model are now in place. PAM should wait until
PBM and the compatibility ledger are stable. PNG and BMP remain too broad for
the next milestone.
