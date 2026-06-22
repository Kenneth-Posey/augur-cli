#!/usr/bin/env bash
# launch-dev.sh - build and launch augur-cli with repo-local config
#
# Builds in debug mode for detailed backtraces and development-friendly
# assertion messages. The binary is launched from target/debug/.
#
# Uses configs/application.yaml so that configs/application.secrets.yaml
# (if present) is automatically merged in at startup. Useful during
# development when you want to test with repo-local config changes rather
# than your installed ~/.augur-cli/ setup.
#
# For production use against the installed ~/.augur-cli/ configuration,
# use launch-release.sh instead.
#
# Usage:
#   ./launch-dev.sh
#   ./launch-dev.sh --config path/to.yaml
#   ./launch-dev.sh --log-filter warn,augur_cli=info
#
# All extra arguments are forwarded to the binary unchanged.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cd "$SCRIPT_DIR"

cargo build 2>&1

has_config=false
has_log_filter=false
for arg in "$@"; do
    [[ "$arg" == "--config" ]] && has_config=true && break
done

for arg in "$@"; do
    [[ "$arg" == "--log-filter" ]] && has_log_filter=true && break
done

extra_args=("$@")
if ! $has_log_filter; then
    extra_args=(--log-filter warn,augur_cli=info "${extra_args[@]}")
fi

if $has_config; then
    exec ./target/debug/augur-cli "${extra_args[@]}"
else
    exec ./target/debug/augur-cli --config "$SCRIPT_DIR/configs/application.yaml" "${extra_args[@]}"
fi
