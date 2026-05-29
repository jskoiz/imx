# Releasing the imx workspace

This is the runbook for publishing the `imx` workspace to [crates.io]. The
workspace is published as a set of crates that share one version; the
`scripts/publish.sh` helper drives the whole process in the correct order.

## What gets published

The workspace publishes **bottom-up in dependency order**:

1. `imx-core` — the core image model and pixel-format conversions. It has no
   intra-workspace dependencies, so it goes first.
2. The codec crates, each of which depends only on `imx-core`:
   `imx-codec-bmp`, `imx-codec-farbfeld`, `imx-codec-gif`, `imx-codec-jpeg`,
   `imx-codec-png`, `imx-codec-pnm`, `imx-codec-qoi`, `imx-codec-tiff`,
   `imx-codec-webp`.
3. `imx-cli` last — it depends on `imx-core` and every codec crate. Its package
   name is `imx-cli`; the binary it installs is named `imx`.

**`imx-preview` is NOT published.** That is the workspace root package — a
lib-only test/bench harness — and it is marked `publish = false` in the root
`Cargo.toml`. `scripts/publish.sh` deliberately omits it from the publish list.

### Why dependency order matters

crates.io only lets you publish a crate once all of its dependencies already
exist on the registry at the required versions. If you publish `imx-cli` before
`imx-core` is indexed, the publish fails to resolve `imx-core`. The script
publishes in the order above and, in `--execute` mode, **polls the crates.io
index** for each just-published version before publishing the crates that
depend on it. Without this wait, a dependent publish can race the index and
fail with `no matching package named imx-core found`. The poll interval and
attempt count are configurable via `IMX_PUBLISH_POLL_INTERVAL` (default 10s) and
`IMX_PUBLISH_POLL_ATTEMPTS` (default 30).

## Prerequisites

- A crates.io account with publish (owner/maintainer) rights on every `imx-*`
  crate.
- An API token. Either run `cargo login` once (cargo stores the token), or set
  `CARGO_REGISTRY_TOKEN` in the environment. `scripts/publish.sh --execute`
  **refuses to run** unless `CARGO_REGISTRY_TOKEN` is set.
- A clean working tree at the commit you intend to release, with the workspace
  version already bumped (see [Version bumping](#version-bumping)).
- `cargo`, `curl`, and a POSIX `awk` on PATH.

## Dry run (default, always safe)

Run with no arguments to dry-run every crate in order:

```sh
scripts/publish.sh
# or the thin CI-friendly wrapper:
scripts/verify-publishable.sh
```

This invokes `cargo publish --dry-run -p <crate>` for each crate and prints a
PASS / SKIPPED / FAIL summary. It never publishes anything.

### Expected: dependents are SKIPPED before the first publish

On a first-ever publish (nothing is on crates.io yet), the dry-run of any
**dependent** crate fails while resolving the registry index with:

```
error: no matching package named `imx-core` found
```

This is expected — the dependency simply is not indexed yet. The script detects
this specific case and reports it as **SKIPPED (deps not yet on crates.io —
expected pre-release)** rather than a hard failure. It still requires that
`imx-core` (which has no workspace dependencies) dry-runs **cleanly end to end**;
if `imx-core` itself fails, the script exits non-zero.

After `imx-core` and the codecs are actually published, re-running the dry-run
resolves further down the graph as more crates become available.

## Executing a real publish

> A real `cargo publish` is **irreversible** — a published version cannot be
> unpublished (only yanked). Do a dry run first.

```sh
export CARGO_REGISTRY_TOKEN=<your-token>   # required
scripts/publish.sh --execute --yes
```

`--execute` is gated:

- It refuses to run unless `CARGO_REGISTRY_TOKEN` is set.
- It prints the ordered plan and the workspace version.
- It requires an explicit `--yes` to proceed non-interactively (so a stray
  `--execute` cannot publish by accident).

Between crates it polls the crates.io index for the just-published version
before moving on to dependents. If any crate fails to publish, the script stops
immediately and prints which crates were already published (those cannot be
undone — fix the issue and re-run; the script will re-attempt from the top, and
already-published versions will fail fast, so prefer publishing only the
remaining crates manually if a partial publish occurred).

## Version bumping

All crates share a single version through `workspace.package.version` in the
root `Cargo.toml`. To cut a new release:

1. Bump `version` under `[workspace.package]` in the root `Cargo.toml`.
2. Bump the matching `version` in `[package]` of `crates/core/Cargo.toml` and
   the pinned `version` of every `imx-*` entry under
   `[workspace.dependencies]` in the root `Cargo.toml` so path+version deps stay
   in lockstep. (`imx-core` and the workspace-dependency pins are spelled out
   explicitly, so they must be edited together.)
3. Move the `## [Unreleased]` section in `CHANGELOG.md` to a dated, versioned
   heading and start a fresh `## [Unreleased]`.
4. Run the dry run (`scripts/publish.sh`) and the full test suite
   (`scripts/ci.sh`) before tagging.

## Post-publish verification

Once `imx-cli` is published, verify a clean install from crates.io:

```sh
cargo install imx-cli
imx self-test
```

`imx self-test` runs an offline confidence check across
identify/transcode/resize/resize-fit/batch-convert and should print
`self-test: passed`. You can also confirm individual crates resolve with
`cargo add imx-core@<version>` in a scratch project.

[crates.io]: https://crates.io/
