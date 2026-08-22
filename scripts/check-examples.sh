#!/usr/bin/env bash
set -euo pipefail

# T16 (PROD-01): prove the five canonical examples build cleanly, pass
# `mdhtml check` without a single E/W diagnostic, and round-trip byte for
# byte through `mdhtml extract`. The mdhtml binary comes from the repository
# build only (pinned .runs/cargo-t16 target dir) — never a network download.
# The script is idempotent: it writes only to a fresh temp dir it owns.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$ROOT/.runs/cargo-t16"
BIN="$TARGET_DIR/debug/mdhtml"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

fail() {
  echo "check-examples: $*" >&2
  exit 1
}

BUILD_LOG="$WORK/cargo-build.log"
if ! cargo build --locked --target-dir "$TARGET_DIR" --manifest-path "$ROOT/Cargo.toml" -p mdhtml >"$BUILD_LOG" 2>&1; then
  cat "$BUILD_LOG" >&2
  fail "mdhtml binary build failed"
fi

for name in resume memo spec recipe chapter; do
  source_file="$ROOT/examples/$name.md"
  artifact="$WORK/$name.md.html"
  roundtrip="$WORK/$name.roundtrip.md"

  [ -f "$source_file" ] || fail "missing canonical example $source_file"

  "$BIN" build "$source_file" -o "$artifact" || fail "build failed for $name"
  for target in "$source_file" "$artifact"; do
    report="$("$BIN" check "$target" 2>&1)" || fail "check failed for $target"
    if grep -qE '^mdhtml: [EW]-[A-Z0-9-]+' <<<"$report"; then
      echo "check-examples: $target reports diagnostics:" >&2
      echo "$report" >&2
      exit 1
    fi
  done
  "$BIN" extract "$artifact" -o "$roundtrip" || fail "extract failed for $name"
  cmp -s "$source_file" "$roundtrip" || fail "round-trip mismatch for $name"
done

echo "check-examples: 5/5 examples build clean, check clean, round-trip empty"
