---
name: 4-review-architecture-tools
description: >
  Architecture review tool contract for Stage 4. Defines which deterministic
  analysis tools to run, how to invoke them, and how to map their output to
  pass/fail signals across languages. Use alongside
  4-review-architecture-validation.
---

# Skill: 4-Review Architecture Tools

## Purpose

Specifies the deterministic analysis tools that must run during architecture review,
how to invoke them, and how to map their output to pass/fail signals.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Architecture / Boundary Linter
- Run the architecture linter against the source tree
- Capture structured output (JSON preferred)
- Extract findings for boundary-contract violations, wrong-direction dependencies,
  and layer breaches
- Map each finding to severity using the linter's own severity field
- Any `critical` or `high` finding → mark signal candidate `fail`

### Tool Category 2: Module Graph Analyzer
- Build the full module dependency graph
- Surface cycles as repeated node paths in the edge list
- Map cycle findings to `severity: critical`
- Any cycle detected → mark signal candidate `fail`

### Tool Category 3: Dependency Intelligence
- Analyze the dependency manifest for advisory or security issues
- Map advisory findings by their embedded severity
- Treat critical/high dependency findings as architecture-stability blockers
- Any `critical` or `high` advisory → mark signal candidate `fail`

## Pass/Fail Rule

- Any `critical` or `high` finding from the architecture linter → **`fail`**
- Any cycle detected by the module graph tool → **`fail`** (Critical)
- Any `critical` or `high` advisory from the dependency advisor → **`fail`**
- `medium` or `low` findings only → **`pass`** with warnings

## Diagnostic Format

All findings from this skill's tools must be mapped to:

```json
{
  "checker": "architecture-checker",
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

Look up `4-review-architecture-tools` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific tool names, build commands, invocation flags, and output schemas.
