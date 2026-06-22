---
name: 4-review-completeness-tools
description: >
  Stage 4 completeness-review tool contract. Defines which deterministic tools
  to run, how to invoke them, and how to map output to pass/fail signals
  across languages. Use alongside 4-review-completeness-validation.
---

# Skill: 4-Review Completeness Tools

## Purpose

Defines the deterministic tools that must run during completeness review, how
to invoke them, and how to map their output to pass/fail signals. Unfinished
stub placeholders in production code are Critical failures.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Build Diagnostics / Stub Detector
- Run the compiler or build tool in diagnostic mode and normalize its output
- Report unfinished stub placeholders (`todo`, `unimplemented`, or equivalent)
  found in production code (not test modules)
- Map each stub finding to `severity: critical`, `rule: stub-macro`
- Map compiler errors to `severity: critical`, `rule: compile-error`

### Tool Category 2: Structural Gap Analyzer
- Detect structural gaps - source files or behaviors with no corresponding tests
- Map `high`-priority gaps to findings with `severity: high`,
  `rule: coverage-gap-<type>`

## Pass/Fail Rule

- Stub placeholder in production code → **`fail`** (Critical)
- High-priority structural gap → **`fail`** (High)
- Medium or low gaps only, or stubs only in test/example code → **`pass`** with warnings

## Standard Diagnostic Format

All findings from this skill's tools must be mapped to:

```json
{
  "checker": "completeness-checker",
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

Look up `4-review-completeness-tools` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific build diagnostic commands, stub macro names, gap-analysis
commands, and output schemas.
