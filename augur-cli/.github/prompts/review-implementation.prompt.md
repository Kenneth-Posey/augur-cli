---
description: "Use when user asks: review implementation, validate plan implementation, verify plan completion"
name: "Review Implementation"
argument-hint: "optional plan root path (defaults to most recent plan root in plans/)"
agent: "agent"
---
Review the implementation against the most recent relevant plan.

## Workflow

1. Identify the plan: use the user-provided plan root path if given; otherwise
   use the most recently updated plan root in `plans/`. Read the root plan file
   and every linked part file.
2. Build a phase-by-phase checklist from the plan and verify:
   - Red/Green/Refactor sequence
   - exact files/symbols changed
   - stale/deprecated removals
   - modular reuse and deduplication requirements
   - validation/tests and acceptance criteria
3. Verify commit state when git inspection data is available:
   - required commit events exist only when the plan or user explicitly required them
   - report whether implementation changes are committed or still pending
4. Include any available code-conformance findings for in-scope files.
5. If follow-up work is needed, create a new `plans/` file for each follow-up
   using `MM-DD-YYYY-HHMM-<followup-slug>.md`.
6. Each follow-up file must include:
   - problem statement and observed gap
   - affected phases/files/symbols
   - current vs required behavior
   - constraints/invariants/non-goals
   - TTD/TDD Red/Green/Refactor expectations
   - validation commands and acceptance criteria
   - stale/duplicate cleanup requirements
   - risk and rollback notes

## Output Format
1. Findings (ordered by severity, with file/symbol references)
2. Required remediation suggestions (one suggestion per failed/partial requirement, mapped to phase and symbol)
3. Follow-up file list (path for each created follow-up file, or `none`)
4. Commit-state summary
5. Gate decision: `pass`, `pass with follow-ups`, or `fail`
