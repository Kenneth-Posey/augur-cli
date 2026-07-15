---
name: 0-external-customization-analyzer
description: >
  Deterministic analyzer for `.github/` customization artifacts (skill specs,
  skill files, prompts, instructions) that validates structure, detects dead
  links, and reports pass/fix/fail gates.
---

# customization-analyzer

## When to use

Use this skill when you need to validate `.github/` customization artifacts
(skills, prompts, instructions, local config) for structural integrity, dead
links, and gate-compliance before committing or deploying changes.

## Purpose

Deterministic analyzer for `.github/` customization artifacts (skill specs,
skill files, prompts, instructions) that validates structure, detects dead
links, and reports pass/fix/fail gates.

## Run

```bash
.github/skills/0-external-customization-analyzer/run.sh <ARTIFACT_PATH>... [OPTIONS]
```

## Arguments

- `<ARTIFACT_PATH>...` - One or more repository-relative or absolute paths to
  the artifacts to analyze. Required.
- `--format <FORMAT>` - Output format for the rendered analysis report.
  Possible values: `text` (human-readable deterministic text output) or `json`
  (machine-readable JSON output). Default: `text`.
- `--fail-on-gate <FAIL_ON_GATE>` - Smallest gate severity that should force
  a non-zero exit code. Possible values: `pass` (exit non-zero for any passing
  report or worse), `pass-with-fixes` (exit non-zero when the report includes
  fixes or failures), `fail` (exit non-zero only for failing reports). Default:
  `fail`.
- `-h, --help` - Print help (see a summary with `-h`).
- `-V, --version` - Print version.

Prefer `--format json` when the output will be summarized, parsed, or fed
back into another tool or model. Use `--format text` only when you need a
human-readable report.

Supported artifact paths:
- `.github/skills/<skill-slug>/SKILL.md`
- `.github/prompts/*.prompt.md`
- `.github/instructions/*.instructions.md`
- `.github/local/*.md`

## Output

The tool produces either human-readable text output (`--format text`) or
machine-readable JSON output (`--format json`). The analysis report includes
gate results with one of three severities: `pass`, `pass-with-fixes`, or
`fail`. Exit codes reflect the `--fail-on-gate` setting: when the smallest
gate severity in the report meets or exceeds the configured threshold, the
tool exits with a non-zero code.

## Examples

```bash
# Analyze a single skill spec
.github/skills/0-external-customization-analyzer/run.sh \
  .github/skills/0-global-tdd-workflow/SKILL.md

# Analyze multiple artifacts with JSON output
.github/skills/0-external-customization-analyzer/run.sh \
  .github/prompts/create-commit.prompt.md \
  --format json

# Exit non-zero for any reported gate, including pass
.github/skills/0-external-customization-analyzer/run.sh \
  .github/skills/0-global-critical-rules/SKILL.md \
  --fail-on-gate pass

# Exit non-zero when fixes or failures are reported
.github/skills/0-external-customization-analyzer/run.sh \
  .github/instructions/*.instructions.md \
  --fail-on-gate pass-with-fixes
```

## Key Files

- `.github/skills/0-external-customization-analyzer/run.sh` - Canonical wrapper for customization analyzer
