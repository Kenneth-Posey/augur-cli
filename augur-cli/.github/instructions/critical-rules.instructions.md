---
description: "Use when applying repo-wide safety, workflow, and delegation rules."
applyTo: "**"
---

# Critical Rules

- Use [`.github/local/rules.md`](../local/rules.md) for project-specific workflow,
  commit, branching, delegation, and implementation policy.
- Use [`.github/routing.md`](../routing.md) for detailed
  primary-context routing and agent-selection scenarios.
- Use `.github/copilot-instructions.md` for minimal baseline guidance.
- Ask clarifying questions only when requirements, scope, or behavioral intent are
  genuinely ambiguous. Once the task is clear enough to execute, implement without
  asking for an extra "go" signal.
- Delegate to the appropriate custom agent before loading heavy task context,
  and always run delegated agents as background tasks. Follow
  [`.github/routing.md`](../routing.md) for agent ownership by scenario.
- Keep tool output out of primary context. Cap all `shell_exec` and search
  calls that could produce large output. Never run broad searches over
  `logs/`. Summarize findings before carrying them into subsequent turns.
  Use `size-check` pre-flight estimates for broad search/list/read operations
  when available, then refine the command if the recommendation is not
  `Proceed`.
  See `.github/local/rules.md` `## Tool Output and Context Discipline` for
  the full rules.
- Route all git actions through `global-git-operator`. For small or non-phased changes,
  ask for explicit user confirmation before committing. For large phased work,
  implement phases in order and start each phase in a fresh background agent
  using the plan and current repository state; do not require `/compact` or
  manual instruction reload between phases. Follow
  [`.github/local/rules.md`](../local/rules.md) for any explicit commit events.
- When preparing to commit, if a required changelog file does not yet exist,
  invoke `global-writer-changelog` first, then proceed with the commit flow.
- Use the `0-global-critical-rules` skill and
  [`.github/local/rules.md`](../local/rules.md) for detailed execution rules:
  TDD, regression-test policy, temporary compile-target stubs needed only so
  Red tests compile, no deferred implementations, definition of done, and local
  validation expectations.
- Use focused skills such as `0-global-critical-rules`, `0-global-documentation-standards`,
  `0-global-dependency-adoption`, `0-global-line-count-check`, and
  `2-plan-architecture-planning`. For capability-key routing, look up
  `2-plan-architecture-planning`, `3-implement-behavior-wiring`,
  `3-implement-domain-implementation`, `3-implement-function-sig-implementation`, and
  `3-implement-test-suite-completion` via
  [`language-companions.md`](../local/language-companions.md) for detailed implementation rules
  instead of duplicating them here.

## Feature Implementation

When asked to implement a feature end-to-end, invoke the
`0-global-orchestration-pipeline` skill before launching agents and follow its
instructions.

## Agent Delegation Requirements

All agents launched via the task tool **must** use `mode: 'background'`.
Use `mode: 'sync'` only when the output is immediately required to determine
the next step and the task is expected to be brief.
