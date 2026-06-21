---
name: 4-review-consolidation-validation
description: >
  Stage 4 consolidation validation contract. Defines the confidence threshold,
  pass/fail criteria, and report format for call-graph consolidation review.
  Use alongside 4-review-consolidation-tools.
---

# Skill: 4-Review Consolidation Validation

## Purpose

Validate that the Stage 3 implementation contains no call-graph consolidation
opportunities at or above the required confidence threshold. This skill is
read-only: report findings, do not patch artifacts.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Dead Code

- No function in the production source tree has zero callers at confidence ≥ 0.7
- Functions with no callers must be removed or integrated before review passes

### 2. Duplicate Functions

- No pair of functions with identical normalized signatures exists in the same
  architectural layer at confidence ≥ 0.7
- Duplicate functions must be collapsed to a single canonical implementation
  before review passes

### 3. Chain-Collapse Candidates

- No linear call chain exists that can be collapsed without behavioral change
  at confidence ≥ 0.7
- Chain-collapse candidates must be simplified or documented as intentional
  before review passes

## Confidence Threshold

The minimum confidence score for a reportable finding is **0.7**. Findings
below this threshold are below noise level and are excluded from pass/fail
evaluation. Run the tool with `--min-confidence 0.7` to enforce this filter
at the source.

## Pass Conditions

- `dead_code_findings` array is empty
- `dedup_findings` array is empty
- `simplification_metadata` array is empty
- `statistics.dead_code_count == 0`, `statistics.dedup_groups == 0`,
  `statistics.collapses_applied == 0`

All conditions must hold simultaneously. Partial pass is not a valid state.

## Fail Conditions

- **High:** Any dead-code finding at confidence ≥ 0.7 - remove or integrate
  the unused function
- **High:** Any duplicate-function finding at confidence ≥ 0.7 - collapse
  duplicates to the canonical; update all call sites
- **High:** Any chain-collapse finding - merge the intermediate function into
  the parent or child; update call sites

A single finding of any type is sufficient to fail this checker. There are no
warnings, partial passes, or deferred findings.

## Validation Signal

| Condition | Signal |
|---|---|
| Zero findings across all types | `pass` |
| Any finding present | `fail` |
| Tool exits non-zero (runtime error) | `fail` |

No intermediate states. Pass means zero findings; fail means at least one.

## Report Format

**On pass (signal = pass):**
- Emit one summary line: `Consolidation: ✓ (0 dead-code, 0 duplicates, 0 chain-collapses)`
- Emit the JSON diagnostic block with `findings: []`

**On fail (signal = fail):**
- Emit a summary line per failing category with the finding count and first
  function ID affected
- Emit the full JSON diagnostic block with all findings populated, including
  function ID, module path, confidence, and actionable fix description for
  each finding
- Do not emit a passing summary for failing categories
