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
curated_summary="$(latest_match '*/curated-corpus/summary.json')"
bench_summary="$(latest_match '*/release-bench-*/summary.json')"
bench_thresholds="$(latest_match '*/release-bench-*/threshold-summary.json')"
bench_regression="$(latest_match '*/bench-regression-*/regression-report.json')"
install_summary="$(latest_match '*/install-verify/install-summary.json')"
glibc_symbol_files="$(find "$evidence_root" -path '*/glibc-symbols*.txt' -type f 2>/dev/null | sort || true)"

if [[ "${IMX_CONFORMANCE_REQUIRE_EVIDENCE:-0}" == 1 ]]; then
  missing_required=()
  [[ -n "$differential_summary" ]] || missing_required+=("differential corpus")
  [[ -n "$curated_summary" ]] || missing_required+=("curated corpus")
  [[ -n "$fuzz_summary" ]] || missing_required+=("fuzz smoke")
  [[ -n "$bench_summary" ]] || missing_required+=("benchmark summary")
  [[ -n "$bench_thresholds" ]] || missing_required+=("benchmark thresholds")
  [[ -n "$bench_regression" ]] || missing_required+=("benchmark regression")
  [[ -n "$install_summary" ]] || missing_required+=("fresh install")
  [[ -n "$glibc_symbol_files" ]] || missing_required+=("GLIBC symbol baseline")
  if [[ -z "$release_dir" || ! -f "$release_dir/SHA256SUMS" ]]; then
    missing_required+=("release archive SHA256SUMS")
  fi
  if ((${#missing_required[@]})); then
    printf 'error: missing required conformance evidence:\n' >&2
    printf '  - %s\n' "${missing_required[@]}" >&2
    exit 1
  fi
fi

glibc_symbols_report="No GLIBC symbol baseline evidence was found."
glibc_symbols_json="[]"
if [[ -n "$glibc_symbol_files" ]]; then
  glibc_symbols_report="$(
    GLIBC_SYMBOL_FILES="$glibc_symbol_files" python3 <<'PY'
import os
from pathlib import Path

for raw_path in os.environ["GLIBC_SYMBOL_FILES"].splitlines():
    path = Path(raw_path)
    print(f"- `{path}`")
    for line in path.read_text().splitlines():
        print(f"  - {line}")
PY
  )"
  glibc_symbols_json="$(
    GLIBC_SYMBOL_FILES="$glibc_symbol_files" python3 <<'PY'
import json
import os
import re
from pathlib import Path

records = []
for raw_path in os.environ["GLIBC_SYMBOL_FILES"].splitlines():
    path = Path(raw_path)
    text = path.read_text()
    max_versions = re.findall(r"max GLIBC_([0-9]+(?:\.[0-9]+)+); allowed GLIBC_([0-9]+(?:\.[0-9]+)+)", text)
    records.append({
        "path": str(path),
        "checks": [
            {"max": f"GLIBC_{observed}", "allowed": f"GLIBC_{allowed}"}
            for observed, allowed in max_versions
        ],
        "status": "passed" if "GLIBC symbol baseline passed" in text else "unknown",
    })
print(json.dumps(records, indent=2))
PY
  )"
fi

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
- Formats: BMP, FARBFELD, JPEG, QOI, PBM, PGM, PNG, PPM
- Commands: \`imx --help\`, \`imx --version\`, \`imx identify [FORMAT:]<input>\`,
  \`imx resize <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\`,
  \`imx resize-fit <width>x<height> [FORMAT:]<input> [FORMAT:]<output>\`, and
  \`imx batch-convert --to <FORMAT> --output-dir <dir>
  [--resize <width>x<height>|--resize-fit <width>x<height>] [FORMAT:]<input>...\`,
  \`imx self-test\`, and
  two-argument transcodes between supported formats, including exact
  \`BMP:\`, \`FARBFELD:\`, \`JPEG:\`, \`QOI:\`, \`PBM:\`, \`PGM:\`, \`PNG:\`, and \`PPM:\`
  operand prefixes and deterministic same-format rewrites when input and output
  paths differ. JPEG rewrites are deterministic lossy decode/re-encode
  operations. Progressive 8-bit grayscale/RGB JPEG input is supported for
  identify/decode/transcode; output remains deterministic baseline quality-90
  JPEG. v0.16.0 added uncompressed Windows BMP support for 24-bit BGR/RGB and
  32-bit BGRA/RGBA rasters across identify, transcode, resize, resize-fit,
  same-format rewrite, and batch-convert. This version adds an installed-binary
  offline self-test that creates temporary fixtures and exercises
  identify/transcode/resize/resize-fit/batch-convert across the supported
  formats without ImageMagick or network access. It does not add indexed, RLE,
  bitfields, OS/2, color-table, or high-depth BMP. $version also carries
  forward bounded nearest-neighbor resize to exact dimensions for
  the same supported formats, plus aspect-preserving nearest-neighbor resize-fit
  to fit within a requested box, and safe batch conversion with deterministic
  output names, existing-directory output, collision preflight, no overwrite,
  no recursive directory walking, no glob parsing, and no partial commit after
  transform or encode failure. The v0.12 intake reliability claim remains
  limited to generated and in-test corpus cases covering comments, high maxval
  Netpbm input, grayscale-alpha/16-bit PNG input, progressive JPEG input, QOI
  RGB linear input, malformed diagnostics, and resource-boundary rejection.
- Runtime dependency policy: no ImageMagick, MagickCore, MagickWand, delegates,
  modules, \`policy.xml\`, or ImageMagick build system linkage.
- Linux release archive policy: published glibc archives must not reference a
  \`GLIBC_*\` symbol version newer than \`GLIBC_2.34\`.

## Release Archives

$archive_table

## Linux GLIBC Baseline

$glibc_symbols_report

## Evidence Inputs

| Gate | Evidence |
| --- | --- |
| Differential corpus | ${differential_summary:-missing} |
| Curated intake corpus | ${curated_summary:-missing} |
| Fuzz | ${fuzz_summary:-missing} |
| Benchmark/RSS | ${bench_summary:-missing} |
| Benchmark thresholds | ${bench_thresholds:-missing} |
| Benchmark regression | ${bench_regression:-missing} |
| Fresh source install | ${install_summary:-missing} |
| Installed-binary self-test | \`imx self-test\` from CLI tests, install smoke, package smoke, archive smoke, and tap formula smoke |
| GLIBC symbol baseline | see Linux GLIBC Baseline section |
| Package archive SHA/no-link/smoke | package-release artifacts and linkage evidence before publication |
| Published archive smoke | post-publish \`scripts/verify-release-archive.sh\` evidence from release jobs |

## Compatibility Coverage

- Golden fixtures cover representative BMP, FARBFELD, JPEG, QOI, PBM, PGM,
  PNG, and PPM bytes.
- Curated intake corpus coverage adds representative generated or in-test
  cases for BMP RGB24/RGBA32, FARBFELD RGBA16, progressive JPEG grayscale, QOI RGB linear, PBM
  ASCII comments, PGM scaled/16-bit, PNG grayscale-alpha/RGBA16, PPM high
  maxval comments, explicit malformed diagnostics, and resource-boundary
  rejection.
- Malformed-input tests cover invalid headers, truncation, oversized dimensions,
  unsupported max values, malformed EXIF Orientation metadata, and failed CLI
  output behavior.
- ImageMagick differential tests cover decoded-pixel compatibility for
  BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM identify, prefixed identify, transcode,
  prefixed transcode, plain and prefixed resize against ImageMagick
  \`-filter Point -resize <width>x<height>!\`, plain and prefixed resize-fit
  against ImageMagick \`-filter Point -resize <width>x<height>\`,
  deterministic same-format
  rewrite paths, batch-convert runs across all supported destination formats,
  batch safety cases for collisions/existing outputs/same-path/malformed input,
  high-depth
  PPM/PNG identify/decode/transcode cases, JPEG RGB8 lossy metric cases, and
  JPEG EXIF Orientation cases compared with ImageMagick \`-auto-orient\`, and
  progressive JPEG RGB/gray/orientation input cases.
- CLI diagnostic tests cover exit code and \`error:\` prefix behavior for
  unknown prefixes, mismatched prefixes, missing paths, unsupported BMP
  variants, invalid geometry, same-path output, batch output-directory
  failures, and unsupported command-shape usage.
- \`imx self-test\` provides a no-network installed-binary smoke check for all
  supported formats and primary command surfaces. It is not an ImageMagick
  differential oracle and does not replace the corpus, fuzz, or benchmark gates.
- Cargo-fuzz targets exercise BMP, FARBFELD, JPEG, QOI, PNG, and shared PNM
  identify/decode entrypoints with seeded corpora.
- Benchmarks record library throughput, process timing, process RSS, and output
  hashes for standalone and ImageMagick oracle cases.

## Confirmed Non-Goals

- No full ImageMagick CLI parser or \`magick\` alias.
- No stdin/stdout streaming, delegates, profiles, color management, transforms
  beyond the explicit nearest-neighbor resize, resize-fit, and safe batch
  composition commands, MagickCore, or
  MagickWand.
- No prefixes beyond exact \`BMP:\`, \`FARBFELD:\`, \`JPEG:\`, \`QOI:\`, \`PBM:\`,
  \`PGM:\`, \`PNG:\`, and \`PPM:\`.
- No APNG, indexed/palette PNG, low-bit PNG, PNG color management/profile
  preservation, TIFF, PAM, PFM, GIF, WebP, indexed BMP, compressed BMP,
  bitfields BMP, OS/2 BMP, or high-depth BMP support in this conformance
  surface.
- No CMYK/YCCK JPEG, 12-bit JPEG, arithmetic-coded JPEG, lossless
  JPEG/JPEG-LS, JPEG 2000, JPEG XL, metadata/profile preservation beyond
  read-only Orientation, or JPEG color-management semantics.
EOF

cat >"$out_dir/conformance-summary.json" <<EOF
{
  "schema_version": 1,
  "version": "$version",
  "formats": ["BMP", "FARBFELD", "JPEG", "QOI", "PBM", "PGM", "PNG", "PPM"],
  "prefixes": ["BMP:", "FARBFELD:", "JPEG:", "QOI:", "PBM:", "PGM:", "PNG:", "PPM:"],
  "bmp": "uncompressed Windows BMP supports 24-bit BGR/RGB and 32-bit BGRA/RGBA rasters without color tables, compression, bitfields, OS/2 headers, or high-depth variants",
  "jpeg_progressive": "8-bit grayscale/RGB progressive JPEG input is supported",
  "jpeg_orientation": "EXIF Orientation values 1 through 8 are normalized on JPEG input",
  "resize": "exact nearest-neighbor resize is supported for BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM",
  "resize_fit": "aspect-preserving nearest-neighbor resize-fit is supported for BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM",
  "batch_convert": "safe batch-convert supports existing BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM inputs, exact uppercase target formats, deterministic output names, collision preflight, and optional resize/resize-fit composition",
  "self_test": "imx self-test creates temporary fixtures and exercises identify/transcode/resize/resize-fit/batch-convert across BMP/FARBFELD/JPEG/QOI/PBM/PGM/PNG/PPM without ImageMagick or network access",
  "cli_diagnostics": "CLI tests cover exit code and error-prefix behavior for unknown prefixes, mismatched prefixes, missing paths, unsupported variants, invalid geometry, same-path output, batch failures, and unsupported command shapes",
  "intake_reliability": "generated and in-test corpus cases cover representative supported-format intake, malformed diagnostics, and resource-boundary rejection",
  "git_rev": "$git_rev",
  "generated_at": "$generated_at",
  "differential_summary": "${differential_summary:-}",
  "curated_summary": "${curated_summary:-}",
  "fuzz_summary": "${fuzz_summary:-}",
  "benchmark_summary": "${bench_summary:-}",
  "benchmark_thresholds": "${bench_thresholds:-}",
  "benchmark_regression": "${bench_regression:-}",
  "install_summary": "${install_summary:-}",
  "glibc_symbols": $glibc_symbols_json,
  "release_dir": "$release_dir",
  "report": "CONFORMANCE_REPORT.md"
}
EOF

echo "$out_dir"
