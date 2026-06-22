---
description: "Use when user asks: execute plan phase, run plan phase, implement plan phase, start phase execution"
name: "Execute Plan Phase"
argument-hint: "plan root path and phase name or number"
agent: "agent"
---
Execute one phase of an implementation plan in the correct TDD order, applying
all implementation and review gates before considering the phase complete. When
replacement work is in scope, do not report the phase complete unless the
activation gate is complete.

This prompt is for single-phase execution only. For end-to-end whole-plan
execution across all phases, use `run-plan`.

This prompt identifies the requested phase and hands execution to the correct
orchestrator. Stage graphs, retries, checkpoints, and next-phase routing stay
with the orchestrators and `0-global-orchestration-pipeline`.

## Inputs

- Path to the plan root file in `plans/` (required).
- Phase name or phase number to execute (required).
- Current repository state (working tree must be clean before starting).
- Optional: active `orch-query` session id when this phase is part of an
  orchestrated run.

## Workflow

1. Read the plan root file. Follow all part-file links and read each part file.
2. Identify the target phase and confirm the request maps to Design, Plan,
   Implement, or Review.
3. If an `orch-query` session id is provided, read session status and halt if
   the session is not active or has unresolved decisions.
4. Route the request to the matching orchestration entrypoint:
   - Design → `design-orchestrator`
   - Plan → `plan-orchestrator`
   - Implement → `implement-orchestrator`
   - Review → `review-orchestrator`
5. Pass the plan path, requested phase identifier, current repository state,
   and any active session id. The selected orchestrator handles execution
   order, retries, failure routing, validation, and checkpoints.
6. If the requested work replaces existing behavior and the activation gate is
   incomplete, report the phase as blocked/incomplete rather than complete.
7. Report the orchestrator result without adding extra routing instructions.

## Output

1. Phase name and completion status
2. Orchestrator entrypoint used
3. Review or validation verdict and any findings resolved
4. Validation results against acceptance criteria
5. Checkpoint commit reference (hash + summary) or block reason
