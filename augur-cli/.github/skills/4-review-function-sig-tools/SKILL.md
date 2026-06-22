---
name: 4-review-function-sig-tools
description: >
  Universal Stage 4 contract for function signature review. Defines which
  deterministic tools to run, how to invoke them, and how to map their output
  to pass/fail signals across languages. Use alongside
  4-review-function-sig-validation.
---

# Skill: 4-Review Function Signature Tools

## Purpose

Defines the deterministic tools for function signature review, how to invoke
them, and how to map their output to pass/fail signals. Missing plan functions
and type mismatches are Critical; oversized parameter lists are High.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Signature Report Tool
- Generate a minimal structured snapshot of implemented function-signature findings
- Compare each entry against the Function Signature Plan
- Map missing plan functions to `severity: critical`, `rule: missing-plan-function`
- Map type mismatches (plan vs. implementation) to `severity: critical`,
  `rule: signature-type-mismatch`
- Use the consolidation preset when broader refactoring evidence is needed

### Tool Category 2: AST / Syntax Analyzer (parameter counts)
- Detect functions whose parameter lists exceed the project maximum (default: 3)
- Map each oversized parameter list to `severity: high`,
  `rule: oversized-param-list`

## Pass/Fail Rule

- Missing plan function → **`fail`** (Critical)
- Type mismatch between plan and implementation → **`fail`** (Critical)
- Oversized parameter list (> project maximum) → **`fail`** (High)
- Only medium or low findings → **`pass`** with warnings

## Standard Diagnostic Format

Map all findings to:

```json
{
  "checker": "function-sig-checker",
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

See
[`.github/local/language-companions.md`](../../local/language-companions.md)
for the language-specific signature report command, plan comparison path, AST
analyzer commands, and output schemas.
