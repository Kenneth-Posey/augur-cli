#!/usr/bin/env bash
# Run the test-gap-fusion analyzer.
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/test-gap-fusion" "$@"
