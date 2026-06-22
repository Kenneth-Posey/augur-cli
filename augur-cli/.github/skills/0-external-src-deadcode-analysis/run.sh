#!/usr/bin/env bash
# src-deadcode canonical runner.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/src-deadcode-analysis" "$@"
