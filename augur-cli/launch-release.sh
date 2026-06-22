#!/usr/bin/env bash
# launch-release.sh - build and launch augur-cli with installed config
#
# Uses the installed ~/.augur-cli/ configuration so your
# application.secrets.yaml with API keys is loaded from
# ~/.augur-cli/config/ alongside application.yaml.
#
# For development work against the repo-local configs/ directory,
# use launch-dev.sh instead.
#
# Usage:
#   ./launch-release.sh
#   ./launch-release.sh --config path/to.yaml
#   ./launch-release.sh --log-filter warn,augur_cli=info
#
# All extra arguments are forwarded to the binary unchanged.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cd "$SCRIPT_DIR"

cargo build --release 2>&1

# Do not pass --config: let the binary's default resolution check
# ~/.augur-cli/config/application.yaml first, so the secrets file
# from ~/.augur-cli/config/application.secrets.yaml is found alongside it.
exec ./target/release/augur-cli "$@"
