#!/usr/bin/env bash
# Canonical entrypoint for the orch-query orchestration state tool.
# Usage: .github/skills/0-external-orch-query/run.sh <subcommand> [args...]
set -euo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/orch-query" "$@"
