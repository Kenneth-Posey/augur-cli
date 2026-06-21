#!/usr/bin/env bash
# stub-detector canonical runner.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/stub-detector" "$@"
