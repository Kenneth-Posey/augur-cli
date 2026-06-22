---
name: 4-review-behavior-tools
description: >
  Stage 4 behavior-review tool contract. Defines what tools to run, how to invoke
  them, and how to map output to pass/fail signals across languages. Use alongside
  4-review-behavior-validation.
---

# Skill: 4-Review Behavior Tools

## Purpose

Defines the tools that must run during behavior review, how to invoke them, and
how to map their output to pass/fail signals. The test runner is the primary
gate: a non-zero exit code produces an immediate `fail` before other tools run.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Test Runner (primary gate)
- Run all unit tests
- Run all integration tests
- Run all documentation-embedded tests
- Non-zero exit code from any suite → **immediate `fail`** (Critical)
- Map each failing test to a finding with `severity: critical`, `rule: test-failure`

### Tool Category 2: Structural Coverage / Gap Analyzer
- Perform structural gap analysis to identify source files, modules, or behaviors
  lacking sufficient test coverage (unit, integration, or documentation tests)
- Map `high`-priority gaps to findings with `severity: high`, `rule: coverage-gap-<type>`
- If a line-coverage tool is available, augment gap analysis with coverage data
  using `test-gap-fusion --cobertura-full`

## Pass/Fail Rule

- Any test failure (non-zero exit code) → **`fail`** (Critical)
- High-priority structural gaps → **`fail`** (High)
- Medium or low gaps only → **`pass`** with warnings

## Standard Diagnostic Format

All findings from this skill's tools must be mapped to:

```json
{
  "checker": "behavior-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "critical|high|medium|low",
      "rule": "<rule-id>",
      "location": "<file>:<line>",
      "message": "<human-readable description>",
      "tool": "<tool-name>",
      "evidence": "<raw output snippet or key value>"
    }
  ]
}
```

## Language Companion

Look up `4-review-behavior-tools` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific test runner commands, coverage tool invocation, gap analysis tool
names, and output schemas.
