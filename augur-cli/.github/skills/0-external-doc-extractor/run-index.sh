#!/usr/bin/env bash
# doc-extractor index-tier runner.
#
# Usage: run-index.sh [options...]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/doc-extractor" --tier index "$@"
