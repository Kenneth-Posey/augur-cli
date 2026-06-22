#!/usr/bin/env bash
# customization-analyzer canonical runner.
#
# Usage:
#   run.sh <artifact-path>... [--format text|json] [--fail-on-gate pass|pass-with-fixes|fail]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/customization-analyzer" "$@"
