#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="${IMX_VERSION:-v$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"imx-preview","version":"\([^"]*\)".*/\1/p')}"
if [[ "$version" != v* ]]; then
  version="v$version"
fi

evidence_root="${IMX_CONFORMANCE_EVIDENCE_DIR:-$root/target}"
release_dir="${IMX_RELEASE_DIR:-}"
out_dir="${IMX_CONFORMANCE_OUT:-$root/target/conformance}"
mkdir -p "$out_dir"

git_rev="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

latest_match() {
  local pattern="$1"
  local matches
  matches="$(find "$evidence_root" -path "$pattern" -type f 2>/dev/null || true)"
  if [[ -z "$matches" ]]; then
    return 0
  fi
  printf '%s\n' "$matches" | xargs ls -t 2>/dev/null | head -n 1
}

fuzz_summary="$(latest_match '*/fuzz-runs/*/summary.json')"
differential_summary="$(latest_match '*/differential-corpus-*/summary.json')"
bench_summary="$(latest_match '*/release-bench-*/summary.json')"
bench_thresholds="$(latest_match '*/release-bench-*/threshold-summary.json')"
bench_regression="$(latest_match '*/bench-regression-*/regression-report.json')"
install_summary="$(latest_match '*/install-verify/install-summary.json')"

archive_table="No release archive directory was supplied."
if [[ -n "$release_dir" && -f "$release_dir/SHA256SUMS" ]]; then
  archive_table="$(awk '{ print "- `" $2 "` sha256 `" $1 "`" }' "$release_dir/SHA256SUMS")"
fi

cat >"$out_dir/CONFORMANCE_REPORT.md" <<EOF
# IMX $version Conformance Report

Generated: $generated_at

Git revision: \`$git_rev\`

## Supported Surface

- Binary: \`imx\`
- Formats: FARBFELD, QOI, PBM, PGM, PPM
- Commands: \`imx --help\`, \`imx --version\`, \`imx identify <input>\`, and
  two-argument transcodes between different supported formats.
- Runtime dependency policy: no ImageMagick, MagickCore, MagickWand, delegates,
  modules, \`policy.xml\`, or ImageMagick build system linkage.

## Release Archives

$archive_table

## Evidence Inputs

| Gate | Evidence |
| --- | --- |
| Differential corpus | ${differential_summary:-missing} |
| Fuzz | ${fuzz_summary:-missing} |
| Benchmark/RSS | ${bench_summary:-missing} |
| Benchmark thresholds | ${bench_thresholds:-missing} |
| Benchmark regression | ${bench_regression:-missing} |
| Fresh source install | ${install_summary:-missing} |
| Release archive SHA/no-link/smoke | release archive smoke summaries from CI matrix |

## Compatibility Coverage

- Golden fixtures cover representative FARBFELD, QOI, PBM, PGM, and PPM bytes.
- Malformed-input tests cover invalid headers, truncation, oversized dimensions,
  unsupported max values, and failed CLI output behavior.
- ImageMagick differential tests cover decoded-pixel compatibility for
  FARBFELD/QOI/PBM/PGM/PPM identify and transcode paths.
- Cargo-fuzz targets exercise FARBFELD, QOI, and shared PNM identify/decode
  entrypoints with seeded corpora.
- Benchmarks record library throughput, process timing, process RSS, and output
  hashes for standalone and ImageMagick oracle cases.

## Confirmed Non-Goals

- No full ImageMagick CLI parser or \`magick\` alias.
- No same-format rewrites, stdin/stdout streaming, delegates, profiles, color
  management, transforms, MagickCore, or MagickWand.
- No PNG, JPEG, TIFF, PAM, PFM, BMP, or high-depth PPM support in this release.
EOF

cat >"$out_dir/conformance-summary.json" <<EOF
{
  "schema_version": 1,
  "version": "$version",
  "git_rev": "$git_rev",
  "generated_at": "$generated_at",
  "differential_summary": "${differential_summary:-}",
  "fuzz_summary": "${fuzz_summary:-}",
  "benchmark_summary": "${bench_summary:-}",
  "benchmark_thresholds": "${bench_thresholds:-}",
  "benchmark_regression": "${bench_regression:-}",
  "install_summary": "${install_summary:-}",
  "release_dir": "$release_dir",
  "report": "CONFORMANCE_REPORT.md"
}
EOF

echo "$out_dir"
