# Release checklist

Terse pre-release checklist for cutting an `imx` workspace release. Full
explanation: [docs/releasing.md](docs/releasing.md). For the 1.0 decision, see
[docs/v1.0-readiness.md](docs/v1.0-readiness.md).

Work top to bottom; do not skip the dry run or the tag-version match.

## 1. Green CI

- [ ] `bash scripts/ci.sh` passes locally.
- [ ] Differential corpus passes **against the ImageMagick oracle**:
      `IMAGEMAGICK_MAGICK=/path/to/magick IMX_REQUIRE_ORACLE=1 bash scripts/ci.sh`.
- [ ] Latest CI on the release commit is green (preview gates: differential,
      fuzz smoke, benchmark regression, install verify).

## 2. Version bump (single shared version)

- [ ] Bump `version` under `[workspace.package]` in the root `Cargo.toml`.
- [ ] Bump the matching `version` in `crates/core/Cargo.toml` `[package]`.
- [ ] Bump the pinned `version` of every `imx-*` entry under
      `[workspace.dependencies]` in the root `Cargo.toml` so path+version deps
      stay in lockstep.
- [ ] `cargo update -w` (or build) so `Cargo.lock` reflects the new version.

## 3. Changelog

- [ ] Move `## [Unreleased]` in `CHANGELOG.md` to a dated, versioned heading
      (`## [X.Y.Z] - YYYY-MM-DD`) and open a fresh empty `## [Unreleased]`.

## 4. Clean dry-run publish

- [ ] Working tree clean at the release commit.
- [ ] `bash scripts/publish.sh` dry-runs cleanly (PASS, or PASS-with-SKIPPED on
      a first-ever publish where dependents are not yet indexed).

## 5. Real publish (irreversible)

- [ ] `CARGO_REGISTRY_TOKEN=<token> bash scripts/publish.sh --execute --yes`.
- [ ] Publish order completes: `imx-core` → 9 codec crates → `imx-cli`.
      (`imx-preview` is `publish = false` and never published.)

## 6. Tag → release CI

- [ ] `git tag vX.Y.Z` (must equal `workspace.package.version`).
- [ ] `git push origin vX.Y.Z`.
- [ ] Release CI (`rust-standalone-preview.yml`) is green: Linux x86_64 +
      arm64 archives packaged, GitHub release published, Homebrew tap formula
      generated from `SHA256SUMS`.

## 7. Verify install

- [ ] `cargo install imx-cli` from a clean environment.
- [ ] `imx --version` reports `imx X.Y.Z`.
- [ ] `imx self-test` prints `self-test: passed`.
- [ ] (Optional) `brew tap jskoiz/imx && brew install imx && imx self-test`.
