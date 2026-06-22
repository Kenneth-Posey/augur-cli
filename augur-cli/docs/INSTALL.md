# augur-cli Installation Guide

## Prerequisites

### GitHub Copilot CLI extension (for the Copilot SDK provider)

The `augur-provider-copilot-sdk` crate requires the official
[GitHub Copilot CLI](https://docs.github.com/en/copilot/using-github-copilot/using-github-copilot-in-the-command-line)
extension to be installed and authenticated on your system.

Install and authenticate:

```sh
# Install the gh extension
gh extension install github/gh-copilot

# Authenticate with GitHub (required before using gh copilot)
gh auth login

# Verify the extension works
gh copilot --version
```

The Copilot SDK provider spawns `gh copilot` subprocesses for chat sessions,
background agents, and guided-plan reviews. Without this extension installed
and authenticated, any Copilot SDK-based provider configuration will fail
at runtime with a subprocess error.

This setup isn't required if you want to use OpenRouter as your provider.

### Rust prerequisites

- **Rust toolchain** (edition 2024 or later) - install via [rustup](https://rustup.rs/)
- **Cargo** - included with the Rust toolchain

Verify your toolchain:

```sh
rustc --version   # should show 1.85+ or later
cargo --version
```

## Build

Clone the repository and build all workspace crates from the root.

The workspace contains ten crates under `crates/`. A single `cargo build` compiles all of them.

## Test

Run the full test suite:

```sh
cargo test
```

## Configuration

Configuration is loaded from `~/.augur-cli/config/`. On first launch, the
binary creates this directory and populates it with:

- `application.yaml` -- the main config file (endpoints, agent settings,
  persistence paths, program settings, user settings)
- `application.secrets.yaml` -- API keys and credentials (never committed)
- `providers/*.yaml` -- provider-specific defaults

### Program settings (excluded directories, etc.)

Program-level defaults such as excluded directory patterns live in the
`program_settings:` section of `~/.augur-cli/config/application.yaml`.
When that section is absent, hardcoded defaults are used.

### User settings (last endpoint, model, reasoning effort)

Your active endpoint, model, and reasoning-effort selections are persisted
to the `user_settings:` section of `~/.augur-cli/config/application.yaml`
automatically at shutdown and restored on the next launch.

## Quick Start

From the workspace root, launch the terminal UI:

```sh
# Two launch scripts are provided:
#
#   bash launch-dev.sh       # uses repo-local configs/ (for development)
#   bash launch-release.sh   # uses ~/.augur-cli/ config (for production)
#
# The release variant loads your API keys from
# ~/.augur-cli/config/application.secrets.yaml.
bash launch-release.sh
```

The TUI starts with your installed configuration from `~/.augur-cli/config/application.yaml`.
Pass additional flags or edit `~/.augur-cli/config/application.yaml` to change settings.