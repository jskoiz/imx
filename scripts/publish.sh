#!/usr/bin/env bash
#
# publish.sh — publish the imx workspace to crates.io in dependency order.
#
# The crate graph publishes bottom-up: imx-core first, then every codec crate
# (each depends only on imx-core), then imx-cli last (depends on core + every
# codec). The workspace root package `imx-preview` is publish = false (it is the
# lib-only test/bench harness) and is intentionally NOT in the list below.
#
# Modes:
#   (no args)    DRY RUN. Runs `cargo publish --dry-run -p <crate>` for every
#                crate in order and prints a PASS/FAIL summary. This is the
#                default so the script is always safe to run.
#   --execute    REAL PUBLISH. Gated: refuses unless CARGO_REGISTRY_TOKEN is set,
#                prints the ordered plan, and requires --yes to proceed
#                non-interactively. Between crates it polls the index for the
#                just-published version before publishing dependents.
#   --yes        Confirm a real --execute run without an interactive prompt.
#   -h|--help    Show usage.
#
#   IMX_PUBLISH_ALLOW_DIRTY_DRY_RUN=1
#                Local PR-readiness only: pass --allow-dirty to dry-run publish
#                commands so uncommitted changes can be packaged before a
#                release commit exists. Real --execute publishes never use this.
#
# IMPORTANT about dry-run of dependents:
#   Until imx-core (and the codec crates) actually exist on crates.io, a
#   `cargo publish --dry-run` of any DEPENDENT crate fails while resolving the
#   registry index with an error like:
#       error: no matching package named `imx-core` found
#   This is EXPECTED for a first-ever publish: the dependency is not indexed yet.
#   The script detects this specific situation, annotates it as SKIPPED (index
#   not yet populated) rather than a hard failure, and still requires that
#   imx-core itself dry-runs cleanly end-to-end. See docs/releasing.md.
#
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Ordered, explicit publish plan. imx-preview (publish = false) is omitted.
CRATES=(
  imx-core
  imx-codec-bmp
  imx-codec-farbfeld
  imx-codec-gif
  imx-codec-jpeg
  imx-codec-png
  imx-codec-pnm
  imx-codec-qoi
  imx-codec-tiff
  imx-codec-webp
  imx-cli
)

# Crates with no intra-workspace dependencies. A dry-run of these must always
# succeed end-to-end even before anything is on crates.io.
ROOT_CRATES=(imx-core)

# How long to wait for a freshly published version to appear on the index
# before publishing the crates that depend on it. Dependents fail to publish
# if the dependency is not yet indexed, so we poll instead of guessing a sleep.
POLL_INTERVAL_SECS="${IMX_PUBLISH_POLL_INTERVAL:-10}"
POLL_MAX_ATTEMPTS="${IMX_PUBLISH_POLL_ATTEMPTS:-30}"

usage() {
  sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^#\{0,1\} \{0,1\}//'
}

is_root_crate() {
  local needle="$1" c
  for c in "${ROOT_CRATES[@]}"; do
    [[ "$c" == "$needle" ]] && return 0
  done
  return 1
}

# Read the shared workspace version from the root Cargo.toml: the first
# `version = "X.Y.Z"` line under the [workspace.package] table.
workspace_version() {
  awk '
    /^\[workspace\.package\]/ { in_wp = 1; next }
    /^\[/ { in_wp = 0 }
    in_wp && /^version *=/ {
      match($0, /"[^"]+"/)
      print substr($0, RSTART + 1, RLENGTH - 2)
      exit
    }
  ' Cargo.toml
}

# Poll crates.io's sparse index until <crate>@<version> is available.
wait_for_index() {
  local crate="$1" version="$2" attempt=1
  local url
  # crates.io sparse index path layout (see doc.rust-lang.org cargo registries).
  case ${#crate} in
    1) url="https://index.crates.io/1/${crate}" ;;
    2) url="https://index.crates.io/2/${crate}" ;;
    3) url="https://index.crates.io/3/${crate:0:1}/${crate}" ;;
    *) url="https://index.crates.io/${crate:0:2}/${crate:2:2}/${crate}" ;;
  esac
  echo "    waiting for ${crate}@${version} to be indexed..."
  while (( attempt <= POLL_MAX_ATTEMPTS )); do
    if curl -fsSL "$url" 2>/dev/null | grep -q "\"vers\":\"${version}\""; then
      echo "    ${crate}@${version} is indexed (attempt ${attempt})."
      return 0
    fi
    sleep "$POLL_INTERVAL_SECS"
    (( attempt++ ))
  done
  echo "    WARNING: ${crate}@${version} not visible on the index after" \
    "$(( POLL_MAX_ATTEMPTS * POLL_INTERVAL_SECS ))s; continuing anyway."
  return 1
}

# ---- argument parsing -------------------------------------------------------
EXECUTE=0
ASSUME_YES=0
for arg in "$@"; do
  case "$arg" in
    --execute) EXECUTE=1 ;;
    --yes|-y) ASSUME_YES=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

VERSION="$(workspace_version)"
echo "imx publish — workspace version ${VERSION:-<unknown>}"
echo "plan (dependency order; imx-preview is publish=false and skipped):"
for c in "${CRATES[@]}"; do echo "  - $c"; done
echo

# ---- dry-run mode (default) -------------------------------------------------
if (( EXECUTE == 0 )); then
  echo "MODE: dry-run (cargo publish --dry-run). No crate will be published."
  echo "Pass --execute --yes to perform a real publish."
  if [[ "${IMX_PUBLISH_ALLOW_DIRTY_DRY_RUN:-0}" == "1" ]]; then
    echo "IMX_PUBLISH_ALLOW_DIRTY_DRY_RUN=1: dry-run publish will include uncommitted changes."
  fi
  echo

  declare -a results=()
  had_fail=0
  had_skip=0
  root_crate_failed=0

  for crate in "${CRATES[@]}"; do
    echo ">>> dry-run: $crate"
    log="$(mktemp)"
    publish_args=(publish --dry-run -p "$crate")
    if [[ "${IMX_PUBLISH_ALLOW_DIRTY_DRY_RUN:-0}" == "1" ]]; then
      publish_args+=(--allow-dirty)
    fi
    if cargo "${publish_args[@]}" >"$log" 2>&1; then
      results+=("PASS    $crate")
      echo "    PASS"
    elif grep -qiE "no matching package named|failed to select a version for the requirement|not found in registry" "$log" && ! is_root_crate "$crate"; then
      # Dependency simply not on crates.io yet — expected for a first publish.
      # Only acceptable for dependent crates; a root crate has no workspace deps.
      results+=("SKIPPED $crate (deps not yet on crates.io — expected pre-release)")
      echo "    SKIPPED: upstream workspace crate not yet on crates.io (expected)."
      had_skip=1
    else
      results+=("FAIL    $crate")
      echo "    FAIL"
      sed 's/^/    | /' "$log"
      had_fail=1
      is_root_crate "$crate" && root_crate_failed=1
    fi
    rm -f "$log"
    echo
  done

  echo "================ dry-run summary ================"
  for r in "${results[@]}"; do echo "  $r"; done
  echo "================================================="
  if (( root_crate_failed == 1 )); then
    echo "RESULT: FAIL — a root crate (no workspace deps) did not dry-run cleanly."
    exit 1
  fi
  if (( had_fail == 1 )); then
    echo "RESULT: FAIL — one or more crates failed to dry-run for a real reason."
    exit 1
  fi
  if (( had_skip == 1 )); then
    echo "RESULT: PASS (with SKIPPED dependents) — imx-core dry-ran cleanly;"
    echo "        dependent crates are SKIPPED only because upstream crates are"
    echo "        not on crates.io yet. This is expected before the first real"
    echo "        publish. See docs/releasing.md."
  else
    echo "RESULT: PASS — every crate dry-ran cleanly."
  fi
  exit 0
fi

# ---- execute mode (gated real publish) --------------------------------------
echo "MODE: EXECUTE — this performs REAL, IRREVERSIBLE cargo publish calls."

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "ERROR: --execute requires CARGO_REGISTRY_TOKEN to be set (or run" >&2
  echo "       'cargo login' so cargo has a stored token, then export it)." >&2
  exit 1
fi

if (( ASSUME_YES == 0 )); then
  echo "ERROR: --execute requires explicit --yes to proceed non-interactively." >&2
  echo "       Re-run: scripts/publish.sh --execute --yes" >&2
  exit 1
fi

if [[ -z "$VERSION" ]]; then
  echo "ERROR: could not determine the workspace version from Cargo.toml." >&2
  exit 1
fi

echo "Publishing workspace version ${VERSION} to crates.io in the order above."
echo

declare -a results=()
last_crate="${CRATES[$(( ${#CRATES[@]} - 1 ))]}"
for crate in "${CRATES[@]}"; do
  echo ">>> publish: $crate@${VERSION}"
  if cargo publish -p "$crate"; then
    results+=("PUBLISHED $crate")
    echo "    PUBLISHED"
    # Wait for the index before publishing crates that depend on this one.
    if [[ "$crate" != "$last_crate" ]]; then
      wait_for_index "$crate" "$VERSION" || true
    fi
  else
    results+=("FAIL      $crate")
    echo
    echo "================ publish summary ================"
    for r in "${results[@]}"; do echo "  $r"; done
    echo "================================================="
    echo "RESULT: FAIL — publishing stopped at $crate. Already-published crates" >&2
    echo "        cannot be unpublished; fix the issue and re-run for the rest." >&2
    exit 1
  fi
  echo
done

echo "================ publish summary ================"
for r in "${results[@]}"; do echo "  $r"; done
echo "================================================="
echo "RESULT: PASS — all crates published at version ${VERSION}."
echo "Verify with: cargo install imx-cli"
