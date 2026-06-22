#!/usr/bin/env bash
# doc-extractor full-doc-tier runner.
#
# Usage: run-full.sh <source-path> [--module <module-name>]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/doc-extractor" "$@" --tier full
