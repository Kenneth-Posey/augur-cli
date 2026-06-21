#!/usr/bin/env bash
# arch-linter canonical runner.
#
# Usage:
#   run.sh [repo-relative-root] [--output-format text|json] [--fail-on-findings yes|no]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/arch-linter" "$@"
