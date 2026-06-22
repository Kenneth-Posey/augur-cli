#!/usr/bin/env bash
# doc-extractor canonical runner.
#
# Usage:
#   run.sh <source-path> [--tier summary|index|full|missing-docs] [--module <name>]
#
# Tier defaults to summary when omitted.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/doc-extractor" "$@"
