---
name: 0-external-doc-extractor
description: >
  Extract public Rust items into summary, index, full, or missing-docs output.
  Use `--full-input` with `--tier full` when the source path is ambiguous.
---

# run.sh

## Purpose

Extract public Rust items into summary, index, full, or missing-docs output.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd <tool-dir>
cargo build --release
```

## Run

```bash
./run.sh <repo-relative-rust-path> [--tier <tier>] [--module <name>] [--full-input <mode>]
```

## Usage

- `<repo-relative-rust-path>` - Rust source file or directory path relative to the repository root; required
- `--tier <tier>` - Output tier to render: `summary` | `index` | `full` | `missing-docs` (default: `summary`)
- `--module <name>` - Module name to use for the full-doc tier (defaults to file stem); optional
- `--full-input <mode>` - Input mode for `--tier full`; only valid with `--tier full`; required when the source path does not clearly indicate Rust source

When using rustdoc JSON input for full-tier extraction, do not read the JSON
file directly in the caller; pass its path to `./run.sh` and let the tool
consume it.

## Examples

```bash
# Extract summary of all public items
./run.sh <repo-relative-rust-path>

# Extract an index for navigation
./run.sh <repo-relative-rust-path> --tier index

# Extract full documentation for a module
./run.sh <repo-relative-rust-path> --tier full --module <module-name> --full-input source

# Find undocumented public items
./run.sh <repo-relative-rust-path> --tier missing-docs
```

## Key Files

- `run.sh` - Canonical wrapper for general extraction runs
- `run-summary.sh` - Summary-tier wrapper
- `run-index.sh` - Index-tier wrapper
- `run-full.sh` - Full-doc wrapper
