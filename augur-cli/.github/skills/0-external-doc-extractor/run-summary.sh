#!/usr/bin/env bash
# doc-extractor summary-tier runner.
#
# Usage: run-summary.sh [options...]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/doc-extractor" --tier summary "$@"
