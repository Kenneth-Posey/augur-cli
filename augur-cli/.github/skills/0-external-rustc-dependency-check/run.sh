#!/usr/bin/env bash
# Run the rustc-dependency-check analyzer.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/rustc-dependency-check" "$@"

