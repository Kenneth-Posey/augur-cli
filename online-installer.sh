#!/usr/bin/env bash
# augur-cli Online Installer
#
# Downloads the latest prebuilt binary from GitHub Releases and installs
# it to ~/.augur-cli/bin/ along with the required runtime assets (.github/).
#
# Directory layout after install:
#   ~/.augur-cli/bin/augur-cli                    - binary
#   ~/.augur-cli/bin/archive/                    - previous binaries (timestamped)
#   ~/.augur-cli/.github/                        - runtime agents, instructions, skills
#   ~/.augur-cli/config/application.yaml         - config (seeded on first binary launch)
#   ~/.augur-cli/config/application.secrets.yaml - secrets (user-managed, not overwritten)
#   ~/.augur-cli/config/providers/               - provider templates
#   ~/.augur-cli/logs/                           - runtime log files
#   ~/.augur-cli/sessions/                       - session JSON files
#
# Usage:
#   bash <(curl -sL https://raw.githubusercontent.com/Kenneth-Posey/augur-cli/copilot-incoming/online-installer.sh)
#
# Or download and run:
#   curl -sLO https://raw.githubusercontent.com/Kenneth-Posey/augur-cli/copilot-incoming/online-installer.sh
#   chmod +x online-installer.sh
#   ./online-installer.sh
#
# For source-based builds (requires Rust toolchain), use augur-cli/install.sh

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
REPO_OWNER="Kenneth-Posey"
REPO_NAME="augur-cli"
INSTALL_DIR="${HOME}/.augur-cli"
BIN_DIR="${INSTALL_DIR}/bin"
ARCHIVE_DIR="${BIN_DIR}/archive"
CONFIG_DIR="${INSTALL_DIR}/config"
LOG_DIR="${INSTALL_DIR}/logs"
SESSIONS_DIR="${INSTALL_DIR}/sessions"
GITHUB_ASSETS_DIR="${INSTALL_DIR}/.github"

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------
show_help() {
    cat <<EOF
Usage: online-installer.sh [OPTIONS]

Options:
  --help         Show this help message
  --version      Print the installer version
  --no-run       Download and install but do NOT run 'augur-cli once'
  --beta         Download from the latest CI release instead of a stable tag
  --dir PATH     Install to a custom directory (default: ~/.augur-cli)

This installer downloads the latest prebuilt augur-cli binary and runtime
assets from GitHub Releases.

After installation, fill in your API keys in:
  ~/.augur-cli/config/application.secrets.yaml

For source-based builds (requires Rust toolchain), use augur-cli/install.sh
instead.
EOF
}

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------
detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             echo "unsupported-${arch}" ;;
    esac
}

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux)  echo "unknown-linux-gnu" ;;
        Darwin) echo "apple-darwin" ;;
        *)      echo "unsupported-${os}" ;;
    esac
}

# ---------------------------------------------------------------------------
# Download helpers
# ---------------------------------------------------------------------------
fetch_latest_release() {
    local api_url="https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest"
    local tag
    tag="$(curl -sSfL "${api_url}" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)",/\1/')"
    if [[ -z "$tag" ]]; then
        echo "Error: Could not find any releases at ${api_url}" >&2
        exit 1
    fi
    echo "$tag"
}

fetch_latest_ci_tag() {
    local api_url="https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases?per_page=1"
    local tag
    tag="$(curl -sSfL "${api_url}" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)",/\1/')"
    if [[ -z "$tag" ]]; then
        echo "Error: Could not find any releases at ${api_url}" >&2
        exit 1
    fi
    echo "$tag"
}

download_asset() {
    local asset_name="$1"
    local output_dir="$2"
    local tag="$3"
    local url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${tag}/${asset_name}"
    echo "  Downloading: ${asset_name}"
    curl -sSfL "${url}" -o "${output_dir}/${asset_name}"
}

# ---------------------------------------------------------------------------
# Dependency check
# ---------------------------------------------------------------------------
check_deps() {
    local missing=()

    # Script-level dependencies — tools this installer needs to run.
    local script_deps=(
      "curl:curl (usually pre-installed on macOS/Linux)"
      "tar:tar (usually pre-installed)"
      "rsync:rsync (install via apt install rsync, brew install rsync)"
      "install:coreutils (install is part of coreutils)"
      "find:findutils (usually pre-installed)"
      "sed:sed (usually pre-installed)"
      "grep:grep (usually pre-installed)"
      "mktemp:coreutils (mktemp is part of coreutils)"
    )

    for entry in "${script_deps[@]}"; do
      local cmd="${entry%%:*}"
      local hint="${entry#*:}"
      if ! command -v "$cmd" &>/dev/null; then
        missing+=("  • ${cmd} — ${hint}")
      fi
    done

    # Runtime dependency — the GitHub CLI (`gh`) is required for Copilot
    # provider support in augur-cli.  Without it the Copilot provider
    # (augur-provider-copilot-sdk) cannot authenticate.
    if ! command -v gh &>/dev/null; then
      missing+=("  • gh (GitHub CLI) — required for Copilot provider support")
      missing+=("    Install: https://cli.github.com/ or")
      missing+=("      macOS: brew install gh")
      missing+=("      Linux: see https://github.com/cli/cli#installation")
    fi

    if [[ ${#missing[@]} -gt 0 ]]; then
      echo ""
      echo "  ╔══════════════════════════════════════════════════════╗"
      echo "  ║        Missing Dependencies                         ║"
      echo "  ╚══════════════════════════════════════════════════════╝"
      echo ""
      echo "  The following required tools are not installed:"
      echo ""
      for item in "${missing[@]}"; do
        echo "    ${item}"
      done
      echo ""
      echo "  Please install the missing tools, then re-run the installer."
      exit 1
    fi

    # Warn about optional runtime tools that enhance the experience.
    local optional_missing=false
    if ! command -v git &>/dev/null; then
      echo "  [info] git not found — session history and repo integration"
      echo "         will be unavailable. Install git to enable them."
      optional_missing=true
    fi

    if [[ "$optional_missing" == "true" ]]; then
      echo ""
    fi
}

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------
install() {
    local run_after="$1"
    local use_beta="$2"
    local install_prefix="$3"

    echo "============================================"
    echo "  augur-cli Online Installer"
    echo "============================================"
    echo ""

    # --- Check dependencies before doing anything ---
    check_deps

    # --- Detect platform ---
    local arch os target
    arch="$(detect_arch)"
    os="$(detect_os)"
    target="${arch}-${os}"

    if [[ "$arch" == unsupported-* || "$os" == unsupported-* ]]; then
        echo "Error: Unsupported platform: $(uname -m) / $(uname -s)" >&2
        echo "Supported targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu," >&2
        echo "                   x86_64-apple-darwin, aarch64-apple-darwin" >&2
        exit 1
    fi

    echo "Platform: ${target}"
    echo ""

    # --- Resolve release tag ---
    local tag
    if [[ "$use_beta" == "true" ]]; then
        echo "Fetching latest CI release..."
        tag="$(fetch_latest_ci_tag)"
    else
        echo "Fetching latest stable release..."
        tag="$(fetch_latest_release)"
    fi
    echo "Release tag: ${tag}"
    echo ""

    # --- Prepare temp directory ---
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "${tmpdir}"' EXIT

    # --- Download binary ---
    echo "Downloading binary..."
    download_asset "augur-cli-latest-${target}.tar.gz" "${tmpdir}" "${tag}"

    # --- Download runtime assets (.github/) ---
    echo "Downloading runtime assets (.github/)..."
    download_asset "dot-github-latest.tar.gz" "${tmpdir}" "${tag}"
    echo ""

    # --- Create directory structure ---
    mkdir -p "${BIN_DIR}"
    mkdir -p "${ARCHIVE_DIR}"
    mkdir -p "${CONFIG_DIR}"
    mkdir -p "${LOG_DIR}"
    mkdir -p "${SESSIONS_DIR}"

    # --- Install binary ---
    echo "Installing binary..."
    tar xzf "${tmpdir}/augur-cli-latest-${target}.tar.gz" -C "${tmpdir}/binary"
    local binary_src
    binary_src="$(find "${tmpdir}/binary" -name 'augur-cli' -type f | head -1)"
    if [[ -z "$binary_src" ]]; then
        echo "Error: Binary not found in downloaded archive" >&2
        exit 1
    fi

    # Archive existing binary
    if [[ -f "${BIN_DIR}/augur-cli" ]]; then
        local timestamp
        timestamp="$(date --utc +'%Y%m%dT%H%M%SZ' 2>/dev/null || date -u +'%Y%m%dT%H%M%SZ')"
        mv "${BIN_DIR}/augur-cli" "${ARCHIVE_DIR}/augur-cli-${timestamp}"
        echo "Archived: augur-cli-${timestamp}"
    fi

    install -m 755 "${binary_src}" "${BIN_DIR}/augur-cli"
    echo "Binary installed: ${BIN_DIR}/augur-cli"

    # Remove any stale cargo-installed binary that would shadow this one in PATH.
    local cargo_bin="${HOME}/.cargo/bin/augur-cli"
    if [[ -f "${cargo_bin}" ]]; then
        rm -f "${cargo_bin}"
        echo "Removed stale binary: ${cargo_bin}"
    fi

    # --- Install runtime .github/ (excludes local/ subdirectory) ---
    echo "Installing runtime assets (.github/)..."
    rm -rf "${GITHUB_ASSETS_DIR}"
    mkdir -p "${GITHUB_ASSETS_DIR}"
    tar xzf "${tmpdir}/dot-github-latest.tar.gz" -C "${tmpdir}/dot-github"
    # The archive contains a .github/ directory; copy its contents excluding local/
    if [[ -d "${tmpdir}/dot-github/.github" ]]; then
        rsync -a --exclude='local/' "${tmpdir}/dot-github/.github/" "${GITHUB_ASSETS_DIR}/"
    else
        # Flat extraction (no .github/ wrapper)
        rsync -a --exclude='local/' "${tmpdir}/dot-github/" "${GITHUB_ASSETS_DIR}/"
    fi
    echo "Runtime assets installed: ${GITHUB_ASSETS_DIR}"

    # --- Seed config files on first launch (run augur-cli once) ---
    if [[ "${run_after}" == "true" ]]; then
        echo ""
        echo "Running 'augur-cli once' to seed configuration..."
        echo "(This creates config/application.yaml, config/providers/, and"
        echo " config/application.secrets.yaml if they do not yet exist.)"
        echo ""
        export PATH="${BIN_DIR}:${PATH}"
        if "${BIN_DIR}/augur-cli" once --repo-root 2>/dev/null; then
            echo "augur-cli once completed."
        else
            echo "Warning: 'augur-cli once' exited with code $? (may be expected if no TTY)."
            echo "Configuration may need manual setup. See ${CONFIG_DIR}/"
        fi
    fi

    # --- PATH setup ---
    local bashrc="${HOME}/.bashrc"
    local path_export="export PATH=\"${BIN_DIR}:\$PATH\""
    local path_export_old="export PATH=\"\$PATH:${BIN_DIR}\""

    if grep -qF "${BIN_DIR}" "${bashrc}" 2>/dev/null; then
        local escaped_old
        escaped_old="$(printf '%s' "${path_export_old}" | sed 's/[\/&]/\\&/g')"
        sed -i "/${escaped_old}/d" "${bashrc}" 2>/dev/null || true
    fi

    if [[ ":${PATH}:" != *":${BIN_DIR}:"* ]] && ! grep -qF "PATH=\"${BIN_DIR}" "${bashrc}" 2>/dev/null; then
        printf '\n# augur-cli\n%s\n' "${path_export}" >> "${bashrc}"
        echo "Added to ${bashrc}: ${path_export}"
        echo "Run 'source ~/.bashrc' or open a new terminal to use augur-cli from anywhere."
    fi

    # --- Summary ---
    echo ""
    echo "============================================"
    echo "  Installation Complete"
    echo "============================================"
    echo ""
    echo "Binary:      ${BIN_DIR}/augur-cli"
    echo "Runtime:     ${GITHUB_ASSETS_DIR}/"
    echo "Config:      ${CONFIG_DIR}/"
    echo "Logs:        ${LOG_DIR}/"
    echo "Sessions:    ${SESSIONS_DIR}/"
    echo ""
    echo "Next steps:"
    echo "  1. Edit ${CONFIG_DIR}/application.secrets.yaml"
    echo "     and add your API keys."
    echo "  2. Source your shell or open a new terminal:"
    echo "       source ~/.bashrc"
    echo "  3. Run augur-cli:"
    echo "       augur-cli"
    echo ""
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    local run_after="true"
    local use_beta="false"
    local install_dir="${INSTALL_DIR}"

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --help)
                show_help
                exit 0
                ;;
            --version)
                echo "online-installer.sh version 1.1.0"
                exit 0
                ;;
            --no-run)
                run_after="false"
                shift
                ;;
            --beta)
                use_beta="true"
                shift
                ;;
            --dir)
                if [[ -z "${2:-}" ]]; then
                    echo "Error: --dir requires a path argument" >&2
                    exit 1
                fi
                install_dir="$2"
                shift 2
                ;;
            *)
                echo "Unknown option: $1" >&2
                show_help
                exit 1
                ;;
        esac
    done

    INSTALL_DIR="${install_dir}"
    BIN_DIR="${INSTALL_DIR}/bin"
    ARCHIVE_DIR="${BIN_DIR}/archive"
    CONFIG_DIR="${INSTALL_DIR}/config"
    LOG_DIR="${INSTALL_DIR}/logs"
    SESSIONS_DIR="${INSTALL_DIR}/sessions"
    GITHUB_ASSETS_DIR="${INSTALL_DIR}/.github"

    install "${run_after}" "${use_beta}" "${install_dir}"
}

main "$@"