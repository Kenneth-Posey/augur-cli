#!/usr/bin/env bash
# launch.sh — build and launch augur-cli
#
# Usage:
#   ./launch.sh                          # uses configs/application.yaml + configs/application.secrets.yaml
#   ./launch.sh --debug                  # verbose debug logging (debug builds only)
#   ./launch.sh --config path/to.yaml    # explicit config file (secrets must be alongside it)
#   ./launch.sh --log-filter warn,augur_cli=info
#
# When no --config is supplied, defaults to configs/application.yaml so that
# configs/application.secrets.yaml is automatically merged in at startup.
# All extra arguments are forwarded to the binary unchanged.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cd "$SCRIPT_DIR"

cargo build --release 2>&1

# Default to the repo-local config so application.secrets.yaml is picked up from
# the same directory. Pass --config explicitly to override.
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
    exec ./target/release/augur-cli "${extra_args[@]}"
else
    exec ./target/release/augur-cli --config "$SCRIPT_DIR/configs/application.yaml" "${extra_args[@]}"
fi