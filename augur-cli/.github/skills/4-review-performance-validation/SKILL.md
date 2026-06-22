---
name: 4-review-performance-validation
description: >
  Universal performance validation contract for Stage 4. Defines what algorithmic
  complexity, data structure selection, allocation patterns, and loop correctness
  must be verified, independent of language. Use at Stage 4 when confirming that
  the implementation avoids common performance anti-patterns before integration testing.
---

# Skill: 4-Review Performance Validation

## Purpose

Check that the implementation matches the plan's performance expectations:
algorithmic complexity, data structure choices, allocation behavior, and bounded
loops and recursion. This is a static review; runtime benchmarking is out of
scope.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

> **N/A Sections:** Omit entire validation categories that do not apply to the
> current feature (e.g., "Recursion" when no recursive functions are present,
> "Allocation Patterns" when no heap-heavy paths exist). Do not write "N/A"
> placeholder sections. A missing section implies the category was not
> applicable.

### 1. Algorithmic Complexity
- Each function's complexity class matches the plan expectation
  (O(1), O(log n), O(n), O(n log n), O(n²), etc.)
- No naive recursive algorithms where iterative solutions are expected
- Recursion is bounded with a clear base case; no unbounded recursion
- No redundant recomputation of values that could be cached or computed once

### 2. Data Structure Selection
- Data structures match their primary access pattern:
  - Sequential access → ordered list / array
  - Key-value lookup → hash map or tree map
  - Membership testing → set
- No data structure mismatches that degrade the algorithmic complexity
  (e.g., using an unsorted list for repeated membership checks)

### 3. Allocation Patterns
- No unnecessary copying or cloning in hot paths
- Collections are pre-allocated where the size is known in advance
- No heap allocation inside tight loops where stack or pre-allocated storage suffices
- String building uses efficient concatenation methods, not repeated individual appends

### 4. Loop Correctness
- All loop termination conditions are explicit and bounded
- No repeated expensive operations (I/O, parsing, compilation of patterns) inside loops
  where a one-time setup would suffice
- No obvious infinite loops

### 5. Complexity Budget
- Cyclomatic complexity per function does not exceed the project ceiling
  (default: 5; 4 is advisory; > 5 is Critical)

## Pass Conditions

- All function complexity classes match the plan
- Data structures are appropriate for their usage patterns
- No allocation anti-patterns in hot paths
- All loops are bounded; no repeated expensive operations inside loops
- Cyclomatic complexity is within budget

## Fail Conditions

- **Critical:** Algorithmic complexity worse than the plan specifies
- **Critical:** Unbounded recursion or infinite loop detected
- **Critical:** Cyclomatic complexity > 5
- **High:** Data structure mismatch that degrades complexity
- **High:** Possible unbounded recursion (base case missing or unclear)
- **Medium:** Unnecessary allocation or copy in a hot path
- **Medium:** String concatenation pattern in a loop
- **Low:** Magic numeric literal with no explanation

## Validation Signal

| Severity present | Signal |
|---|---|
| Critical or High findings | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |

## Report Format

**On pass (signal = pass):**
- Emit one summary line per validation category in the form:
  `Category Name: ✓ (brief note, e.g., "12 modules verified")`
- Emit the JSON diagnostic block with `findings: []` (or `findings` with only
  Medium/Low entries if present)
- Omit: detailed row-by-row verification tables, per-item bullet lists,
  validation checklists, and any duplicate `## Signal` section at the bottom
  - the signal is already stated in the report header

**On fail (signal = fail):**
- Emit full detail (table/bullets/evidence) only for the failing categories
- Emit the summary line format for all passing categories
- Emit the JSON diagnostic block with all findings fully populated

## Language Companion

For language-specific allocation patterns, clone/copy detection rules, loop
efficiency anti-patterns, and checker logic, see
[`4-review-performance-validation` in `.github/local/language-companions.md`](../../local/language-companions.md).
