---
name: Build Implementation Plan
description: >
  Use when asked to create a new implementation plan for a feature, refactor,
  or migration. Produces plan artifacts and a concise readiness summary for
  user review.
argument-hint: "task description or feature scope"
agent: agent
---

# build-plan

## Workflow

1. Gather the task description and any scope constraints.
2. Apply the architecture clarity gate from the planning standards. If the
   architecture is unclear, require a dependency-design artifact before
   drafting the plan.
3. Draft the plan files from the task description and any prerequisite design
   artifact.
4. Review the plan package for completeness, missing prerequisites, and
   unresolved questions.
5. If gaps remain, report the plan path and required corrections. Do not begin
   implementation.
6. Present the plan path(s), phase summary, and validation notes to the user.
7. Wait for explicit user confirmation before implementation.

## Output

1. Plan file path(s) created
2. Phase summary (name, objective, key inputs, outputs)
3. Validation notes and any open questions
4. State that implementation waits for explicit user confirmation.
