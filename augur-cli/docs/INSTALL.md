# augur-cli Installation Guide

## Prerequisites

- **Rust toolchain** (edition 2024 or later) — install via [rustup](https://rustup.rs/)
- **Cargo** — included with the Rust toolchain

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

## Lint

Lint with strict warnings across all production and test targets:

```sh
cargo clippy --all-targets -- -D warnings
```

## Configuration

Default program settings are loaded from `crates/augur-core/src/config/program_settings.yml`. User-specific overrides go in `crates/augur-core/src/config/user-settings.yaml`. Both files use YAML and are read at startup by the config loader.

## Quick Start

From the workspace root, launch the terminal UI:

```sh
bash launch.sh
```

The TUI starts with the default configuration. Pass additional flags or override settings by editing `user-settings.yaml` before launching.