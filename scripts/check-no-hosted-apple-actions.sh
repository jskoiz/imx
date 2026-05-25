#!/usr/bin/env bash
set -euo pipefail

workflow_dir="${1:-.github/workflows}"

if [[ ! -d "$workflow_dir" ]]; then
  echo "workflow directory not found: $workflow_dir" >&2
  exit 1
fi

if rg -n -i 'runs-on:[[:space:]]*.*macos|runner:[[:space:]]*macos-|macos-[0-9]|macos-latest|xcodebuild|xcrun|simctl|iphonesimulator|platform[=:]iOS|destination=.*iOS' \
  "$workflow_dir" \
  -g '*.yml' \
  -g '*.yaml'
then
  echo "hosted Apple GitHub Actions runner reference found in $workflow_dir" >&2
  exit 1
fi

echo "No hosted Apple GitHub Actions runner references found in $workflow_dir."
