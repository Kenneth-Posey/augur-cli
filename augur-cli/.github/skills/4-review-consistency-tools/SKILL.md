---
name: 4-review-consistency-tools
description: >
  Deterministic consistency-review tool contract for Stage 4. Defines what
  checks to run, how to invoke them, and how to map results to pass/fail
  across languages. Use with 4-review-consistency-validation.
---

# Skill: 4-Review Consistency Tools

## Purpose

Defines the deterministic tools required during consistency review, how to run
them, and how to map results to pass/fail signals. Missing documentation on
public API items is a High-severity finding.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Documentation Extractor
- Find all public API items (functions, types, modules) missing documentation comments
- Produce a structured list of documentation gaps
- Map each missing-doc finding to `severity: high`, `rule: missing-public-doc`

### Tool Category 2: AST / Syntax Analyzer (documentation and naming)
- Parse the source AST to detect missing documentation and naming convention violations
- Use the analyzer's own severity and rule fields for each finding
- Supplement or confirm documentation extractor findings

## Pass/Fail Rule

- Missing documentation on a public API item → **`fail`** (High)
- Internal undocumented items → **`pass`** with warning
- Naming violations → **`fail`** if High severity per project rules

## Standard Diagnostic Format

Map all findings from this skill's tools to:

```json
{
  "checker": "consistency-checker",
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

See `4-review-consistency-tools` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific documentation extractor commands, AST analyzer invocation,
naming rule IDs, and output schemas.
