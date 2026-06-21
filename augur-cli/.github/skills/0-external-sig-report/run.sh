#!/usr/bin/env bash
# sig-report runner: thin wrapper around the Rust CLI.
#
# Usage:
#   run.sh <rustdoc.json>                           # provided snapshot (legacy)
#   run.sh --snapshot provided:<path> [options...]  # explicit provided mode
#   run.sh --snapshot cached:<path>   [options...]  # explicit cached mode
#   run.sh --snapshot generated       [options...]  # generate via cargo rustdoc (nightly) into repo-root/reports/rustdoc.json
#
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

exec "$SCRIPT_DIR/sig-report" "$@"
