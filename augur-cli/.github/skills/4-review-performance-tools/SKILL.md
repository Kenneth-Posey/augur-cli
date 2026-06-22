---
name: 4-review-performance-tools
description: >
  Stage 4 performance review tool contract. Specifies which deterministic
  analysis tools to run, how to invoke them, and how to map their output to
  pass/fail signals, independent of language. Use alongside
  4-review-performance-validation.
---

# Skill: 4-Review Performance Tools

## Purpose

Defines the deterministic checks required during performance review, how to run
them, and how to map their output to pass/fail signals. Cyclomatic complexity
over 5 or detected unbounded recursion is an immediate `fail`.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: AST / Complexity Analyzer
- Run the AST-based complexity analyzer against the source tree
- Detect: cyclomatic complexity violations, deep conditional chains, oversized
  functions, unexplained magic literals
- Apply this severity mapping:

| Rule | Default severity |
|---|---|
| `complexity` | High |
| `chain` | Medium |
| `length` | Medium |
| `magic` | Low |

- Override: cyclomatic complexity > 5 → Critical regardless of tool output

- Chain findings trigger when the deepest `if`/`else if` chain reaches 5 or
  more, so the default allows up to 4 chained branches.

## Pass/Fail Rule

- Cyclomatic complexity > 5 → **`fail`** (Critical)
- Unbounded recursion detected → **`fail`** (Critical)
- Advisory warning threshold (complexity 4) → **`pass`** with warnings
- Medium or low findings only → **`pass`** with warnings

## Standard Diagnostic Format

Map all tool findings to:

```json
{
  "checker": "performance-checker",
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

Use the `4-review-performance-tools` entry in
[`.github/local/language-companions.md`](../../local/language-companions.md) to find
the language-specific AST analyzer, invocation flags (max-complexity,
max-chain, max-lines), and output schema.
