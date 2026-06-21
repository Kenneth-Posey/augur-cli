---
name: 4-review-activation-tools
description: >
  Stage 4 activation-review tool contract. Defines how to collect deterministic
  cutover/wiring, legacy-bypass, and runtime-assertion evidence for replacement work.
---

# Skill: 4-Review Activation Tools

## Purpose

Defines the deterministic evidence collection used during activation review. This skill
maps source, test, and plan artifacts to pass/fail signals.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract (Language-Agnostic)

### Tool Category 1: Wiring Evidence Scan
- Inspect entrypoints, handlers, route tables, or callsites for the new module path
- Capture file-and-line evidence for the active replacement path
- Mark missing or ambiguous wiring evidence as fail

### Tool Category 2: Legacy-Bypass Scan
- Inspect old call paths, feature flags, and route tables for bypass evidence
- Confirm the legacy path is removed, unreachable, or off by default
- Mark surviving active legacy paths as fail

### Tool Category 3: Runtime-Assertion Scan
- Locate the test that proves the legacy path is not used and the new path is active
- Confirm the test is in scope and exercises the replacement path deterministically
- Mark missing runtime assertion coverage as fail

## Pass/Fail Rule

- Missing wiring, bypass, or runtime-assertion evidence → `fail`
- Ambiguous activation state → `fail`
- Deterministic evidence for all required categories → `pass`

## Standard Diagnostic Format

Map findings to:

```json
{
  "checker": "activation-checker",
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

Use [`../../local/language-companions.md`](../../local/language-companions.md) for any
language-specific test naming, runtime-assertion, or search conventions needed to locate
activation proof.
