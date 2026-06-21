#!/usr/bin/env bash
# Run the dependency-intel analyzer.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/dependency-intel" "$@"
