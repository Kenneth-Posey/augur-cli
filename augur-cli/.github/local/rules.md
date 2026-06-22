# Project-Specific Rules

## Commit Policy

### Small or Non-Phased Changes
- **Wait for explicit user confirmation before committing**
- Do not auto-commit without asking
- Route all git commands through `global-git-operator`; no other agent may run git

### Large Phased Implementations
- Implement phases in order.
- Each phase must be executed by a new background agent.
- After a phase completes:
  1. Create a commit via `global-git-operator` that records all changes from the completed phase.
  2. Start the next phase in a fresh background agent using the active plan files and current repository state.
- Do not require `/compact` or manual instruction reload between phases.

## Branching and Merging

- **Always stay on the current branch** — do not switch branches unless user requests
- **User manages merges** — do not merge into `main` without explicit user instruction
- **Only `global-git-operator` may run git** — all git inspection and mutation goes through that agent
- **Do not push without explicit user instruction** 

## Primary Context Delegation

- Require custom agents for full subtasks. If a suitable agent exists, delegate
  it as a background task before loading heavy skills, reading many files, or
  doing broad investigation in the primary context.
- Use agents for whole units of work, not partial fragments that still leave the
  primary context carrying the large investigation history.
- Background-task execution is mandatory for delegated agents unless an explicit
  repository rule states an exception.
- Use [`.github/routing.md`](../routing.md) for the detailed
  agent-by-agent routing matrix and scenario guidance.
- Keep the high-level split: specialized review goes to review agents,
  `.github/` customization goes to `global-customization-author` and
  `global-customization-reviewer`, broad repository questions go to
  `utility-question-answering`, small bounded updates go to `utility-quick-patch-code`, and all git
  work goes to `global-git-operator`.
- After an agent reports back, do not duplicate its investigation in the primary
  context unless a concrete blocker or contradiction requires follow-up.

## Tool Output and Context Discipline

- Keep tool output out of primary context unless it is needed to decide the next action.
- Use targeted file reads and bounded searches; avoid broad scans that flood context.
- Never run broad searches over `logs/`.
- Summarize key findings before carrying them forward.

## Implementation Requirements

### Test-First Development (TDD)
- Write failing tests first (Red)
- Implement minimal code to pass (Green)
- Refactor for clarity (Refactor)
- **No exceptions** — always write tests before production code

### Bug Fixes
- Add a regression test BEFORE fixing the code
- Test must fail without the fix
- Test must pass with the fix
- Prevents silent recurrence

### Code Completeness
- No stub implementations for requested scope
- No deferred behavior or TODO placeholders
- No temporary mock logic or partially wired code
- **Definition of done**: All requested behavior is fully implemented, tested, and passing

### Standards Enforcement
- Max 3 function parameters (bundle excess into struct)
- Max 5 struct fields (extract semantic sub-structs)
- Named predicates before branches (boolean derivation)
- No bare domain primitives in public APIs (use semantic newtypes)
- No unsafe blocks without explicit approval
- No magic numbers (use named constants)

## Research Snapshot Retention

Research snapshots produced by `codebase-probe` do not currently have a
verified dedicated storage path in this repo snapshot. If a session persists
one, write it to an explicitly chosen verified path in an existing directory
instead of assuming `logs/research/` exists.

- **Keep the most recent snapshot** per planning session. Do not accumulate
  stale snapshots; overwrite the previous file when producing a new one.
- **Do not commit snapshots** to the repository unless the session requires
  a committed baseline for reproducibility. When a commit is required, route
  through `global-git-operator`.
- **Replace the file** at the start of each new planning or debugging session
  to ensure consumers always start from a fresh artifact.
- Snapshots with `provenance.is_degraded = true` must be regenerated before
  being used for authoritative planning decisions.

## Definition of Done

A task is complete only when ALL of the following are true:

1. Tests are written first (TDD Red) and passing (TDD Green)
2. Implementation satisfies all tests
3. Code is refactored for clarity (TDD Refactor)
4. Local tests pass (`cargo test`)
5. Acceptance criteria from plan are met
6. Implementation is fully feature-complete for the requested scope
7. No deferred implementations, stubs, or placeholders remain
