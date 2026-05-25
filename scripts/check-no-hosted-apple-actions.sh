#!/usr/bin/env bash
set -euo pipefail

workflow_dir="${1:-.github/workflows}"

if [[ ! -d "$workflow_dir" ]]; then
  echo "workflow directory not found: $workflow_dir" >&2
  exit 1
fi

pattern='runs-on:[[:space:]]*.*macos|runner:[[:space:]]*macos-|macos-[0-9]|macos-latest|xcodebuild|xcrun|simctl|iphonesimulator|platform[=:]iOS|destination=.*iOS'

if command -v rg >/dev/null 2>&1; then
  matches="$(rg -n -i "$pattern" "$workflow_dir" -g '*.yml' -g '*.yaml' || true)"
else
  matches="$(grep -RInE --include='*.yml' --include='*.yaml' "$pattern" "$workflow_dir" || true)"
fi

if [[ -n "$matches" ]]; then
  printf '%s\n' "$matches"
  echo "hosted Apple GitHub Actions runner reference found in $workflow_dir" >&2
  exit 1
fi

echo "No hosted Apple GitHub Actions runner references found in $workflow_dir."
