---
name: 0-external-customization-analyzer
description: >
  Deterministic analyzer for `.github/` customization artifacts (skill specs,
  skill files, prompts, instructions) that validates structure, detects dead
  links, and reports pass/fix/fail gates.
---

# run.sh

## Purpose

Deterministic analyzer for `.github/` customization artifacts (skill specs,
skill files, prompts, instructions) that validates structure, detects dead
links, and reports pass/fix/fail gates.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-customization-analyzer
cargo build --release
```

## Run

```bash
.github/skills/0-external-customization-analyzer/run.sh <artifact-path>... [--format <format>] [--fail-on-gate <gate>]
```

## Usage

- `<artifact-path>...` - One or more repository-relative or absolute artifact paths; required
- `--format <format>` - Output format: `text` | `json` (default: `text`)
- `--fail-on-gate <gate>` - Smallest gate that exits non-zero: `pass` | `pass-with-fixes` | `fail` (default: `fail`)

Prefer `--format json` when the output will be summarized, parsed, or fed
back into another tool or model. Use `text` only when you need a human-readable
report.

Supported artifact paths:
- - `.github/skills/<skill-slug>/SKILL.md`
- `.github/prompts/*.prompt.md`
- `.github/instructions/*.instructions.md`
- `.github/local/*.md`

## Examples

```bash
# Analyze a single skill spec
.github/skills/0-external-customization-analyzer/run.sh .github/skills/0-global-tdd-workflow/SKILL.md

# Analyze multiple artifacts with JSON output
.github/skills/0-external-customization-analyzer/run.sh \
  .github/prompts/create-commit.prompt.md \
  --format json

# Exit non-zero for any reported gate, including `pass`
.github/skills/0-external-customization-analyzer/run.sh .github/skills/0-global-critical-rules/SKILL.md --fail-on-gate pass

# Exit non-zero when fixes or failures are reported
.github/skills/0-external-customization-analyzer/run.sh .github/instructions/*.instructions.md --fail-on-gate pass-with-fixes
```

## Key Files

- `run.sh` - Canonical wrapper for customization analyzer
