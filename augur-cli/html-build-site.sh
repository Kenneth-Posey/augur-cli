#!/usr/bin/env bash
# ------------------------------------------------------------------
# html-build-site.sh
#
# Builds the public-html/ output locally for preview.
# Usage: ./html-build-site.sh [output-dir]
#
# Default output directory: public-html-temp
# Override: ./html-build-site.sh /path/to/output
#
# The committed source files (index.html, .gitignore, etc.) are
# copied from public-html/ and the generated artifacts
# (graph-data.json, api/) are placed alongside them.
# ------------------------------------------------------------------

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUTPUT_DIR="${1:-"$ROOT_DIR/public-html-temp"}"

echo "==> Output directory: $OUTPUT_DIR"

# Resolve to absolute path in case a relative path was given
OUTPUT_DIR="$(cd "$(dirname "$OUTPUT_DIR")" && pwd)/$(basename "$OUTPUT_DIR")"

# ------------------------------------------------------------------
# 1. Copy committed source files from public-html/
# ------------------------------------------------------------------
if [ ! -d "$ROOT_DIR/public-html" ]; then
  echo "ERROR: public-html/ not found at $ROOT_DIR/public-html"
  exit 1
fi

echo "==> Copying committed source files from public-html/ ..."
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
# Copy everything except an existing api/ or graph-data.json
rsync -a --exclude='api/' --exclude='graph-data.json' \
  "$ROOT_DIR/public-html/" "$OUTPUT_DIR/"

# ------------------------------------------------------------------
# 2. Generate graph-data.json
# ------------------------------------------------------------------
echo "==> Building graph data ..."
cargo run -p augur-graph-builder -- \
  --manifest-path "$ROOT_DIR/Cargo.toml" \
  --output "$OUTPUT_DIR/graph-data.json"

# ------------------------------------------------------------------
# 3. Build API docs
# ------------------------------------------------------------------
echo "==> Building API docs ..."
cargo doc --no-deps --workspace \
  --exclude augur-graph-builder \
  --target-dir "$ROOT_DIR/target"
cp -r "$ROOT_DIR/target/doc" "$OUTPUT_DIR/api"

# ------------------------------------------------------------------
# 4. Report
# ------------------------------------------------------------------
echo ""
echo "============================================"
echo "  Site built at: $OUTPUT_DIR"
echo "  Preview it with: ./html-serve-site.sh"
echo "============================================"