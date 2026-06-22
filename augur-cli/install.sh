#!/usr/bin/env bash
# Builds augur-cli in release mode and installs it to ~/.augur-cli/bin/.
# Run this from the repo root to update the installed binary independently
# of any running instance.
#
# Directory layout after install:
#   ~/.augur-cli/bin/augur-cli                    - binary
#   ~/.augur-cli/bin/archive/                    - previous binaries (timestamped)
#   ~/.augur-cli/config/application.yaml         - config (created on first install)
#   ~/.augur-cli/config/application.secrets.yaml - secrets (user-managed, not overwritten)
#   ~/.augur-cli/config/providers/               - provider templates
#   ~/.augur-cli/logs/                           - runtime log files
#   ~/.augur-cli/sessions/                       - session JSON files
#
# Add ~/.augur-cli/bin to PATH to run augur-cli from anywhere.
#
# Usage: ./install.sh [--debug]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -z "${HOME:-}" ]]; then
    echo "Error: HOME environment variable is not set. Cannot install." >&2
    exit 1
fi
PROFILE="release"
CARGO_FLAGS="--release"

if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    CARGO_FLAGS=""
fi

echo "Building augur-cli (${PROFILE})..."
cargo build ${CARGO_FLAGS} -p augur-app --bin augur-cli

BINARY="${SCRIPT_DIR}/target/${PROFILE}/augur-cli"
INSTALL_DIR="${HOME}/.augur-cli"
BIN_DIR="${INSTALL_DIR}/bin"
ARCHIVE_DIR="${BIN_DIR}/archive"
CONFIG_DIR="${INSTALL_DIR}/config"
LOG_DIR="${INSTALL_DIR}/logs"
SESSIONS_DIR="${INSTALL_DIR}/sessions"

mkdir -p "${BIN_DIR}"
mkdir -p "${ARCHIVE_DIR}"
mkdir -p "${CONFIG_DIR}"
mkdir -p "${LOG_DIR}"
mkdir -p "${SESSIONS_DIR}"

# Copy .github runtime data (agents, instructions, workflows) on first install.
GITHUB_DIR="${INSTALL_DIR}/.github"
if [[ ! -d "${GITHUB_DIR}" ]]; then
    cp -a "${SCRIPT_DIR}/.github" "${GITHUB_DIR}" && rm -rf "${GITHUB_DIR}/local"
    echo "Installed: ${GITHUB_DIR}"
else
    echo ".github: ${GITHUB_DIR} (exists, not overwritten)"
fi

# Archive the existing binary (if any) before overwriting it.
EXISTING="${BIN_DIR}/augur-cli"
if [[ -f "${EXISTING}" ]]; then
    TIMESTAMP="$(date -u +'%Y%m%dT%H%M%SZ')"
    ARCHIVE_NAME="augur-cli-${TIMESTAMP}"
    mv "${EXISTING}" "${ARCHIVE_DIR}/${ARCHIVE_NAME}"
    echo "Archived: ${EXISTING} -> ${ARCHIVE_DIR}/${ARCHIVE_NAME}"
fi

cp "${BINARY}" "${BIN_DIR}/augur-cli"

# Remove any stale cargo-installed binary that would shadow this one in PATH.
CARGO_BIN="${HOME}/.cargo/bin/augur-cli"
if [[ -f "${CARGO_BIN}" ]]; then
    rm -f "${CARGO_BIN}"
    echo "Removed stale binary: ${CARGO_BIN}"
fi

# Write a starter application.yaml on first install only.
# Edit this file to configure endpoints, models, and other settings.
CONFIG_FILE="${CONFIG_DIR}/application.yaml"
if [[ ! -f "${CONFIG_FILE}" ]]; then
    cp "${SCRIPT_DIR}/configs/application.yaml" "${CONFIG_FILE}"
    # Append persistence overrides so logs and sessions go to installed locations.
    printf '\npersistence:\n  log_dir: "%s"\n  sessions_dir: "%s"\n' "${LOG_DIR}" "${SESSIONS_DIR}" >> "${CONFIG_FILE}"
    echo "Created: ${CONFIG_FILE}"
else
    # Patch an existing config that is missing the persistence section.
    if ! grep -q "^persistence:" "${CONFIG_FILE}"; then
        printf '\npersistence:\n  log_dir: "%s"\n  sessions_dir: "%s"\n' "${LOG_DIR}" "${SESSIONS_DIR}" >> "${CONFIG_FILE}"
        echo "Config:  ${CONFIG_FILE} (patched: added persistence section)"
    else
        echo "Config:  ${CONFIG_FILE} (exists, not overwritten)"
    fi
fi

mkdir -p "${CONFIG_DIR}/providers"
cp "${SCRIPT_DIR}/configs/providers/"*.yaml "${CONFIG_DIR}/providers/"

# Write application.secrets.yaml on first install only.
# Add your API keys here; this file is never overwritten by the installer.
SECRETS_FILE="${CONFIG_DIR}/application.secrets.yaml"
if [[ ! -f "${SECRETS_FILE}" ]]; then
    cp "${SCRIPT_DIR}/configs/application.secrets.template.yaml" "${SECRETS_FILE}"
    echo "Created: ${SECRETS_FILE}"
else
    echo "Secrets: ${SECRETS_FILE} (exists, not overwritten)"
fi

echo "Installed:  ${BIN_DIR}/augur-cli"
echo "Logs dir:   ${LOG_DIR}/"

BASHRC="${HOME}/.bashrc"
PATH_EXPORT="export PATH=\"${BIN_DIR}:\$PATH\""
PATH_EXPORT_OLD="export PATH=\"\$PATH:${BIN_DIR}\""

# Detect BSD vs GNU sed for in-place editing flags.
if sed --version 2>/dev/null | grep -q GNU; then
    SED_INPLACE=( -i )
else
    SED_INPLACE=( -i "" )
fi
# Remove any old append-style PATH entry for this binary.
if grep -qF "${BIN_DIR}" "${BASHRC}" 2>/dev/null; then
    sed "${SED_INPLACE[@]}" "/$(printf '%s' "${PATH_EXPORT_OLD}" | sed 's/[\/&]/\\&/g')/d" "${BASHRC}" 2>/dev/null || true
fi

if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]] || ! grep -qF "PATH=\"${BIN_DIR}" "${BASHRC}" 2>/dev/null; then
    if ! grep -qF "PATH=\"${BIN_DIR}" "${BASHRC}" 2>/dev/null; then
        printf '\n# augur-cli\n%s\n' "${PATH_EXPORT}" >> "${BASHRC}"
        echo "Added to ${BASHRC}: ${PATH_EXPORT}"
        echo "Run 'source ~/.bashrc' or open a new terminal to use augur-cli from anywhere."
    fi
fi