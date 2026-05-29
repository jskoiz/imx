#!/usr/bin/env bash
#
# verify-publishable.sh — dry-run the whole publish graph.
#
# Thin wrapper around scripts/publish.sh with no arguments (dry-run mode). Safe
# to run anywhere, including CI: it never publishes anything. It confirms that
# imx-core dry-runs cleanly and reports the status of every other crate.
#
# Dependent crates will be SKIPPED on a first-ever publish because their
# workspace dependencies are not on crates.io yet; that is expected and not a
# failure. See scripts/publish.sh and docs/releasing.md for details.
#
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "$root/scripts/publish.sh"
