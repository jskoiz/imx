#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

msrv="${IMX_MSRV:-1.85.0}"
toolchain="${IMX_MSRV_TOOLCHAIN:-$msrv}"

if ! rustup toolchain list | grep -q "^$toolchain"; then
  echo "error: Rust MSRV toolchain $toolchain is not installed; run: rustup toolchain install $toolchain --profile minimal" >&2
  exit 2
fi

rustup run "$toolchain" cargo check --workspace --all-targets
