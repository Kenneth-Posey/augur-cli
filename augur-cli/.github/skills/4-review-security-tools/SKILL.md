---
name: 4-review-security-tools
description: >
  Universal Stage 4 security tool contract. Specifies which deterministic checks
  to run, how to invoke them, and how to map their output to pass/fail signals
  across languages. Use alongside 4-review-security-validation.
---

# Skill: 4-Review Security Tools

## Purpose

Defines the deterministic checks required during security review, how to run
them, and how to map their output to pass/fail signals. Unsafe operations
without justification comments are Critical; linter unsafe-code violations are High.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Linter with Unsafe / Security Focus
- Run the language linter with unsafe-code warnings enabled
- Normalize output to structured JSON findings
- Map `unsafe_code` lint violations to `severity: high`, `rule: unsafe-code-lint`
- Map unsafe blocks lacking a justification comment to `severity: critical`,
  `rule: unsafe-missing-safety-comment`

### Tool Category 2: AST / Syntax Analyzer (security patterns)
- Detect bare primitives that should be semantic wrapper types (prevents type confusion)
- Detect unexplained numeric magic literals
- Map bare-primitive findings on public API to `severity: high`,
  `rule: bare-primitive-public-api`
- Map magic literal findings to `severity: low`, `rule: magic-literal`

## Pass/Fail Rule

- Unsafe operation without justification comment → **`fail`** (Critical)
- Linter `unsafe_code` violations → **`fail`** (High)
- Bare primitive on public API → **`fail`** (High)
- Magic literal findings only → **`pass`** with warnings

## Standard Diagnostic Format

All findings from this skill's tools must be mapped to:

```json
{
  "checker": "security-checker",
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

See `4-review-security-tools` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific linter commands, unsafe-focus flags, AST analyzer commands,
and output schemas.
