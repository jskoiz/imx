#!/usr/bin/env bash
set -euo pipefail

max_glibc="${IMX_GLIBC_MAX:-2.34}"

if [[ $# -eq 0 ]]; then
  echo "usage: IMX_GLIBC_MAX=2.34 $0 <elf-binary> [<elf-binary>...]" >&2
  exit 2
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required for GLIBC symbol-version comparison" >&2
  exit 2
fi

dump_symbols() {
  local binary="$1"
  local output

  if [[ -n "${IMX_GLIBC_SYMBOL_COMMAND:-}" ]]; then
    read -r -a configured_command <<<"$IMX_GLIBC_SYMBOL_COMMAND"
    "${configured_command[@]}" "$binary"
    return
  fi

  local candidates=(
    "readelf --version-info"
    "objdump -T"
    "aarch64-linux-gnu-readelf --version-info"
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    read -r -a command_parts <<<"$candidate"
    if ! command -v "${command_parts[0]}" >/dev/null 2>&1; then
      continue
    fi
    if output="$("${command_parts[@]}" "$binary" 2>/dev/null)"; then
      printf '%s\n' "$output"
      return
    fi
  done

  echo "error: no objdump/readelf command could inspect GLIBC symbols for $binary" >&2
  exit 2
}

status=0
for binary in "$@"; do
  if [[ ! -f "$binary" ]]; then
    echo "error: binary not found: $binary" >&2
    status=1
    continue
  fi

  if ! dump_symbols "$binary" | python3 -c '
import os
import re
import sys

limit_text, binary = sys.argv[1:3]
text = sys.stdin.read()


def parse_version(value):
    parts = [int(part) for part in value.split(".")]
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts)


limit = parse_version(limit_text)
versions = sorted(
    {match.group(1) for match in re.finditer(r"GLIBC_([0-9]+(?:\.[0-9]+)+)", text)},
    key=parse_version,
)

if not versions:
    if os.environ.get("IMX_ALLOW_NO_GLIBC_SYMBOLS") == "1":
        print(f"{binary}: no GLIBC version references found; accepted by IMX_ALLOW_NO_GLIBC_SYMBOLS=1")
        sys.exit(0)
    print(f"error: {binary} has no GLIBC version references to prove; set IMX_ALLOW_NO_GLIBC_SYMBOLS=1 only for intentional non-glibc/static binaries", file=sys.stderr)
    sys.exit(1)

observed = versions[-1]
observed_key = parse_version(observed)
print(f"{binary}: max GLIBC_{observed}; allowed GLIBC_{limit_text}")
print("observed GLIBC versions: " + ", ".join(f"GLIBC_{version}" for version in versions))

if observed_key > limit:
    print(
        f"error: {binary} requires GLIBC_{observed}, above allowed GLIBC_{limit_text}",
        file=sys.stderr,
    )
    sys.exit(1)

print(f"{binary}: GLIBC symbol baseline passed")
' "$max_glibc" "$binary"
  then
    status=1
  fi
done

exit "$status"
