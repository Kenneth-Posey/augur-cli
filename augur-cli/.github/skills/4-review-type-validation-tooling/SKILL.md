---
name: 4-review-type-validation-tooling
description: >
  Universal type validation tool-running contract for Stage 4. Specifies what
  deterministic tools to run for type review, how to invoke them, and how to
  map their output to pass/fail signals, independent of language. Use alongside
  4-review-type-validation to perform deterministic checks.
---

# Skill: 4-Review Type Validation Tooling

## Purpose

Specifies the deterministic tools required during type validation review, how
to invoke them, and how to map their output to pass/fail signals. Compiler
errors cause an immediate `fail`.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Compiler / Type Checker (primary gate)
- Run the compiler in check mode (no code generation) against all targets
- Non-zero exit code → **immediate `fail`** (Critical)
- Map each compiler error to `severity: critical`, `rule: compile-error`

### Tool Category 2: Linter / Static Analyzer
- Run the language linter with all warnings enabled
- Normalize output to structured JSON findings
- Map `error`-level linter findings to `severity: critical`, `rule: lint-error`
- Map `warning`-level linter findings to `severity: medium`, `rule: lint-warning`

## Pass/Fail Rule

- Compiler error (non-zero exit) → **`fail`** (Critical)
- Error-level linter finding → **`fail`** (Critical)
- Warning-level linter findings only → **`pass`** with warnings

## Standard Diagnostic Format

All findings from this skill's tools must be mapped to:

```json
{
  "checker": "type-checker",
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

Look up `4-review-type-validation-tooling` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific compiler check command, linter invocation flags, output
normalization tool, and output schemas.
