---
name: 4-review-consolidation-tools
description: >
  Stage 4 consolidation-review tool contract. Defines how to invoke the
  0-external-consolidator tool, how to interpret its JSON output, and how to
  map findings to pass/fail signals. Use alongside
  4-review-consolidation-validation.
---

# Skill: 4-Review Consolidation Tools

## Purpose

Defines the deterministic tool invocation and output-mapping rules used during
consolidation review. This skill runs `0-external-consolidator` and translates
its JSON output into the standard Stage 4 diagnostic format.

## Key Files

- `README.md` - overview and usage notes

## Tool Contract

### Tool: 0-external-consolidator

Run the consolidator against the project root using JSON output and the
`0.7` minimum-confidence threshold:

```bash
.github/skills/0-external-consolidator/run.sh . --output-format json --min-confidence 0.7
```

Arguments:
- `.` - source path (project root containing `Cargo.toml`)
- `--output-format json` - machine-readable JSON output
- `--min-confidence 0.7` - report only findings with confidence ≥ 0.7

## JSON Output Schema

The tool emits JSON with the following top-level fields:

```json
{
  "format_version": 1,
  "graph_id": "<string>",
  "dead_code_findings": [
    {
      "target_function": "<function-id>",
      "module_path": "<crate::module::path>",
      "visibility": "<pub|pub(crate)|private>",
      "confidence": 0.85,
      "reason": "<why this function is considered dead>"
    }
  ],
  "dedup_findings": [
    {
      "canonical": "<function-id>",
      "duplicates": ["<function-id>", "..."],
      "confidence": 0.9
    }
  ],
  "simplification_metadata": [
    {
      "parent": "<function-id>",
      "intermediate": "<function-id>",
      "child": "<function-id>"
    }
  ],
  "statistics": {
    "dead_code_count": 0,
    "dedup_groups": 0,
    "collapses_applied": 0
  }
}
```

## Pass/Fail Rule

- All three statistics fields are `0` → **`pass`**
- Any statistic is non-zero → **`fail`**

No intermediate states. A single finding is a fail.

## Finding-to-Diagnostic Mapping

Map each finding from the tool output to the standard diagnostic format.
Include enough detail for a downstream reviewer or human to locate and fix the issue.

### Dead code (`dead_code_findings`)

```json
{
  "severity": "high",
  "rule": "dead-code",
  "location": "<module_path>",
  "message": "Remove or integrate dead function '<target_function>' in '<module_path>': <reason>",
  "tool": "0-external-consolidator",
  "evidence": "function_id: <target_function>, module_path: <module_path>, confidence: <confidence>, reason: <reason>"
}
```

### Duplicate functions (`dedup_findings`)

```json
{
  "severity": "high",
  "rule": "duplicate-function",
  "location": "<canonical function module>",
  "message": "Remove duplicate(s) of '<canonical>' and replace call sites with the canonical: <duplicates[]>",
  "tool": "0-external-consolidator",
  "evidence": "canonical: <canonical>, duplicates: [<list>], confidence: <confidence>"
}
```

### Chain-collapse candidates (`simplification_metadata`)

```json
{
  "severity": "high",
  "rule": "chain-collapse",
  "location": "<intermediate function module>",
  "message": "Collapse linear call chain: '<parent>' → '<intermediate>' → '<child>'; merge intermediate into parent or child",
  "tool": "0-external-consolidator",
  "evidence": "parent: <parent>, intermediate: <intermediate>, child: <child>"
}
```

## Standard Diagnostic Envelope

All findings from this skill's tool must be wrapped in:

```json
{
  "checker": "consolidation-checker",
  "signal": "pass|fail",
  "findings": [
    {
      "severity": "high",
      "rule": "<rule-id>",
      "location": "<module_path>",
      "message": "<actionable human-readable description>",
      "tool": "0-external-consolidator",
      "evidence": "<key fields from raw tool output>"
    }
  ]
}
```

On pass, `findings` is an empty array `[]`.
