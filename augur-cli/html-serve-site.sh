#!/usr/bin/env bash
# ------------------------------------------------------------------
# html-serve-site.sh
#
# Launches a local HTTP server and opens the site in the browser.
# Usage: ./html-serve-site.sh [path-to-site-dir] [port]
#
# Default directory: public-html-temp
# Default port: 8080
# ------------------------------------------------------------------

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
SITE_DIR="${1:-"$ROOT_DIR/public-html-temp"}"
PORT="${2:-8080}"

# Resolve site dir to absolute path
SITE_DIR="$(cd "$(dirname "$SITE_DIR")" && pwd)/$(basename "$SITE_DIR")"

if [ ! -d "$SITE_DIR" ]; then
  echo "ERROR: Site directory not found: $SITE_DIR"
  echo ""
  echo "Run ./html-build-site.sh first to build the site,"
  echo "or pass the path to an existing build output directory."
  exit 1
fi

if [ ! -f "$SITE_DIR/index.html" ]; then
  echo "WARNING: No index.html found in $SITE_DIR"
  echo "  The directory may not be a valid site build."
fi

echo "==> Serving $SITE_DIR on http://localhost:$PORT"
echo ""

# Try python3 first, fall back to python
if command -v python3 &>/dev/null; then
  python3 -m http.server "$PORT" -d "$SITE_DIR"
elif command -v python &>/dev/null; then
  python -m http.server "$PORT" -d "$SITE_DIR"
else
  echo "ERROR: Neither python3 nor python found. Install Python to serve locally."
  exit 1
fi